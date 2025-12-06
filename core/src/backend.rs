use std::process::Output;
use std::error::Error;
use crate::types::{NodeId, RemoteRef, Author};

/// Трейт RepoBackend: низкоуровневое взаимодействие с системой контроля версий (VCS).
///
/// Предоставляет абстракцию над физическими операциями Git (или другой VCS),
/// позволяя работать с объектами (деревьями, коммитами) и ссылками.
pub trait RepoBackend {
    /// Выполняет произвольную команду VCS.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Имя команды (например, "git").
    /// * `args` - Список аргументов команды.
    ///
    /// # Returns
    ///
    /// Возвращает `Output` процесса или ошибку выполнения.
    fn run_cmd(&self, cmd: &str, args: Vec<&str>) -> Result<Output, Box<dyn Error>>;

    /// Читает текущий идентификатор (хэш) указанной ссылки.
    ///
    /// # Arguments
    ///
    /// * `refname` - Полное имя ссылки (например, "refs/heads/master" или "HEAD").
    ///
    /// # Returns
    ///
    /// * `Ok(Some(NodeId))` - Если ссылка существует.
    /// * `Ok(None)` - Если ссылка не найдена.
    /// * `Err` - В случае системной ошибки ввода-вывода.
    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>>;

    /// Создает объект дерева (tree object) из текущего состояния рабочей директории.
    /// Аналог `git write-tree` после добавления всех файлов в индекс.
    ///
    /// # Returns
    ///
    /// Возвращает строковый хэш созданного дерева (Tree OID).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если не удалось прочитать файлы или записать объект в БД Git.
    fn create_tree(&self) -> Result<String, Box<dyn Error>>;

    /// Создает объект коммита (commit object) на основе дерева и родителей.
    /// Аналог `git commit-tree`.
    ///
    /// # Arguments
    ///
    /// * `tree_oid` - Хэш дерева, полученный из `create_tree`.
    /// * `parents` - Список идентификаторов родительских нод.
    /// * `message` - Сообщение коммита.
    /// * `author` - Информация об авторе изменений.
    ///
    /// # Returns
    ///
    /// Возвращает `NodeId` созданного коммита.
    fn create_commit(
        &self,
        tree_oid: &str,
        parents: &[NodeId],
        message: &str,
        author: &Author
    ) -> Result<NodeId, Box<dyn Error>>;

    /// Обновляет ссылку на удаленном репозитории, выполняя push.
    ///
    /// # Arguments
    ///
    /// * `remote` - Структура, описывающая удаленный репозиторий (URL, имя).
    /// * `local_tip_id` - Локальный `NodeId`, который станет вершиной на удаленном сервере.
    /// * `remote_target_ref` - Имя ссылки на удаленном сервере, которую нужно обновить.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если сеть недоступна или сервер отклонил изменения.
    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>>;

    fn is_repo_empty(&self) -> Result<bool, Box<dyn Error>>; // костыль порожденный необходимостью иметь че-нибудь в гит для коммита
}

/// Трейт для получения данных ноды из графа (ReadOnly операции).
pub trait GraphOps {
    /// Загружает данные ноды по её идентификатору.
    ///
    /// # Arguments
    ///
    /// * `id` - Идентификатор ноды.
    ///
    /// # Returns
    ///
    /// Возвращает полные данные ноды `Node`.
    fn get_node(&self, id: &NodeId) -> Result<crate::types::Node, Box<dyn Error>>;
}