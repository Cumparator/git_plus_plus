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