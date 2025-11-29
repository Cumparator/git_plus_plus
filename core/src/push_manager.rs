// core/src/push_manager.rs

use std::collections::{HashSet, VecDeque};
use std::error::Error;
// Предполагаем, что эти типы импортируются из types.rs
use crate::types::{NodeId, RemoteRef};
// Предполагаем, что RepoBackend и VersionGraph определены в других файлах core
use crate::backend::RepoBackend;
use crate::graph::VersionGraph;

/// Управляющий модуль для всех операций пуша.
/// Отвечает за вычисление набора нод для отправки и валидацию правил непрерывности.
pub struct PushManager {
    // VersionGraph для доступа к метаданным нод (родители, remotes).
    graph: VersionGraph,
    // RepoBackend для низкоуровневого взаимодействия с Git (чтение ссылок).
    backend: Box<dyn RepoBackend>,
}

impl PushManager {
    /// Вспомогательный метод: получает ID последней запушенной ноды для данного ремоута.
    ///
    /// Использует внутреннее соглашение Git++: `refs/gpp/remote/<name>`.
    ///
    /// # Возвращает
    /// `Ok(Some(NodeId))` если ссылка существует, `Ok(None)` в противном случае.
    fn get_remote_tip(&self, remote_name: &str) -> Result<Option<NodeId>, Box<dyn Error>> {
        // Конструируем внутреннюю Git-ссылку.
        let ref_name = format!("refs/gpp/remote/{}", remote_name);

        // Используем бэкенд для чтения ссылки.
        self.backend.read_ref(ref_name)
    }

    /// Проверяет, что все ноды от `start_id` до `end_id` (или корня)
    /// имеют разрешение на пуш для данного ремоута.
    ///
    /// Это реализует критически важное правило "непрерывной цепочки remotes".
    ///
    /// # Аргументы
    /// * `start_id` - Нода, с которой начинаем проверку (самая новая).
    /// * `remote` - Проверяемый удаленный репозиторий.
    /// * `end_id` - Опциональная нода, на которой нужно остановить проверку (удаленная вершина).
    fn validate_contiguous_remotes(
        &self,
        start_id: &NodeId,
        remote: &RemoteRef,
        end_id: Option<&NodeId>,
    ) -> Result<(), Box<dyn Error>> {
        let mut current_id = start_id.clone();

        loop {
            // Если текущая нода совпадает с удаленной вершиной, валидация успешна.
            if end_id.map_or(false, |tip| tip == &current_id) {
                return Ok(());
            }

            // Получаем ноду из графа.
            let current_node = self.graph.get_node(&current_id)
                .map_err(|e| format!("Не удалось получить ноду {}: {}", current_id.0, e))?;

            // 1. Проверяем наличие RemoteRef в наборе remotes текущей ноды.
            if !current_node.remotes.contains(remote) {
                return Err(format!(
                    "Нарушение непрерывности: Нода {} не разрешена для пуша на '{}'.",
                    current_id.0, remote.name
                ).into());
            }

            // 2. Если достигнут корень графа, и все ноды были разрешены, возвращаем успех.
            if current_node.parents.is_empty() {
                return Ok(());
            }

            // 3. Для проверки непрерывности переходим к первому родителю (как в 'git log').
            // NOTE: Для поддержки слияний, возможно, потребуется более сложная логика в будущем.
            current_id = current_node.parents[0].clone();
        }
    }

    /// Определяет минимальный набор нод, которые необходимо запушить.
    ///
    /// # Логика
    /// 1. Находит удаленную вершину (`remote_tip`).
    /// 2. Валидирует непрерывность разрешений на пуш до этой вершины.
    /// 3. Выполняет обход графа (BFS), собирая все ноды, которые находятся
    ///    между `node_id` (включительно) и `remote_tip` (исключая его предков).
    ///
    /// # Возвращает
    /// Вектор `NodeId` в топологическом порядке (от старых к новым).
    pub fn compute_nodes_to_push(
        &self,
        node_id: &NodeId,
        remote: &RemoteRef,
    ) -> Result<Vec<NodeId>, Box<dyn Error>> {
        // 1. Получаем удаленную вершину.
        let remote_tip = self.get_remote_tip(&remote.name)?;

        // 2. Проверяем непрерывность цепочки remotes.
        self.validate_contiguous_remotes(node_id, remote, remote_tip.as_ref())?;

        // Если локальная нода уже находится на ремоуте, пушить нечего.
        if remote_tip.as_ref().map_or(false, |tip| tip == node_id) {
            return Ok(vec![]);
        }

        // 3. Обход графа (BFS) для сбора разницы (nodes_to_push).
        let mut nodes_to_push: Vec<NodeId> = Vec::new();
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        queue.push_back(node_id.clone());
        visited.insert(node_id.clone());

        while let Some(current_id) = queue.pop_front() {
            // Если текущая нода является удаленной вершиной, останавливаем обход по этой ветке.
            if remote_tip.as_ref().map_or(false, |tip| tip == &current_id) {
                continue;
            }

            // Добавляем ноду в список для пуша.
            nodes_to_push.push(current_id.clone());

            let current_node = self.graph.get_node(&current_id)
                .map_err(|e| format!("Ошибка при обходе графа (get_node {}): {}", current_id.0, e))?;

            // Добавляем всех родителей в очередь для продолжения обхода.
            for parent_id in &current_node.parents {
                if visited.insert(parent_id.clone()) {
                    queue.push_back(parent_id.clone());
                }
            }
        }

        // 4. Инвертируем порядок. BFS обходит от новой к старой, но Git требует пуш от старой к новой.
        nodes_to_push.reverse();

        Ok(nodes_to_push)
    }
}