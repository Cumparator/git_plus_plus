use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;

use crate::types::{NodeId, RemoteRef};
use crate::backend::{RepoBackend, GraphOps};

#[derive(Debug)]
pub struct PushError(String);

impl fmt::Display for PushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Push Error: {}", self.0)
    }
}

impl Error for PushError {}

pub struct PushManager<'a> {
    graph: &'a dyn GraphOps,
    backend: &'a dyn RepoBackend,
}

impl<'a> PushManager<'a> {
    
    pub fn new(graph: &'a dyn GraphOps, backend: &'a dyn RepoBackend) -> Self {
        Self { graph, backend }
    }
    
    fn compute_nodes_to_push(
        &self,
        start_node: &NodeId,
        remote: &RemoteRef,
        remote_head: Option<&NodeId>,
    ) -> Result<Vec<NodeId>, Box<dyn Error>> {
        let mut to_push = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(start_node.clone());
        visited.insert(start_node.clone());

        while let Some(current_id) = queue.pop_front() {
            if let Some(head) = remote_head {
                if &current_id == head {
                    continue;
                }
            }

            let node = self.graph.get_node(&current_id)?;

            if !node.remotes.contains(remote) {
                return Err(Box::new(PushError(format!(
                    "Node {:?} does not allow pushing to remote '{}'",
                    current_id, remote.name
                ))));
            }

            to_push.push(current_id.clone());

            for parent_id in node.parents {
                if !visited.contains(&parent_id) {
                    visited.insert(parent_id.clone());
                    queue.push_back(parent_id);
                }
            }
        }

        Ok(to_push)
    }
    
    pub fn push(
        &self,
        node_id: &NodeId,
        remote: &RemoteRef,
        dry_run: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let remote_branch = "main";
        let remote_ref_name = format!("refs/heads/{}", remote_branch);

        let cached_remote_ref = format!("refs/remotes/{}/{}", remote.name, remote_branch);
        let remote_head = self.backend.read_ref(cached_remote_ref)?;

        let nodes_to_push = self.compute_nodes_to_push(node_id, remote, remote_head.as_ref())?;

        if nodes_to_push.is_empty() {
            println!("Все ноды до {:?} уже находятся на удаленном репозитории '{}'.", node_id, remote.name);
            return Ok(false);
        }

        if dry_run {
            println!("--- DRY RUN: Селективный Пуш ---");
            println!("  Удаленный репозиторий: '{}' ({})", remote.name, remote.url);
            println!("  Будет отправлено {} новых нод.", nodes_to_push.len());
            println!("  Целевая Git-ссылка: {}", remote_ref_name);
            println!("  Новая вершина: {:?}", node_id);
            println!("---------------------------------");
            return Ok(true);
        }

        println!("Отправка {} нод на '{}'...", nodes_to_push.len(), remote.name);

        self.backend.push_update_ref(remote, node_id, &remote_ref_name)?;

        println!("Успешно обновлена ссылка {} -> {:?}", remote_ref_name, node_id);

        Ok(true)
    }
}