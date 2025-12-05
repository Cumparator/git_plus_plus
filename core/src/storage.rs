use crate::types::{Node, NodeId};
use thiserror::Error;

/// Ошибки, возникающие на уровне абстракции хранилища.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Unknown node {0:?}")]
    NodeNotFound(NodeId),

    #[error("Transaction error: {0}")]
    Tx(String),
}

/// Результат выполнения операций хранилища.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Дескриптор транзакции.
#[derive(Debug, Clone)]
pub struct TxHandle {
    /// Идентификатор или путь контекста транзакции.
    pub path: std::path::PathBuf,
}

/// Абстракция хранилища графа версий (Интерфейс).
/// Этот трейт будут реализовывать другие пакеты (например, storage-file).
pub trait GraphStorage {
    /// Сохраняет ноду.
    fn persist_node(&mut self, node: &Node) -> Result<()>;

    /// Загружает ноду по ID.
    fn load_node(&self, id: &NodeId) -> Result<Node>;

    /// Возвращает список корневых нод.
    fn list_roots(&self) -> Result<Vec<NodeId>>;

    /// Начинает транзакцию.
    fn begin_tx(&self) -> Result<TxHandle>;

    /// Фиксирует транзакцию.
    fn commit_tx(&self, tx: TxHandle) -> Result<()>;

    /// Откатывает транзакцию.
    fn rollback_tx(&self, tx: TxHandle) -> Result<()>;
}