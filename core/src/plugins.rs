use std::collections::HashMap;

pub struct PluginManager {
    // В будущем здесь будут загруженные плагины
    loaded_plugins: HashMap<String, String>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            loaded_plugins: HashMap::new(),
        }
    }
}