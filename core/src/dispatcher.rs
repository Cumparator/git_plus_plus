use std::error::Error;
use std::collections::HashMap;
use crate::version_graph::VersionGraph;
use crate::backend::{RepoBackend, GraphOps};
use crate::push_manager::PushManager;
use crate::types::{NodeId, Author, RemoteRef};
use crate::plugins::PluginManager;

/// Результат выполнения команды
#[derive(Debug)]
pub enum CmdResult {
    Success(String),
    Output(String),
    None,
}

/// Абстракция команды (DTO)
#[derive(Debug, Clone)]
pub enum Command {
    Add {
        message: String,
        author: Author,
        parents: Vec<NodeId>,
    },
    Log,
    ChangeRemote {
        remote: String,
        url: Option<String>,
        node: Option<NodeId>,
        remove: bool,
    },
    Push {
        remote_name: String,
        remote_url: String, // В реальном коде можно доставать из конфига, здесь передаем явно
        node: Option<NodeId>,
        dry_run: bool,
    },
    // Custom allows plugins/extensions to pass raw args
    Custom {
        name: String,
        args: Vec<String>,
    }
}

/// Спецификация команды для регистрации (из диаграммы)
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn CommandHandler>,
}

/// Трейт для динамических обработчиков команд
pub trait CommandHandler: Send + Sync {
    fn execute(&self, args: &[String], graph: &mut VersionGraph) -> Result<CmdResult, Box<dyn Error>>;
}

pub struct CommandDispatcher {
    // Диспетчер владеет графом
    graph: VersionGraph,
    // Нам нужен отдельный экземпляр backend (или clone) для операций PushManager,
    // так как graph владеет своим backend'ом.
    // В данном случае предполагаем, что aux_backend — это "чистый" коннектор к той же директории.
    aux_backend: Box<dyn RepoBackend>,

    plugin_mgr: PluginManager,

    // Реестр динамических команд
    registry: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandDispatcher {
    pub fn new(
        graph: VersionGraph,
        aux_backend: Box<dyn RepoBackend>,
    ) -> Self {
        Self {
            graph,
            aux_backend,
            plugin_mgr: PluginManager::new(),
            registry: HashMap::new(),
        }
    }

    pub fn register_command(&mut self, spec: CommandSpec) {
        self.registry.insert(spec.name, spec.handler);
    }

    pub fn dispatch(&mut self, cmd: Command) -> Result<CmdResult, Box<dyn Error>> {
        match cmd {
            Command::Add { message, author, parents } => {
                let node_id = self.graph.add_node(parents, author, message)?;
                Ok(CmdResult::Success(format!("Node created: {}", node_id.0)))
            }

            Command::Log => {
                // Log - операция чтения, используем graph как GraphOps
                let roots = self.graph.get_roots()?; // Нужно добавить get_roots в VersionGraph или использовать storage напрямую
                // Примечание: VersionGraph не экспонирует list_roots, поэтому добавим логику здесь
                // или предположим расширение API VersionGraph.
                // Для примера выведем простой текст.
                let mut output = String::new();
                for root in roots {
                    let node = self.graph.get_node(&root)?;
                    output.push_str(&format!("Root: {} [{}]\n", root.0, node.message));
                }
                Ok(CmdResult::Output(output))
            }

            Command::ChangeRemote { remote, url, node, remove } => {
                let target_node = node.ok_or("Node ID required for chrm")?;

                if remove {
                    self.graph.remove_remote_permission(&target_node, &remote)?;
                    Ok(CmdResult::Success(format!("Removed permission for remote '{}'", remote)))
                } else {
                    let u = url.ok_or("URL required for adding remote")?;
                    let r = RemoteRef { name: remote.clone(), url: u, specs: Default::default() };
                    self.graph.add_remote_permission(&target_node, r)?;
                    Ok(CmdResult::Success(format!("Added permission for remote '{}'", remote)))
                }
            }

            Command::Push { remote_name, remote_url, node, dry_run } => {
                let target_node = node.ok_or("Node ID required for push")?;

                // Создаем PushManager on-the-fly, передавая ссылки на граф и backend
                let push_mgr = PushManager::new(&self.graph, self.aux_backend.as_ref());

                let remote_ref = RemoteRef {
                    name: remote_name,
                    url: remote_url,
                    specs: Default::default(),
                };

                match push_mgr.push(&target_node, &remote_ref, dry_run)? {
                    true => Ok(CmdResult::Success("Push completed successfully".into())),
                    false => Ok(CmdResult::Success("Nothing to push (up to date)".into())),
                }
            }

            Command::Custom { name, args } => {
                if let Some(handler) = self.registry.get(&name) {
                    handler.execute(&args, &mut self.graph)
                } else {
                    Err(format!("Unknown command: {}", name).into())
                }
            }
        }
    }
}

// Расширение VersionGraph для удобства (в реальном проекте добавить в version_graph.rs)
impl VersionGraph {
    pub fn get_roots(&self) -> Result<Vec<NodeId>, Box<dyn Error>> {
        // Здесь мы обращаемся к приватному storage, что недопустимо снаружи.
        // В реальном коде нужно добавить метод `list_roots` в `VersionGraph`.
        // Пока сделаем хак через трейт GraphStorage, если он публичный, или оставим заглушку.
        // В рамках задания, предполагаем, что метод list_roots проксируется:
        // self.storage.list_roots().map_err(|e| e.into())
        Err("VersionGraph::list_roots() needs to be public".into())
    }
}