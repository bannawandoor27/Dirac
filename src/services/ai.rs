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
        let current_dir = std::env::current_dir().unwrap_or_default().display().to_string();
        let os_type = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        let directory_structure = std::fs::read_dir(".").map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| format!("  {}", e.file_name().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("\n")
        }).unwrap_or_default();

        // Improved system prompt:
        let prompt = format!(
            "You are a sophisticated terminal command generator that converts natural language requests into precise, executable shell commands.
When provided with a user request and additional context, you must:
  
1. **Ensure Accuracy and Safety**:
   - Generate commands that can be executed directly without any modifications.
   - Prioritize safe, non-destructive commands (e.g., 'ls', 'pwd') when the request is ambiguous.
   - Convert natural language navigation requests (such as 'go to', 'open', 'change to') into the appropriate 'cd' commands.

2. **Detect and Correct Typos**:
   - Identify any typographical errors (for example, convert 'lsbkk' to 'ls').
   - If a correction is made or multiple interpretations are possible, include clear guidance in the explanation.

3. **Leverage Context**:
   - Use the provided details about the current working directory, operating system, and directory structure to tailor your response.
   - Ensure that any suggested navigation or file-related commands reflect the actual environment.

4. **Follow the Strict Response Format**:
   - Your answer must be in the exact format shown below with no extra text:
     
     COMMAND: <the exact command to execute>
     EXPLANATION: <a concise explanation of the command, including any corrections or alternative suggestions>

**Input Details**:
- User Request: '{}'
- Additional Context: '{}'
- Current Environment:
   - Working Directory: {}
   - OS Type: {}
   - Directory Structure:
{}

Based on these details, generate the appropriate terminal command and a brief explanation.",
            input,
            context,
            current_dir,
            os_type,
            directory_structure
        );

        let response = self
            .client
            .post(&self.api_url)
            .json(&json!({
                "model": self.model,
                "prompt": prompt,
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
