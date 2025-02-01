use crate::core::lib::{CommandExecutor, DiracError, DiracResult, AIProcessor};
use std::process::Command;
use std::env;
use which::which;
use std::sync::RwLock;
use tokio::process::Command as TokioCommand;

#[derive(Debug)]
pub struct ShellCommandExecutor {
    current_dir: RwLock<String>,
    shell_path: String,
    ai_processor: OllamaProcessor,
}

use crate::services::ai::OllamaProcessor;

impl ShellCommandExecutor {
    pub fn new() -> Self {
        let shell_path = env::var("SHELL").unwrap_or_else(|_| String::from("/bin/sh"));
        ShellCommandExecutor {
            current_dir: RwLock::new(
                env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::from("/"))
            ),
            shell_path,
            ai_processor: OllamaProcessor::with_default_config(),
        }
    }

    pub fn is_valid_command(&self, command: &str) -> bool {
        let first_word = command.split_whitespace().next().unwrap_or("");
        if first_word.is_empty() {
            return false;
        }
        if first_word == "cd" {
            return true;
        }
        which(first_word).is_ok()
    }

    pub fn get_current_dir(&self) -> String {
        self.current_dir.read().unwrap().clone()
    }

    fn handle_cd(&self, args: &str) -> DiracResult<String> {
        let path = args.trim();
        if path.is_empty() {
            return Err(DiracError::CommandExecutionError("No path specified for cd".to_string()));
        }

        let new_dir = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{}", self.current_dir.read().unwrap(), path)
        };

        match env::set_current_dir(&new_dir) {
            Ok(_) => {
                *self.current_dir.write().unwrap() = new_dir;
                Ok(String::new())
            }
            Err(e) => {
                let suggestion = match path {
                    "back" => "Use 'cd ..' to navigate to the parent directory.",
                    _ => {
                        if path.contains('/') {
                            "Make sure the directory exists and you have permission to access it."
                        } else {
                            "Use 'cd ..' to go up one directory or 'cd ~' to go to your home directory."
                        }
                    }
                };
                Err(DiracError::CommandExecutionError(format!("Failed to change directory: {}\n\nℹ️ Suggestion:\n{}", e, suggestion)))
            }
        }
    }
}

#[async_trait::async_trait]
impl CommandExecutor for ShellCommandExecutor {
    async fn execute(&self, command: &str) -> DiracResult<String> {
        if command.trim().is_empty() {
            return Err(DiracError::CommandExecutionError("Empty command provided".to_string()));
        }

        let parts: Vec<&str> = command.splitn(2, ' ').collect();
        let cmd = parts[0];
        let args = parts.get(1).unwrap_or(&"");

        if cmd == "cd" {
            return self.handle_cd(args);
        }

        // Update current directory from environment in case it was changed externally
        if let Ok(current_dir) = env::current_dir() {
            *self.current_dir.write().unwrap() = current_dir.to_string_lossy().to_string();
        }

        // Verify command exists before execution
        if !self.is_valid_command(cmd) {
            return Err(DiracError::CommandExecutionError(
                format!("Command '{}' not found or not executable", cmd)
            ));
        }

        // Use tokio's async Command for potentially long-running operations
        let output = if command.starts_with("lsof") || command.starts_with("netstat") {
            TokioCommand::new(&self.shell_path)
                .arg("-c")
                .arg(command)
                .output()
                .await
                .map_err(|e| DiracError::CommandExecutionError(e.to_string()))?
        } else {
            Command::new(&self.shell_path)
                .arg("-c")
                .arg(command)
                .env("ZDOTDIR", env::var("HOME").unwrap_or_else(|_| String::from("/")))
                .env("ZSH_HISTORY_FILE", ".zsh_history")
                .current_dir(self.current_dir.read().unwrap().as_str())
                .output()
                .map_err(|e| {
                    let error_msg = format!("Command execution error: {}", e);
                    let suggestion = match e.kind() {
                        std::io::ErrorKind::NotFound => {
                            format!("Command '{}' not found. Check if it's installed or try using natural language to describe what you want to do.", command)
                        },
                        std::io::ErrorKind::PermissionDenied => {
                            format!("Permission denied for command '{}'. Try using 'sudo' if you have the necessary permissions.", command)
                        },
                        _ => {
                            // Get AI suggestion for the failed command
                            let ai_suggestion = tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(self.ai_processor.process(
                                    &format!("Command failed: {}\nError: {}", command, e),
                                    "Please suggest a fix or alternative command"
                                ))
                                .unwrap_or_else(|_| String::from("Try 'help' for command usage or use natural language to describe what you want to do."));
                            format!("\n\nℹ️ Suggestion:\n{}\n", ai_suggestion)
                        }
                    };
                    DiracError::CommandExecutionError(format!("{}

{}", error_msg, suggestion))
                })?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Update current directory after command execution
        if let Ok(new_dir) = env::current_dir() {
            *self.current_dir.write().unwrap() = new_dir.to_string_lossy().to_string();
        }

        // Verify command execution status and provide detailed feedback
        if !output.status.success() || !stderr.is_empty() {
            let error_message = if !stderr.is_empty() { stderr } else { "Command failed".to_string() };
            let suggestion = if cmd == "cd" {
                match args.trim() {
                    "back" => "Use 'cd ..' to navigate to the parent directory.".to_string(),
                    _ => {
                        if args.contains('/') {
                            "Make sure the directory exists and you have permission to access it.".to_string()
                        } else {
                            "Use 'cd ..' to go up one directory or 'cd ~' to go to your home directory.".to_string()
                        }
                    }
                }
            } else {
                // Get AI suggestion for failed command with more context
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(self.ai_processor.process(
                        &format!("Command failed: {}\nError: {}\nCurrent directory: {}", 
                            command, error_message, self.get_current_dir()),
                        "Please analyze this error and suggest a fix or alternative command. Consider the current directory context."
                    ))
                    .unwrap_or_else(|_| "Try rephrasing your command or check the syntax.".to_string())
            };
            return Err(DiracError::CommandExecutionError(format!("{}

ℹ️ Suggestion:
{}", error_message, suggestion)));
        }

        Ok(stdout)
    }

    fn get_ai_suggestion(&self, failed_command: &str) -> DiracResult<String> {
        // This is a placeholder for AI suggestion implementation
        // In a real implementation, this would call the AI service to get suggestions
        Ok(format!("Command '{}' failed. Consider checking the command syntax or try using natural language to describe what you want to do.", failed_command))
    }
}