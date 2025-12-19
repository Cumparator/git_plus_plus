use std::collections::HashMap;
use crate::dispatcher::{CommandHandler};

/// Трейт, который описывает плагин
pub trait Plugin: Send + Sync {
    /// Имя команды, которую добавляет плагин (например, "stats")
    fn name(&self) -> &str;
    /// Описание для help
    fn description(&self) -> &str;
    /// Создает обработчик команды
    fn create_handler(&self) -> Box<dyn CommandHandler>;
}

pub struct PluginManager {
    /// Реестр плагинов: имя команды -> сам плагин
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Регистрация нового плагина
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    /// Получение обработчика по имени команды
    pub fn get_handler(&self, name: &str) -> Option<Box<dyn CommandHandler>> {
        self.plugins.get(name).map(|p| p.create_handler())
    }

    /// Список команд для генерации справки
    pub fn list_commands(&self) -> Vec<(String, String)> {
        self.plugins.iter()
            .map(|(k, v)| (k.clone(), v.description().to_string()))
            .collect()
    }
}