use std::process::Output;
use std::error::Error;
use crate::types::{NodeId, RemoteRef};
use crate::types::Node; // Предположим, что Node тоже нужен

/// Трейт RepoBackend: низкоуровневое взаимодействие с системой контроля версий
pub trait RepoBackend {
    fn run_cmd(&self, cmd: &str, args: Vec<&str>) -> Result<Output, Box<dyn Error>>;
    fn read_ref(&self, refname: String) -> Result<Option<NodeId>, Box<dyn Error>>;
    // ...
    fn push_update_ref(
        &self,
        remote: &RemoteRef,
        local_tip_id: &NodeId,
        remote_target_ref: &str
    ) -> Result<(), Box<dyn Error>>;
}

pub trait GraphOps {
    fn get_node(&self, id: &NodeId) -> Result<Node, Box<dyn Error>>;
}