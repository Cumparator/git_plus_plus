use std::collections::HashMap;

pub struct PluginManager {
    loaded_plugins: HashMap<String, String>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            loaded_plugins: HashMap::new(),
        }
    }
}