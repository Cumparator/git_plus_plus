use std::process::Output;
use std::error::Error;
use crate::Node;
use crate::types::{NodeId, RemoteRef, Author};


pub trait RepoBackend {
    // по-идее, от этого надо будет избавиться, потому что любые runcmd нужные для git должен делать сам RepoBackend
    fn run_cmd(&self, cmd: &str, args: Vec<&str>) -> Result<Output, Box<dyn Error>>;

    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>>;

    fn create_tree(&self) -> Result<String, Box<dyn Error>>;

    fn create_commit(
        &self,
        tree_oid: &str,
        parents: &[NodeId],
        message: &str,
        author: &Author
    ) -> Result<NodeId, Box<dyn Error>>;

    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>>;

    // это тоже должен бы проверять сам RepoBackend...
    fn is_repo_empty(&self) -> Result<bool, Box<dyn Error>>; // костыль порожденный необходимостью иметь че-нибудь в гит для коммита

    fn checkout_node(&self, node: &Node) -> Result<(), Box<dyn Error>>;
}

/// Трейт для получения данных ноды из графа.
pub trait GraphOps {
    fn get_node(&self, id: &NodeId) -> Result<Node, Box<dyn Error>>;
}