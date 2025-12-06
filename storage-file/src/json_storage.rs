use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use gpp_core::types::{Node, NodeId};
use gpp_core::storage::{GraphStorage, TxHandle, StorageError, Result};

pub struct JsonStorage {
    db_path: PathBuf,
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
}

impl JsonStorage {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let path = db_path.as_ref().to_path_buf();
        let nodes = if path.exists() {
            let file = File::open(&path).map_err(StorageError::Io)?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).map_err(StorageError::Serde)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            db_path: path,
            nodes: Arc::new(RwLock::new(nodes)),
        })
    }
}

impl GraphStorage for JsonStorage {
    fn persist_node(&mut self, node: &Node) -> Result<()> {
        let mut map = self.nodes.write().map_err(|_| StorageError::Tx("Lock poisoned".into()))?;
        map.insert(node.id.clone(), node.clone());
        Ok(())
    }

    fn load_node(&self, id: &NodeId) -> Result<Node> {
        let map = self.nodes.read().map_err(|_| StorageError::Tx("Lock poisoned".into()))?;
        map.get(id)
            .cloned()
            .ok_or_else(|| StorageError::NodeNotFound(id.clone()))
    }

    fn list_roots(&self) -> Result<Vec<NodeId>> {
        let map = self.nodes.read().map_err(|_| StorageError::Tx("Lock poisoned".into()))?;
        Ok(map.values()
            .filter(|n| n.parents.is_empty())
            .map(|n| n.id.clone())
            .collect())
    }

    fn begin_tx(&self) -> Result<TxHandle> {
        Ok(TxHandle {
            path: self.db_path.clone(),
        })
    }

    fn commit_tx(&self, _tx: TxHandle) -> Result<()> {
        let map = self.nodes.read().map_err(|_| StorageError::Tx("Lock poisoned".into()))?;

        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }

        let file = File::create(&self.db_path).map_err(StorageError::Io)?;
        let writer = BufWriter::new(file);

        serde_json::to_writer_pretty(writer, &*map).map_err(StorageError::Serde)?;
        Ok(())
    }

    fn rollback_tx(&self, _tx: TxHandle) -> Result<()> {
        let mut map = self.nodes.write().map_err(|_| StorageError::Tx("Lock poisoned".into()))?;

        if self.db_path.exists() {
            let file = File::open(&self.db_path).map_err(StorageError::Io)?;
            let reader = BufReader::new(file);
            *map = serde_json::from_reader(reader).map_err(StorageError::Serde)?;
        } else {
            map.clear();
        }
        Ok(())
    }
}