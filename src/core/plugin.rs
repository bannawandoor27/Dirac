use crate::core::lib::{Plugin, PluginManager, DiracResult};

#[derive(Debug)]
pub struct DefaultPluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl DefaultPluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }
}

impl PluginManager for DefaultPluginManager {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    fn get_plugin(&self, name: &str) -> Option<&Box<dyn Plugin>> {
        self.plugins.iter().find(|p| p.name() == name)
    }

    fn list_plugins(&self) -> Vec<(&str, &str)> {
        self.plugins
            .iter()
            .map(|p| (p.name(), p.description()))
            .collect()
    }
}

// Example plugin implementation
#[derive(Debug)]
pub struct HistoryPlugin {
    history: Vec<String>,
}

impl HistoryPlugin {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }
}

impl Plugin for HistoryPlugin {
    fn name(&self) -> &str {
        "history"
    }

    fn description(&self) -> &str {
        "Manages command history and provides history-related commands"
    }

    fn execute(&self, input: &str) -> DiracResult<String> {
        match input {
            "history" => Ok(self.history.join("\n")),
            _ => Ok(String::new()),
        }
    }
}