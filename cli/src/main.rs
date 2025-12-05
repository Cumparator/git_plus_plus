use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use gpp_core::types::{Author, NodeId};
use gpp_core::version_graph::VersionGraph;
use backend_git::GitRepo;
use storage_file::json_storage::JsonStorage;

/// Git++: Экспериментальная система контроля версий
#[derive(Parser)]
#[command(name = "gpp")]
#[command(about = "Git Plus Plus CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Инициализация нового репозитория
    Init,

    /// Создание новой ноды (аналог git commit -am)
    Add {
        /// Сообщение коммита
        #[arg(short, long)]
        message: String,
    },

    /// Просмотр истории (пока только корневые ноды)
    Log,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let current_dir = std::env::current_dir()?;

    let gpp_dir = current_dir.join(".gitpp");
    let db_path = gpp_dir.join("graph.json");
    let head_path = gpp_dir.join("HEAD"); // Файл, где хранится ID текущей ноды

    match cli.command {
        Commands::Init => {
            if gpp_dir.exists() {
                println!("Репозиторий Git++ уже существует в {:?}", current_dir);
                return Ok(());
            }

            println!("Инициализация Git++...");

            fs::create_dir_all(&gpp_dir).context("Не удалось создать .gitpp")?;

            JsonStorage::new(&db_path).context("Ошибка создания хранилища")?;

            let git = GitRepo::new(&current_dir);
            git.init_bare(".git").context("Ошибка инициализации Git")?;

            println!("Готово! Репозиторий создан.");
        }

        Commands::Add { message } => {
            if !gpp_dir.exists() {
                anyhow::bail!("Репозиторий не найден. Сначала выполните `gpp init`");
            }

            let storage = Box::new(JsonStorage::new(&db_path)?);
            let backend = Box::new(GitRepo::new(&current_dir));
            let mut graph = VersionGraph::new(storage, backend);

            let parents = if head_path.exists() {
                let head_content = fs::read_to_string(&head_path)?;
                let parent_id = head_content.trim().to_string();
                if parent_id.is_empty() {
                    vec![]
                } else {
                    vec![NodeId(parent_id)]
                }
            } else {
                vec![]
            };

            let author = Author {
                name: "User".to_string(),
                email: "user@example.com".to_string(),
            };

            println!("Создание снимка и коммит...");

            let new_node_id = graph.add_node(parents, author, message)
                .map_err(|e| anyhow::anyhow!(e))?;

            fs::write(&head_path, new_node_id.0.as_bytes())
                .context("Не удалось обновить HEAD")?;

            println!("Нода создана: {}", new_node_id.0);
        }

        Commands::Log => {
            if !gpp_dir.exists() {
                anyhow::bail!("Репозиторий не найден.");
            }

            let storage = JsonStorage::new(&db_path)?;

            if head_path.exists() {
                let head_id = fs::read_to_string(&head_path)?.trim().to_string();
                println!("Текущий HEAD: {}", head_id);

                if let Ok(node) = storage.load_node(&NodeId(head_id)) {
                    println!("Сообщение: {}", node.message);
                    println!("Автор: {}", node.author.name);
                }
            } else {
                println!("HEAD пуст (истории нет).");
            }

            println!("\n--- Корневые ноды графа ---");
            let roots = storage.list_roots().map_err(|e| anyhow::anyhow!(e))?;
            for root in roots {
                println!("Root: {:?}", root);
            }
        }
    }

    Ok(())
}