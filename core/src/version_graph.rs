use std::error::Error;
use std::collections::{HashSet, HashMap};
use chrono::Utc;

use crate::types::{Node, NodeId, Author, NodePayload, RemoteRef};
use crate::backend::{RepoBackend, GraphOps};
use crate::storage::GraphStorage;

/// Основной компонент бизнес-логики Git++.
/// Отвечает за управление графом версий, создание нод и координацию между хранилищем и Git-бэкендом.
pub struct VersionGraph {
    storage: Box<dyn GraphStorage>,
    backend: Box<dyn RepoBackend>,
}

impl VersionGraph {
    /// Создает новый экземпляр графа версий.
    ///
    /// # Arguments
    ///
    /// * `storage` - Реализация постоянного хранилища метаданных графа (Box<dyn GraphStorage>).
    /// * `backend` - Реализация низкоуровневого доступа к Git.
    pub fn new(storage: Box<dyn GraphStorage>, backend: Box<dyn RepoBackend>) -> Self {
        Self { storage, backend }
    }

    /// Создает новую ноду в графе на основе текущего состояния рабочей директории.
    ///
    /// Метод выполняет следующие действия:
    /// 1. Создает снимок (snapshot) рабочей директории (Git Tree).
    /// 2. Создает физический Git-коммит.
    /// 3. Вычисляет наследуемые права на пуш (`remotes`) от родителей.
    /// 4. Формирует структуру `Node` и сохраняет её в хранилище.
    /// 5. Обновляет ссылки `children` у родительских нод.
    ///
    /// # Arguments
    ///
    /// * `parents` - Список идентификаторов родительских нод.
    /// * `author` - Данные автора изменения.
    /// * `message` - Сообщение коммита (описание изменений).
    ///
    /// # Returns
    ///
    /// Возвращает `NodeId` только что созданной ноды.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если:
    /// * Не удалось выполнить Git-операции (write-tree, commit-tree).
    /// * Не удалось записать данные в `GraphStorage`.
    /// * Указанный родитель не найден в хранилище.
    pub fn add_node(
        &mut self,
        parents: Vec<NodeId>,
        author: Author,
        message: String,
    ) -> Result<NodeId, Box<dyn Error>> {
        let tree_id = self.backend.create_tree()?;

        let commit_id = self.backend.create_commit(&tree_id, &parents, &message, &author)?;

        let inherited_remotes = if let Some(first_parent_id) = parents.first() {
            let parent_node = self.storage.load_node(first_parent_id)?;
            parent_node.remotes
        } else {
            HashSet::new()
        };

        let node = Node {
            id: commit_id.clone(),
            parents: parents.clone(),
            children: HashSet::new(),
            author,
            message,
            created_at: Utc::now(),
            payload: NodePayload { tree_id },
            remotes: inherited_remotes,
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

    /// Добавляет разрешение на пуш (RemoteRef) для указанной ноды.
    /// Используется командой `chrm` (Change Remote).
    ///
    /// # Arguments
    ///
    /// * `node_id` - Идентификатор ноды.
    /// * `remote` - Описание удаленного репозитория.
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

    /// Удаляет разрешение на пуш для указанной ноды по имени репозитория.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Идентификатор ноды.
    /// * `remote_name` - Имя удаленного репозитория (например, "origin").
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
        self.backend.checkout_tree(&node.payload.tree_id)?;
        Ok(())
    }
}

impl GraphOps for VersionGraph {
    /// Получает ноду из подключенного хранилища.
    /// Используется внешними модулями (например, PushManager) для анализа графа.
    fn get_node(&self, id: &NodeId) -> Result<Node, Box<dyn Error>> {
        Ok(self.storage.load_node(id)?)
    }
}