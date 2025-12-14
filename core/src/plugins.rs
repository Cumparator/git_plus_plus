use std::collections::HashMap;

// мы не знаем как делать плагинизацию помогите...
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