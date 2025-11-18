use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parents: Vec<NodeId>,
    pub children: Vec<NodeId>,
    pub tags: Vec<Tag>,
    pub metadata: HashMap<String, String>,
}
