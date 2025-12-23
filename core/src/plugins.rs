use std::collections::HashMap;
use crate::dispatcher::{CommandHandler};

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn create_handler(&self) -> Box<dyn CommandHandler>;
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    pub fn get_handler(&self, name: &str) -> Option<Box<dyn CommandHandler>> {
        self.plugins.get(name).map(|p| p.create_handler())
    }

    pub fn list_commands(&self) -> Vec<(String, String)> {
        self.plugins.iter()
            .map(|(k, v)| (k.clone(), v.description().to_string()))
            .collect()
    }
}