use crate::core::lib::{AIProcessor, DiracError, DiracResult};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug)]
pub struct OllamaProcessor {
    client: Client,
    model: String,
    api_url: String,
}

impl OllamaProcessor {
    pub fn new(model: impl Into<String>, api_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            api_url: api_url.into(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(
            "qwen2.5:3b",
            "http://localhost:11434/api/generate",
        )
    }
}

#[async_trait::async_trait]
impl AIProcessor for OllamaProcessor {
    async fn process<'a>(&'a self, input: &'a str, context: &'a str) -> DiracResult<String> {
        let response = self
            .client
            .post(&self.api_url)
            .json(&json!({
                "model": self.model,
                "prompt": format!("You are a helpful terminal command assistant. Given this request: '{}' and context: '{}', generate a terminal command that would help accomplish this task.\n\nCurrent Environment:\n- Working Directory: {}\n- OS Type: {}\n- Directory Structure:\n{}\n\nFollow these rules:\n1. For navigation commands like 'go to [dir]', 'open [dir]', 'change to [dir]', convert them to appropriate 'cd' commands\n2. If the input is ambiguous (like single letters 'l'), suggest the most common command (like 'ls') and explain alternatives\n3. If the input looks like a typo (e.g. 'srcc' instead of 'src'), suggest the correction\n4. For failed commands, provide helpful suggestions and alternatives\n5. Include examples when relevant\n6. Consider the current directory structure and OS when suggesting commands\n7. ALWAYS provide both a command and an explanation\n8. NEVER return an empty command\n9. If unsure, suggest a safe, basic command like 'ls' or 'pwd'\n10. For natural language navigation (e.g. 'go to', 'open directory'), always convert to proper cd commands\n\nYour response MUST be in this exact format:\nCOMMAND: <the exact command to run>\nEXPLANATION: <brief explanation of what the command does, including alternatives or corrections if applicable>", 
                    input, 
                    context,
                    std::env::current_dir().unwrap_or_default().display(),
                    if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
                    std::fs::read_dir(".").map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .map(|e| format!("  {}", e.file_name().to_string_lossy()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }).unwrap_or_default()),
                "stream": false
            }))
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    DiracError::AIProcessingError(
                        "Ollama service is not running. To install and start Ollama:\n".to_string() +
                        "1. Visit https://ollama.ai to download and install Ollama\n" +
                        "2. Start the Ollama service\n" +
                        "3. Run 'ollama pull qwen2.5:3b' to download the model"
                    )
                } else if e.is_timeout() {
                    DiracError::AIProcessingError("Connection to Ollama service timed out. Please check if the service is responding.".to_string())
                } else {
                    DiracError::AIProcessingError(format!("Failed to connect to AI service: {}", e))
                }
            })?;

        let text = response
            .text()
            .await
            .map_err(|e| DiracError::AIProcessingError(format!("Failed to read AI response: {}", e)))?;

        // Try to parse the response as JSON to check for error messages
        if let Ok(json_response) = serde_json::from_str::<Value>(&text) {
            if let Some(error) = json_response.get("error") {
                let error_msg = error.as_str().unwrap_or("Unknown error");
                if error_msg.contains("model") {
                    return Err(DiracError::AIProcessingError(
                        format!("Model '{}' not found. To install the model:\n", self.model) +
                        "1. Ensure Ollama is running\n" +
                        format!("2. Run 'ollama pull {}' to download the model", self.model).as_str()
                    ));
                }
                return Err(DiracError::AIProcessingError(format!("Ollama error: {}", error_msg)));
            }

            // Extract the response from the JSON
            if let Some(response) = json_response.get("response") {
                let response_text = response.as_str().unwrap_or("").trim();
                if response_text.is_empty() {
                    // Return a default command suggestion for empty responses
                    return Ok("COMMAND: ls\nEXPLANATION: Lists files and directories in the current directory. This is a safe default command when the request is unclear.".to_string());
                }

                // Parse the response format
                let mut command = String::new();
                let mut explanation = String::new();

                for line in response_text.lines() {
                    if line.starts_with("COMMAND:") {
                        command = line.trim_start_matches("COMMAND:").trim().to_string();
                    } else if line.starts_with("EXPLANATION:") {
                        explanation = line.trim_start_matches("EXPLANATION:").trim().to_string();
                    }
                }

                // Ensure we always return a valid command and explanation
                if command.is_empty() {
                    command = "ls".to_string();
                    explanation = if explanation.is_empty() {
                        "Lists files and directories in the current directory. This is a safe default command when the request is unclear.".to_string()
                    } else {
                        explanation
                    };
                } else if explanation.is_empty() {
                    explanation = "Executes the specified command.".to_string();
                }

                // Return both command and explanation in a format that can be parsed by the terminal
                return Ok(format!("COMMAND: {}\nEXPLANATION: {}", command, explanation));
            }
        }

        // Return a default command for any parsing failures
        Ok("COMMAND: ls\nEXPLANATION: Lists files and directories in the current directory. This is a safe default command when the request cannot be processed.".to_string())
    }
}