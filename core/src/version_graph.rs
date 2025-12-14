use std::error::Error;
use std::collections::{HashSet, HashMap};
use chrono::Utc;

use crate::types::{Node, NodeId, Author, NodePayload, RemoteRef};
use crate::backend::{RepoBackend, GraphOps};
use crate::storage::GraphStorage;

pub struct VersionGraph {
    storage: Box<dyn GraphStorage>,
    backend: Box<dyn RepoBackend>,
}

impl VersionGraph {
    pub fn new(storage: Box<dyn GraphStorage>, backend: Box<dyn RepoBackend>) -> Self {
        Self { storage, backend }
    }

    pub fn add_node(
        &mut self,
        parents: Vec<NodeId>,
        author: Author,
        message: String,
        requested_remotes: Option<Vec<String>>,
    ) -> Result<NodeId, Box<dyn Error>> {

        // Собираем все допустимые ремоуты от родителей (Union)
        let mut allowed_remotes: HashMap<String, RemoteRef> = HashMap::new();

        for parent_id in &parents {
            let p_node = self.storage.load_node(parent_id)?;
            for remote in p_node.remotes {
                // надо проверять на конфликт URL, но пока пропустим.
                allowed_remotes.insert(remote.name.clone(), remote);
            }
        }

        // 2. Определяем итоговый список ремоутов для новой ноды
        let final_remotes: HashSet<RemoteRef> = if let Some(req_names) = requested_remotes {
            // Ветка А: Пользователь явно запросил конкретные ремоуты
            let mut result = HashSet::new();

            if parents.is_empty() {
                // Edge Case: Корневая нода (нет родителей).
                // Мы не можем валидировать "наследие".
                // Тут архитектурный вопрос: откуда брать URL для RemoteRef?
                // Для простоты, если это корень, разрешаем создавать RemoteRef без URL (или с пустым).
                for name in req_names {
                    result.insert(RemoteRef {
                        name,
                        url: "".to_string(), // TODO: Надо бы брать из git config
                        specs: Default::default(),
                    });
                }
            } else {
                // Стандартный случай: Валидация подмножества
                for name in req_names {
                    match allowed_remotes.get(&name) {
                        Some(r_ref) => {
                            result.insert(r_ref.clone());
                        },
                        None => {
                            return Err(format!(
                                "Validation Error: Remote '{}' is not present in parent nodes. \
                                Cannot extend history seamlessly. Parents have: {:?}",
                                name,
                                allowed_remotes.keys().collect::<Vec<_>>()
                            ).into());
                        }
                    }
                }
            }
            result
        } else {
            // Ветка Б: Пользователь ничего не указал -> Наследуем ВСЁ (Union)
            if parents.is_empty() {
                // Если корень и не указали ремоутов -> наверно "origin"?
                HashSet::from([RemoteRef {
                    name: "origin".into(),
                    url: "".into(),
                    specs: Default::default()
                }])
            } else {
                allowed_remotes.into_values().collect()
            }
        };

        // TODO: Здесь есть проблема. create_commit пишет в ТЕКУЩИЙ активный контекст.
        let tree_id = self.backend.create_tree()?;
        let commit_id = self.backend.create_commit(&tree_id, &parents, &message, &author)?;

        let node = Node {
            id: commit_id.clone(),
            parents: parents.clone(),
            children: HashSet::new(),
            author,
            message,
            created_at: Utc::now(),
            payload: NodePayload { tree_id },
            remotes: final_remotes,
            tags: HashMap::new(),
            metadata: HashMap::new(),
        };

        let tx = self.storage.begin_tx()?;
        self.storage.persist_node(&node)?;

        for parent_id in &parents {
            let mut p_node = self.storage.load_node(parent_id)?;
            p_node.children.insert(commit_id.clone());
            self.storage.persist_node(&p_node)?;
        }
        self.storage.commit_tx(tx)?;

        Ok(commit_id)
    }

    pub fn add_remote_permission(
        &mut self,
        node_id: &NodeId,
        remote: RemoteRef
    ) -> Result<(), Box<dyn Error>> {
        let tx = self.storage.begin_tx()?;

        let mut node = self.storage.load_node(node_id)?;
        node.add_remote(remote);
        self.storage.persist_node(&node)?;

        self.storage.commit_tx(tx)?;
        Ok(())
    }

    pub fn remove_remote_permission(
        &mut self,
        node_id: &NodeId,
        remote_name: &str
    ) -> Result<(), Box<dyn Error>> {
        let tx = self.storage.begin_tx()?;

        let mut node = self.storage.load_node(node_id)?;
        node.remove_remote(remote_name);
        self.storage.persist_node(&node)?;

        self.storage.commit_tx(tx)?;
        Ok(())
    }

    pub fn checkout(&self, node_id: &NodeId) -> Result<(), Box<dyn Error>> {
        let node = self.storage.load_node(node_id)?;
        self.backend.checkout_node(&node)?;
        Ok(())
    }

    pub fn list_roots(&self) -> Result<Vec<NodeId>, Box<dyn Error>> {
        Ok(self.storage.list_roots()?)
    }
}

impl GraphOps for VersionGraph { // на кой хрен было вводить graphOps я не знаю, кто-нибудь мне объясните?
    fn get_node(&self, id: &NodeId) -> Result<Node, Box<dyn Error>> {
        Ok(self.storage.load_node(id)?)
    }
}