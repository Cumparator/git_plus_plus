use core::{
    GraphStorage, Result, StorageError,
    Node, NodeId, TxHandle
};
use serde::{Serialize, Deserialize};
use std::{collections::HashMap, fs, path::{Path, PathBuf}};

const GRAPH_FILE: &str = "graph.json";

#[derive(Debug, Serialize, Deserialize)]
struct GraphData {
    nodes: HashMap<String, Node>,
}

pub struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        fs::create_dir_all(path.as_ref()).unwrap();
        Self { root: path.as_ref().to_path_buf() }
    }

    fn graph_path(&self) -> PathBuf {
        self.root.join(GRAPH_FILE)
    }

    fn load_all(&self) -> Result<GraphData> {
        let path = self.graph_path();
        if !path.exists() {
            return Ok(GraphData { nodes: HashMap::new() });
        }
        let data = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&data)?)
    }

    fn save_all(&self, graph: &GraphData) -> Result<()> {
        let tmp = self.root.join("graph.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(graph)?)?;
        fs::rename(tmp, self.graph_path())?;
        Ok(())
    }
}

impl GraphStorage for FileStorage {
    fn persist_node(&mut self, node: &Node) -> Result<()> {
        let mut graph = self.load_all()?;
        graph.nodes.insert(node.id.0.clone(), node.clone());
        self.save_all(&graph)?;
        Ok(())
    }

    fn load_node(&self, id: &NodeId) -> Result<Node> {
        let graph = self.load_all()?;
        graph.nodes
            .get(&id.0)
            .cloned()
            .ok_or(StorageError::NodeNotFound(id.clone()))
    }

    fn list_roots(&self) -> Result<Vec<NodeId>> {
        let graph = self.load_all()?;
        let mut roots = vec![];

        for (id, node) in &graph.nodes {
            if node.parents.is_empty() {
                roots.push(NodeId(id.clone()));
            }
        }

        Ok(roots)
    }

    fn begin_tx(&self) -> Result<TxHandle> {
        let tx_path = self.root.join("graph.tx");
        fs::copy(self.graph_path(), &tx_path)?;
        Ok(TxHandle { path: tx_path })
    }

    fn commit_tx(&self, tx: TxHandle) -> Result<()> {
        let final_path = self.graph_path();
        fs::rename(tx.path, final_path)?;
        Ok(())
    }

    fn rollback_tx(&self, tx: TxHandle) -> Result<()> {
        if tx.path.exists() {
            fs::remove_file(tx.path)?;
        }
        Ok(())
    }
}
