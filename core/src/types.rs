// git_plus_plus/core/src/types.rs

use serde::{Serialize, Deserialize};
// Используем HashMap и HashSet для эффективного хранения данных.
use std::collections::{HashMap, HashSet};
// Импортируем типы DateTime и Utc для работы со временем.
use chrono::{DateTime, Utc};
use std::hash::{Hash, Hasher}; // Требуется для ручной реализации Hash

// =========================================================================
// I. БАЗОВЫЕ АЛИАСЫ И ВСПОМОГАТЕЛЬНЫЕ СТРУКТУРЫ
// =========================================================================

/// Уникальный идентификатор ноды (NodeId), по сути, Git-хэш коммита.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Идентификатор коммита (CommitId).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

/// Информация об авторе ноды.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// Полезная нагрузка ноды (ссылка на состояние рабочей директории в Git-дереве).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePayload {
    pub tree_id: String,
}

// =========================================================================
// II. СТРУКТУРЫ ГРАФА
// =========================================================================

/// Представляет удаленный репозиторий.
/// Не может использовать стандартный derive(Hash), так как содержит HashMap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRef {
    pub name: String,
    pub url: String,
    // Спецификации для пуша/фетча. Не участвуют в хешировании.
    pub specs: HashMap<String, String>,
}

// Ручная реализация трейтов для использования RemoteRef в HashSet (см. Node::remotes)
impl PartialEq for RemoteRef {
    /// Сравнение RemoteRef только по имени и URL.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.url == other.url
    }
}
impl Eq for RemoteRef {}

impl Hash for RemoteRef {
    /// Хеширование RemoteRef только по имени и URL.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.url.hash(state);
    }
}


/// Структура Тега.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub meta: HashMap<String,String>,
}

// =========================================================================
// III. CORE STRUCTURE: NODE
// =========================================================================

/// Главная структура: Нода в графе версий Git++.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    // Список родительских нод.
    pub parents: Vec<NodeId>,
    // Список дочерних нод (HashSet для эффективного добавления/удаления).
    pub children: HashSet<NodeId>,

    // Метаданные ноды
    pub author: Author,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub payload: NodePayload,

    // Контроль Селективного Пуша: Набор разрешенных удаленных репозиториев.
    pub remotes: HashSet<RemoteRef>,

    // Теги, ассоциированные с нодой (HashMap для быстрого поиска по имени).
    pub tags: HashMap<String, Tag>,

    // Дополнительные метаданные
    pub metadata: HashMap<String, String>,
}

// Методы управления нодой.
impl Node {
    /// Добавляет разрешение на пуш для указанного удаленного репозитория.
    pub fn add_remote(&mut self, remote: RemoteRef) {
        // Используем HashSet::insert для гарантии уникальности.
        self.remotes.insert(remote);
    }

    /// Удаляет разрешение на пуш для удаленного репозитория по его имени.
    pub fn remove_remote(&mut self, remote_name: &str) {
        // Используем retain, чтобы удалить RemoteRef, имя которого совпадает.
        self.remotes.retain(|r| r.name != remote_name);
    }

    /// Удаляет все разрешения на пуш, делая ноду и ее потомков "непушабельными".
    pub fn remove_all_remotes(&mut self) {
        self.remotes.clear();
    }

    /// Добавляет новый тег к ноде.
    pub fn add_tag(&mut self, tag: Tag) {
        let tag_name = tag.name.clone();
        // Используем имя тега как ключ для предотвращения дубликатов.
        self.tags.insert(tag_name, tag);
    }

    /// Удаляет тег из ноды по его имени.
    pub fn remove_tag(&mut self, tag_name: &str) {
        self.tags.remove(tag_name);
    }
}