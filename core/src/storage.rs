use crate::{Node, NodeId};
use thiserror::Error;

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

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone)]
pub struct TxHandle {
    pub path: std::path::PathBuf,
}

pub trait GraphStorage {
    fn persist_node(&mut self, node: &Node) -> Result<()>;
    fn load_node(&self, id: &NodeId) -> Result<Node>;
    fn list_roots(&self) -> Result<Vec<NodeId>>;

    fn begin_tx(&self) -> Result<TxHandle>;
    fn commit_tx(&self, tx: TxHandle) -> Result<()>;
    fn rollback_tx(&self, tx: TxHandle) -> Result<()>;
}
