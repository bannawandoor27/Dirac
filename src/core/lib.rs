use std::error::Error;
use std::fmt;

#[async_trait::async_trait]
pub trait AIProcessor {
    async fn process<'a>(&'a self, input: &'a str, context: &'a str) -> DiracResult<String>;
}

pub trait CommandExecutor {
    async fn execute(&self, command: &str) -> DiracResult<String>;
}

pub trait TerminalInterface {
    fn read_line(&mut self, prompt: &str) -> DiracResult<String>;
    fn add_history(&mut self, line: &str);
    fn display_output(&self, output: &str);
    fn display_error(&self, error: &str);
}

pub trait Plugin: std::fmt::Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, input: &str) -> DiracResult<String>;
}

pub trait PluginManager {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>);
    fn get_plugin(&self, name: &str) -> Option<&Box<dyn Plugin>>;
    fn list_plugins(&self) -> Vec<(&str, &str)>;
}

#[derive(Debug)]
pub enum DiracError {
    CommandExecutionError(String),
    AIProcessingError(String),
    InputError(String),
}

impl fmt::Display for DiracError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DiracError::CommandExecutionError(msg) => write!(f, "Command execution error: {}", msg),
            DiracError::AIProcessingError(msg) => write!(f, "AI processing error: {}", msg),
            DiracError::InputError(msg) => write!(f, "Input error: {}", msg),
        }
    }
}

impl Error for DiracError {}

pub type DiracResult<T> = Result<T, DiracError>;