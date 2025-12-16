use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use colored::*; // <--- Цвет
use dialoguer::{Input}; // <--- Интерактивность

use gpp_core::types::{Author, NodeId};
use gpp_core::version_graph::VersionGraph;
use gpp_core::dispatcher::{CommandDispatcher, Command, CmdResult};

use backend_git::git_repo::GitRepo;
use storage_file::json_storage::JsonStorage;

use tracing_subscriber;

#[derive(Parser)]
#[command(name = "gpp")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(num_args = 0.., help = "Список контекстов (remotes)")]
        remotes: Vec<String>,
    },
    Add {
        #[arg(short, long)]
        message: Option<String>, // <--- Стал Option
        #[arg(short, long)]
        parents: Option<Vec<String>>,
        #[arg(short, long, num_args = 0..)]
        remotes: Option<Vec<String>>,
    },
    Log,
    Chrm {
        #[arg(short, long)]
        remote: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long, action)]
        remove: bool,
    },
    Push {
        #[arg(short, long, default_value = "origin")]
        remote: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    Checkout {
        #[arg(help = "ID ноды")]
        node: String,
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let current_dir = std::env::current_dir()?;
    let gpp_dir = current_dir.join(".gitpp");
    let db_path = gpp_dir.join("graph.json");
    let head_path = gpp_dir.join("HEAD");

    // --- INIT ---
    if let Commands::Init { remotes } = cli.command {
        if gpp_dir.exists() {
            println!("{}", "Репозиторий Git++ уже существует".yellow());
            return Ok(());
        }
        println!("{}", "Инициализация Git++...".green().bold());
        
        fs::create_dir_all(&gpp_dir).context("Не удалось создать .gitpp")?;
        fs::write(&db_path, "{}").context("Не удалось создать graph.json")?;
        JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
        let git = GitRepo::new(&current_dir);

        let targets: Vec<String> = if remotes.is_empty() {
            vec!["origin".to_string()]
        } else {
            remotes
        };

        for (i, target_spec) in targets.iter().enumerate() {
            let (name, url) = match target_spec.split_once('=') {
                Some((n, u)) => (n, Some(u)),
                None => (target_spec.as_str(), None),
            };

            println!("Настройка контекста '{}'...", name.cyan());

            git.init_context(name, url)
                .map_err(|e| anyhow::anyhow!("Failed to init context {}: {}", name, e))?;

            if i == 0 {
                git.switch_context(name)
                    .map_err(|e| anyhow::anyhow!("Failed to switch to {}: {}", name, e))?;
            }
        }

        // Обновляем .git/info/exclude
        let exclude_path = current_dir.join(".git").join("info").join("exclude");
        if let Some(parent) = exclude_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new().write(true).append(true).create(true).open(&exclude_path)?;
        writeln!(file, ".gitpp")?;
        writeln!(file, ".git_*")?;

        println!("{} Готово!", "SUCCESS:".green().bold());
        return Ok(());
    }

    if !gpp_dir.exists() {
        // Красивый вывод ошибки
        anyhow::bail!("{} Запустите gpp init", "Репозиторий не найден.".red().bold());
    }

    // --- Dependency Injection ---
    let storage = Box::new(JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?);
    let backend_main = Box::new(GitRepo::new(&current_dir));
    let backend_aux = Box::new(GitRepo::new(&current_dir));

    let graph = VersionGraph::new(storage, backend_main);
    let mut dispatcher = CommandDispatcher::new(graph, backend_aux);

    let get_head = || -> Result<Option<NodeId>> {
        if head_path.exists() {
            let h = fs::read_to_string(&head_path)?;
            let pid = h.trim().to_string();
            if pid.is_empty() { Ok(None) } else { Ok(Some(NodeId(pid))) }
        } else {
            Ok(None)
        }
    };

    // --- MAPPING CLI -> COMMAND ---
    let cmd_dto = match &cli.command {
        Commands::Init { .. } => unreachable!(),

        Commands::Add { message, parents, remotes } => {
            // ИНТЕРАКТИВНОСТЬ: Если нет сообщения, спрашиваем
            let msg = match message {
                Some(m) => m.clone(),
                None => {
                    Input::new()
                        .with_prompt("Введите сообщение коммита")
                        .interact_text()?
                }
            };
            
            let resolved_parents = if let Some(p_list) = parents {
                p_list.iter().map(|s| NodeId(s.clone())).collect()
            } else {
                get_head()?.map(|h| vec![h]).unwrap_or_default()
            };

            Command::Add {
                message: msg,
                author: Author { name: "User".into(), email: "user@example.com".into() },
                parents: resolved_parents,
                target_remotes: remotes.clone(),
            }
        },

        Commands::Log => Command::Log,

        Commands::Chrm { remote, url, node, remove } => {
            let target = if let Some(id) = node { Some(NodeId(id.clone())) } else { get_head()? };
            Command::ChangeRemote {
                remote: remote.clone(),
                url: url.clone(),
                node: target,
                remove: *remove
            }
        },

        Commands::Push { remote, url, node, dry_run } => {
            let target = if let Some(id) = node { Some(NodeId(id.clone())) } else { get_head()? };
            let u = url.clone().unwrap_or_else(|| format!("git@github.com:{}.git", remote));
            Command::Push {
                remote_name: remote.clone(),
                remote_url: u,
                node: target,
                dry_run: *dry_run
            }
        },

        Commands::Checkout { node } => {
            Command::Checkout { node: NodeId(node.clone()) }
        }
    };

    // --- DISPATCH & OUTPUT ---
    match dispatcher.dispatch(cmd_dto) {
        Ok(result) => {
            match result {
                CmdResult::Success(msg) => {
                    // Зеленый успех
                    println!("{} {}", "SUCCESS:".green().bold(), msg);
                    
                    if let Commands::Add { .. } = &cli.command {
                        if let Some(id) = msg.strip_prefix("Node created: ") {
                            fs::write(&head_path, id.trim())?;
                        }
                    }
                    if let Commands::Checkout { node } = &cli.command {
                        fs::write(&head_path, node)?;
                    }
                },
                CmdResult::Output(text) => println!("{}", text),
                CmdResult::None => {},
            }
        },
        Err(e) => {
            // Красная ошибка
            eprintln!("{} {}", "ERROR:".red().bold(), e);
            // Можно не делать exit(1), чтобы anyhow сам обработал, 
            // но так красивее
        },
    }

    Ok(())
}