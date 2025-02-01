use crate::core::lib::{CommandExecutor, DiracError, DiracResult};
use std::process::Command;
use std::env;
use which::which;
use std::cell::RefCell;

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

        let new_dir = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{}", self.current_dir.borrow(), path)
        };

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
    fn execute(&self, command: &str) -> DiracResult<String> {
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

        let output = Command::new(&self.shell_path)
            .arg("-c")
            .arg(command)
            .env("ZDOTDIR", env::var("HOME").unwrap_or_else(|_| String::from("/")))
            .env("ZSH_HISTORY_FILE", ".zsh_history")
            .current_dir(&*self.current_dir.borrow())
            .output()
            .map_err(|e| DiracError::CommandExecutionError(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Update current directory after command execution
        if let Ok(new_dir) = env::current_dir() {
            *self.current_dir.borrow_mut() = new_dir.to_string_lossy().to_string();
        }

        if !output.status.success() {
            return Err(DiracError::CommandExecutionError(format!("Command failed: {}", stderr)));
        }

        if !stderr.is_empty() {
            Err(DiracError::CommandExecutionError(stderr))
        } else {
            Ok(stdout)
        }
    }
}