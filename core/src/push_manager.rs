// core/src/push_manager.rs

use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;

// --- ЗАГЛУШКИ ДЛЯ КОМПИЛЯЦИИ (В реальном проекте это будут импорты) ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteRef {
    pub name: String,
    pub url: String,
}

// Минимальная структура Node
pub struct Node {
    pub parents: Vec<NodeId>,
    pub remotes: HashSet<RemoteRef>,
}

// Минимальный трейт GraphOps
pub trait GraphOps {
    fn get_node(&self, id: &NodeId) -> Result<Node, Box<dyn Error>>;
}

// Минимальная структура VersionGraph
pub struct VersionGraph;
impl GraphOps for VersionGraph {
    fn get_node(&self, _id: &NodeId) -> Result<Node, Box<dyn Error>> {
        Err("Метод VersionGraph::get_node не реализован".into())
    }
}


/// Расширенный трейт RepoBackend (добавлен метод для пуша)
pub trait RepoBackend {
    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>>;

    /// Выполняет пуш всех Git-объектов, необходимых для достижения `local_tip_id`,
    /// и обновляет удаленную ссылку `remote_target_ref` на `local_tip_id`.
    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>>;
}

// --- КОНЕЦ ЗАГЛУШЕК И ТРЕЙТОВ ---


/// Управляющий модуль для всех операций пуша.
pub struct PushManager {
    graph: VersionGraph,
    backend: Box<dyn RepoBackend>,
}

impl PushManager {
    /// Выполняет операцию селективного пуша.
    ///
    /// # Аргументы
    /// * `node_id` - Нода, которую мы хотим сделать вершиной удаленного репозитория.
    /// * `remote` - Удаленный репозиторий для пуша.
    /// * `dry_run` - Если true, только вычисляет и выводит, что будет запушено.
    ///
    /// # Возвращает
    /// `Ok(true)`, если пуш был выполнен или симулирован; `Ok(false)`, если пушить нечего.
    pub fn push(
        &self,
        node_id: &NodeId,
        remote: &RemoteRef,
        dry_run: bool,
    ) -> Result<bool, Box<dyn Error>> {
        // 1. Вычисляем набор нод для пуша.
        let nodes_to_push = self.compute_nodes_to_push(node_id, remote)?;

        // Если список пуст, значит, целевая нода уже на удаленном репозитории.
        if nodes_to_push.is_empty() {
            println!("Node {:?} уже присутствует на удаленном репозитории '{}'. Пуш не требуется.", node_id, remote.name);
            return Ok(false);
        }

        // 2. Подготавливаем метаданные для пуша.
        let count = nodes_to_push.len();
        // Внутренняя ссылка, которую мы будем обновлять на удаленном репозитории.
        let remote_ref_name = format!("refs/gpp/remote/{}", remote.name);

        // 3. Логика Dry Run.
        if dry_run {
            println!("--- DRY RUN: Селективный Пуш ---");
            println!("  Удаленный репозиторий: '{}' ({})", remote.name, remote.url);
            println!("  Будет запущено {} нод.", count);
            println!("  Целевая Git-ссылка: {}", remote_ref_name);
            println!("  Вершина пуша: {:?}", node_id);
            println!("  Начальная нода в цепочке: {:?}", nodes_to_push.first().unwrap());
            println!("---------------------------------");
            return Ok(true);
        }

        // 4. Фактический Пуш.

        println!("Пуш {} нод на удаленный репозиторий '{}'...", count, remote.name);

        // Делегируем низкоуровневую Git-работу бэкенду.
        self.backend.push_update_ref(remote, node_id, &remote_ref_name)?;

        println!("Успешный пуш. Удаленная ссылка {} обновлена до {:?}", remote_ref_name, node_id);

        // NOTE: Здесь можно добавить логику плагинов (PluginManager) для post-push хуков.

        Ok(true)
    }
}