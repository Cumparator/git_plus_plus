use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use chrono::{DateTime, Utc};

/// Уникальный идентификатор ноды (NodeId).
/// Обычно это SHA-1 хэш Git-коммита.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Идентификатор коммита (CommitId).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

/// Информация об авторе изменений.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// Полезная нагрузка ноды (ссылка на дерево Git).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePayload {
    /// Хэш объекта дерева (Git Tree ID).
    pub tree_id: String,
}


/// Описание удаленного репозитория.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    pub name: String,
    pub url: String,
    pub specs: HashMap<String, String>,
}

/// Реализация сравнения для RemoteRef (игнорируем specs)
impl PartialEq for RemoteRef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.url == other.url
    }
}
impl Eq for RemoteRef {}

/// Реализация хеширования для RemoteRef (для использования в HashSet)
impl Hash for RemoteRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.url.hash(state);
    }
}

/// Тег (метка версии).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub meta: HashMap<String, String>,
}

/// Нода графа версий.
///
/// Это основная сущность Git++. Она хранит связи с родителями и детьми,
/// метаданные и настройки селективного пуша (remotes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Уникальный ID ноды.
    pub id: NodeId,

    /// Список родителей (от кого произошла эта нода).
    pub parents: Vec<NodeId>,

    /// Список детей (кто произошел от этой ноды).
    pub children: HashSet<NodeId>,

    /// Автор изменений.
    pub author: Author,

    /// Сообщение коммита.
    pub message: String,

    /// Дата создания.
    pub created_at: DateTime<Utc>,

    /// Ссылка на контент (дерево файлов).
    pub payload: NodePayload,

    /// **Селективный пуш**: список репозиториев, куда разрешено отправлять эту ноду.
    pub remotes: HashSet<RemoteRef>,

    /// Теги этой ноды.
    pub tags: HashMap<String, Tag>,

    /// Дополнительные данные.
    pub metadata: HashMap<String, String>,
}

impl Node {
    /// Добавляет разрешение на пуш в указанный remote.
    pub fn add_remote(&mut self, remote: RemoteRef) {
        self.remotes.insert(remote);
    }

    /// Удаляет разрешение на пуш для remote по имени.
    pub fn remove_remote(&mut self, remote_name: &str) {
        self.remotes.retain(|r| r.name != remote_name);
    }

    /// Удаляет все разрешения на пуш (делает ноду локальной).
    pub fn remove_all_remotes(&mut self) {
        self.remotes.clear();
    }

    /// Добавляет тег.
    pub fn add_tag(&mut self, tag: Tag) {
        self.tags.insert(tag.name.clone(), tag);
    }

    /// Удаляет тег.
    pub fn remove_tag(&mut self, tag_name: &str) {
        self.tags.remove(tag_name);
    }
}