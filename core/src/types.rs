use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePayload {
    pub tree_id: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    pub name: String,
    pub url: String,
    pub specs: HashMap<String, String>,
}

impl PartialEq for RemoteRef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.url == other.url
    }
}
impl Eq for RemoteRef {}

impl Hash for RemoteRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.url.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub meta: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,

    pub parents: Vec<NodeId>,

    pub children: HashSet<NodeId>,

    pub author: Author,

    pub message: String,

    pub created_at: DateTime<Utc>,

    pub payload: NodePayload,

    pub remotes: HashSet<RemoteRef>,

    pub tags: HashMap<String, Tag>,

    pub metadata: HashMap<String, String>,
}

impl Node {
    pub fn add_remote(&mut self, remote: RemoteRef) {
        self.remotes.insert(remote);
    }

    pub fn remove_remote(&mut self, remote_name: &str) {
        self.remotes.retain(|r| r.name != remote_name);
    }

    pub fn remove_all_remotes(&mut self) {
        self.remotes.clear();
    }

    pub fn add_tag(&mut self, tag: Tag) {
        self.tags.insert(tag.name.clone(), tag);
    }

    pub fn remove_tag(&mut self, tag_name: &str) {
        self.tags.remove(tag_name);
    }
}