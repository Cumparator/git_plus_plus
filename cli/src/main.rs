use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::fs;
use std::io::Write; // Нужно для записи в .git/info/exclude

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
        #[arg(num_args = 0.., help = "Список контекстов (remotes) в формате name или name=url")]
        remotes: Vec<String>,
    },
    Add {
        #[arg(short, long)]
        message: String,
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
        #[arg(help = "ID ноды или тег (пока только ID)")]
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

    // Специальная обработка Init, так как для Dispatcher нужен уже существующий репо
    if let Commands::Init { remotes } = cli.command {
        if gpp_dir.exists() {
            println!("Репозиторий Git++ уже существует");
            return Ok(());
        }
        println!("Инициализация Git++...");
        fs::create_dir_all(&gpp_dir).context("Не удалось создать .gitpp")?;

        JsonStorage::new(&db_path)
            .map_err(|e| anyhow::anyhow!(e))
            .context("Ошибка создания хранилища")?;

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

            println!("Настройка контекста '{}'...", name);

            git.init_context(name, url)
                .map_err(|e| anyhow::anyhow!("Failed to init context {}: {}", name, e))?;

            if i == 0 {
                git.switch_context(name)
                    .map_err(|e| anyhow::anyhow!("Failed to switch to {}: {}", name, e))?;
            }
        }

        // Обновляем .git/info/exclude (чтобы git игнорировал папки других контекстов)
        let exclude_path = current_dir.join(".git").join("info").join("exclude");
        if let Some(parent) = exclude_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(&exclude_path)?;

        writeln!(file, ".gitpp")?;
        writeln!(file, ".git_*")?;

        println!("Готово!");
        return Ok(());
    }

    if !gpp_dir.exists() {
        anyhow::bail!("Репозиторий не найден. Запустите gpp init");
    }

    // --- Dependency Injection ---
    let storage = Box::new(JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?);
    let backend_main = Box::new(GitRepo::new(&current_dir));
    let backend_aux = Box::new(GitRepo::new(&current_dir)); // Для PushManager

    let graph = VersionGraph::new(storage, backend_main);
    let mut dispatcher = CommandDispatcher::new(graph, backend_aux);

    // --- CLI -> Command DTO ---
    let get_head = || -> Result<Option<NodeId>> {
        if head_path.exists() {
            let h = fs::read_to_string(&head_path)?;
            let pid = h.trim().to_string();
            if pid.is_empty() { Ok(None) } else { Ok(Some(NodeId(pid))) }
        } else {
            Ok(None)
        }
    };

    let cmd_dto = match &cli.command {
        Commands::Init { .. } => unreachable!(),

        Commands::Add { message } => {
            let parents = get_head()?.map(|h| vec![h]).unwrap_or_default();
            Command::Add {
                message: message.clone(),
                author: Author { name: "User".into(), email: "user@example.com".into() },
                parents,
            }
        },

        Commands::Log => Command::Log,

        Commands::Chrm { remote, url, node, remove } => {
            let target = if let Some(id) = node {
                Some(NodeId(id.clone()))
            } else {
                get_head()?
            };
            Command::ChangeRemote {
                remote: remote.clone(),
                url: url.clone(),
                node: target,
                remove: *remove
            }
        },

        Commands::Push { remote, url, node, dry_run } => {
            let target = if let Some(id) = node {
                Some(NodeId(id.clone()))
            } else {
                get_head()?
            };
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

    match dispatcher.dispatch(cmd_dto) {
        Ok(result) => {
            match result {
                CmdResult::Success(msg) => {
                    println!("{}", msg);
                    // Если это был Add, надо обновить HEAD (это логика приложения, а не Core)
                    // В идеале Dispatcher должен возвращать измененные данные, но для простоты:
                    match &cli.command {
                        Commands::Checkout { node } => {
                            fs::write(&head_path, node.as_bytes())?;
                        },
                        // Для Add обновление HEAD пока оставим "как есть" (нужно парсить msg или менять return type),
                        // но для Checkout это критично.
                        _ => {}
                    }
                },
                CmdResult::Output(text) => println!("{}", text),
                CmdResult::None => {},
            }
        },
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}