use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

// Импортируем типы из gpp_core
use gpp_core::types::{Author, NodeId, RemoteRef};
use gpp_core::version_graph::VersionGraph;
use gpp_core::push_manager::PushManager;
use gpp_core::storage::GraphStorage;

// Импортируем наши реализации
// Убедись, что модули объявлены в main.rs или lib.rs
use backend_git::git_repo::GitRepo;

// Если JsonStorage лежит в gpp_core или соседнем модуле, поправь путь.
// Если он в этом же крейте в файле json_storage.rs:
use storage_file::json_storage::JsonStorage;

#[derive(Parser)]
#[command(name = "gpp")]
#[command(about = "Git Plus Plus CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
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
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let current_dir = std::env::current_dir()?;

    let gpp_dir = current_dir.join(".gitpp");
    let db_path = gpp_dir.join("graph.json");
    let head_path = gpp_dir.join("HEAD");

    match cli.command {
        Commands::Init => {
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

            git.init_bare(".git")
                .map_err(|e| anyhow::anyhow!("{}", e))
                .context("Ошибка инициализации Git")?;

            println!("Готово!");
        }

        Commands::Add { message } => {
            if !gpp_dir.exists() {
                anyhow::bail!("Репозиторий не найден");
            }

            let storage = Box::new(JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?);
            let backend = Box::new(GitRepo::new(&current_dir));
            let mut graph = VersionGraph::new(storage, backend);

            let parents = if head_path.exists() {
                let h = fs::read_to_string(&head_path)?;
                let pid = h.trim().to_string();
                if pid.is_empty() { vec![] } else { vec![NodeId(pid)] }
            } else { vec![] };

            let author = Author { name: "User".into(), email: "user@example.com".into() };

            println!("Коммит...");

            let new_node_id = graph.add_node(parents, author, message)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            fs::write(&head_path, new_node_id.0.as_bytes())?;
            println!("Нода создана: {}", new_node_id.0);
        }

        Commands::Log => {
            if !gpp_dir.exists() { anyhow::bail!("Репозиторий не найден"); }

            let storage = JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;

            if head_path.exists() {
                println!("HEAD: {}", fs::read_to_string(&head_path)?.trim());
            }

            let roots = storage.list_roots()
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            for root in roots {
                let node = storage.load_node(&root)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("Root: {} [{}]", root.0, node.message);
            }
        }

        Commands::Chrm { remote, url, node, remove } => {
            if !gpp_dir.exists() { anyhow::bail!("Репозиторий не найден"); }

            let target_node_id = if let Some(id) = node {
                NodeId(id)
            } else if head_path.exists() {
                NodeId(fs::read_to_string(&head_path)?.trim().to_string())
            } else {
                anyhow::bail!("HEAD пуст");
            };

            let storage = Box::new(JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?);
            let backend = Box::new(GitRepo::new(&current_dir));
            let mut graph = VersionGraph::new(storage, backend);

            if remove {
                graph.remove_remote_permission(&target_node_id, &remote)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("Права удалены");
            } else {
                let u = url.ok_or_else(|| anyhow::anyhow!("Нужен URL"))?;
                let r = RemoteRef { name: remote, url: u, specs: Default::default() };
                graph.add_remote_permission(&target_node_id, r)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("Права добавлены");
            }
        }

        Commands::Push { remote, url, node, dry_run } => {
            if !gpp_dir.exists() { anyhow::bail!("Репозиторий не найден"); }

            let target_node_id = if let Some(id) = node {
                NodeId(id)
            } else if head_path.exists() {
                NodeId(fs::read_to_string(&head_path)?.trim().to_string())
            } else {
                anyhow::bail!("Нет ноды для пуша");
            };

            let remote_url = url.unwrap_or_else(|| format!("git@github.com:{}.git", remote));
            let remote_ref = RemoteRef { name: remote, url: remote_url, specs: Default::default() };

            let storage = Box::new(JsonStorage::new(&db_path).map_err(|e| anyhow::anyhow!(e))?);
            let backend_g = Box::new(GitRepo::new(&current_dir));
            let backend_p = GitRepo::new(&current_dir);

            let graph = VersionGraph::new(storage, backend_g);
            let push_mgr = PushManager::new(&graph, &backend_p);

            match push_mgr.push(&target_node_id, &remote_ref, dry_run) {
                Ok(true) => println!("Пуш выполнен"),
                Ok(false) => println!("Нечего пушить"),
                Err(e) => eprintln!("Ошибка: {}", e),
            }
        }
    }
    Ok(())
}