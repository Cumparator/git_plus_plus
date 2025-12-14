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
    Checkout {
        node: NodeId,
    },
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

            Command::Log {} => {
                let mut output = String::new();
                let mut queue = std::collections::VecDeque::new();
                let mut visited = std::collections::HashSet::new();

                let roots = self.graph.list_roots()?;
                for r in roots {
                    queue.push_back(r);
                }

                while let Some(current_id) = queue.pop_front() {
                    if !visited.insert(current_id.clone()) {
                        continue;
                    }

                    let node = self.graph.get_node(&current_id)?;

                    output.push_str(&format!("Commit: {}\n", current_id.0));
                    output.push_str(&format!("Author: {} <{}>\n", node.author.name, node.author.email));
                    output.push_str(&format!("Message: {}\n", node.message));
                    output.push_str("------------------------------\n");

                    for parent_id in node.parents {
                        queue.push_back(parent_id);
                    }
                }

                if output.is_empty() {
                    Ok(CmdResult::Output("History is empty.".to_string()))
                } else {
                    Ok(CmdResult::Output(output))
                }
            }

            Command::Checkout { node } => {
                self.graph.checkout(&node)?;
                Ok(CmdResult::Success(format!("HEAD is now at {}", node.0)))
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
