use crate::core::lib::{CommandExecutor, DiracError, DiracResult};
use std::process::Command;
use std::env;
use which::which;
use std::cell::RefCell;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

#[derive(Debug)]
pub struct ShellCommandExecutor {
    current_dir: RefCell<String>,
    shell_path: String,
}

impl ShellCommandExecutor {
    pub fn new() -> Self {
        let shell_path = env::var("SHELL").unwrap_or_else(|_| String::from("/bin/sh"));
        ShellCommandExecutor {
            current_dir: RefCell::new(
                env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| String::from("/"))
            ),
            shell_path,
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
        self.current_dir.borrow().to_string()
    }

    fn handle_cd(&self, args: &str) -> DiracResult<String> {
        let path = args.trim();
        if path.is_empty() {
            return Err(DiracError::CommandExecutionError("No path specified for cd".to_string()));
        }

        let path = if path == "~" || path.starts_with("~/") {
            path.replacen("~", &env::var("HOME").unwrap_or_else(|_| String::from("/")), 1)
        } else {
            path.to_string()
        };

        let new_dir = if path.starts_with('/') {
            path
        } else {
            format!("{}/{}", self.current_dir.borrow(), path)
        };

        // Resolve the path to handle . and .. components and symbolic links
        let _canonical_path = std::fs::canonicalize(&new_dir)
            .map_err(|e| DiracError::CommandExecutionError(format!("Invalid path: {}", e)))?;

        match env::set_current_dir(&new_dir) {
            Ok(_) => {
                *self.current_dir.borrow_mut() = new_dir;
                Ok(String::new())
            }
            Err(e) => Err(DiracError::CommandExecutionError(format!("Failed to change directory: {}", e)))
        }
    }
}

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
            *self.current_dir.borrow_mut() = current_dir.to_string_lossy().to_string();
        }

        // Set a reasonable timeout for commands
        let timeout_duration = Duration::from_secs(30);

        // Use tokio's async Command for all commands to ensure proper output handling
        let output = TokioCommand::new(&self.shell_path)
            .arg("-c")
            .arg(command)
            .env("TERM", "xterm-256color")
            .env("SHELL", &self.shell_path)
            .env("PATH", env::var("PATH").unwrap_or_else(|_| String::from("/usr/local/bin:/usr/bin:/bin")))
            .current_dir(&*self.current_dir.borrow())
            .output()
            .await
            .map_err(|e| DiracError::CommandExecutionError(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Update current directory after command execution
        if let Ok(new_dir) = env::current_dir() {
            *self.current_dir.borrow_mut() = new_dir.to_string_lossy().to_string();
        }

        // Handle command execution status and output
        if output.status.success() {
            if !stdout.is_empty() {
                Ok(stdout)
            } else if !stderr.is_empty() {
                Ok(stderr)
            } else {
                Ok(String::new())
            }
        } else {
            Err(DiracError::CommandExecutionError(
                format!("Command failed (exit code: {}): {}", 
                    output.status.code().unwrap_or(-1),
                    if !stderr.is_empty() { stderr.trim() } else { stdout.trim() }
                )
            ))
        }
    }
}