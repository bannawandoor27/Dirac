use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, EditMode};
use rustyline::completion::{FilenameCompleter, Completer, Pair};
use rustyline::validate::{MatchingBracketValidator, Validator};
use rustyline::highlight::{MatchingBracketHighlighter, Highlighter};
use rustyline::hint::{HistoryHinter, Hinter};
use std::borrow::Cow;
use rustyline::history::DefaultHistory;
use std::io::Write;

pub struct DiracCompleter {
    filename_completer: FilenameCompleter,
    command_history: Vec<String>,
}

impl rustyline::Helper for DiracHelper {}

impl DiracCompleter {
    fn new() -> Self {
        Self {
            filename_completer: FilenameCompleter::new(),
            command_history: Vec::new(),
        }
    }
}

impl Completer for DiracCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) 
        -> rustyline::Result<(usize, Vec<Pair>)> {
        // First try filename completion
        let filename_result = self.filename_completer.complete(line, pos, _ctx)?;
        
        // If we have filename completions, return those
        if !filename_result.1.is_empty() {
            return Ok(filename_result);
        }

        // Otherwise, try command history completion
        let word = line[..pos].split_whitespace().last().unwrap_or("");
        let start = pos - word.len();
        
        let mut matches: Vec<Pair> = self.command_history.iter()
            .filter(|cmd| cmd.starts_with(word))
            .map(|cmd| Pair {
                display: cmd.to_string(),
                replacement: cmd.to_string(),
            })
            .collect();
        matches.dedup_by(|a, b| a.display == b.display);
        
        Ok((start, matches))
    }
}

pub struct DiracHelper {
    completer: DiracCompleter,
    validator: MatchingBracketValidator,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
}

impl DiracHelper {
    fn new() -> Self {
        Self {
            completer: DiracCompleter::new(),
            validator: MatchingBracketValidator::new(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter {},
        }
    }
}

impl Completer for DiracHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) 
        -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Validator for DiracHelper {
    fn validate(&self, ctx: &mut rustyline::validate::ValidationContext) -> rustyline::Result<rustyline::validate::ValidationResult> {
        self.validator.validate(ctx)
    }
}

impl Highlighter for DiracHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Hinter for DiracHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

use crate::services::{ShellCommandExecutor, OllamaProcessor};
use crate::core::{DefaultPluginManager, AIProcessor, CommandExecutor, DiracError, PluginManager};

pub struct DiracTerminal {
    editor: Editor<DiracHelper, DefaultHistory>,
    command_executor: ShellCommandExecutor,
    ai_processor: OllamaProcessor,
}

impl DiracTerminal {
    pub fn new() -> Self {
        let config = Config::builder()
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();
        let editor = Editor::with_config(config).unwrap();
        Self {
            editor,
            command_executor: ShellCommandExecutor::new(),
            ai_processor: OllamaProcessor::with_default_config(),
        }
    }
    
    pub fn display_welcome(&self) {
        println!("{}", "=== Welcome to Dirac - Your AI-powered terminal! ===".green().bold());
        println!("{}", "Available features:".blue());
        println!("{}", " - Natural language command processing".blue());
        println!("{}", " - Smart command completion and suggestions".blue());
        println!("{}", " - File path completion".blue());
        println!("{}", " - Command history with search".blue());
        println!("{}", " - Plugin system for extended functionality".blue());
        println!("{}", "\nType 'help' for more information or start typing your commands.".yellow());
    }

    pub async fn run(&mut self) {
        self.display_welcome();

        // Set up signal handlers for terminal control
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let tx_clone = tx.clone();

        // Spawn a task to handle terminal control signals
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigtstp = signal(SignalKind::from_raw(libc::SIGTSTP)).unwrap();
            let mut sigcont = signal(SignalKind::from_raw(libc::SIGCONT)).unwrap();

            loop {
                tokio::select! {
                    _ = sigint.recv() => {
                        let _ = tx_clone.send("INT").await;
                    }
                    _ = sigtstp.recv() => {
                        let _ = tx_clone.send("TSTP").await;
                    }
                    _ = sigcont.recv() => {
                        let _ = tx_clone.send("CONT").await;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                signal = rx.recv() => {
                    match signal.unwrap_or_default() {
                        "INT" => {
                            println!("{}", "\nCTRL-C pressed. Use 'exit' or 'quit' to exit properly.".yellow());
                            continue;
                        }
                        "TSTP" => {
                            println!("{}", "\nCTRL-Z pressed. Terminal will continue running.".yellow());
                            continue;
                        }
                        "CONT" => {
                            println!("{}", "\nTerminal resumed.".green());
                            self.editor.clear_screen().unwrap_or_default();
                            self.display_welcome();
                        }
                        _ => {}
                    }
                }
                input_result = self.process_input() => {
                    match input_result {
                        Ok(should_exit) => {
                            if should_exit {
                                println!("{}", "Goodbye!".green());
                                break;
                            }
                        }
                        Err(ReadlineError::Interrupted) => {
                            println!("{}", "CTRL-C pressed. Use 'exit' or 'quit' to exit properly.".yellow());
                            continue;
                        }
                        Err(ReadlineError::Eof) => {
                            println!("{}", "CTRL-D pressed. Use 'exit' or 'quit' to exit properly.".yellow());
                            continue;
                        }
                        Err(err) => {
                            eprintln!("{} {}", "Error:".red(), err);
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_input(&mut self) -> Result<bool, ReadlineError> {
        let current_dir = self.command_executor.get_current_dir();
        let path_components: Vec<&str> = current_dir.split('/').filter(|s| !s.is_empty()).collect();
        let dir_display = if path_components.len() >= 2 {
            format!("{}/{}", path_components[path_components.len()-2], path_components.last().unwrap())
        } else if path_components.len() == 1 {
            path_components[0].to_string()
        } else {
            "/".to_string()
        };
        let prompt = format!("dirac[{}]> ", dir_display);
        let line = self.editor.readline(&prompt)?;
        self.editor.add_history_entry(line.as_str()).unwrap();
        let input = line.trim();

        if input.is_empty() {
            return Ok(false);
        }

        if input == "exit" || input == "quit" {
            return Ok(true);
        }

        match self.command_executor.execute(input).await {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                eprintln!("{}", e.to_string().red());
                // Get AI feedback for the failed command
                match self.ai_processor.process(
                    &format!("Command '{}' failed. Please explain what went wrong and suggest a solution.", input),
                    &e.to_string()
                ).await {
                    Ok(feedback) => {
                        println!("");
                        println!("{}", "🤖 AI Feedback:".blue().bold());
                        println!("{}", feedback);
                    }
                    Err(ai_err) => {
                        eprintln!("{}", format!("Failed to get AI feedback: {}", ai_err).red());
                    }
                }
            }
        };
        Ok(false)
    }

    async fn process_command(&mut self, input: &str) {
        let input = input.trim();

        // Handle empty input
        if input.is_empty() {
            return;
        }

        // Check for common typos in directory names
        if input.starts_with("cd ") {
            let path = input.trim_start_matches("cd ").trim();
            if !std::path::Path::new(path).exists() {
                // Try to find similar directory names
                if let Ok(entries) = std::fs::read_dir(".") {
                    let similar: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .filter(|name| name.len() >= 2 && path.len() >= 2 && 
                                name.chars().next() == path.chars().next())
                        .collect();

                    if !similar.is_empty() {
                        println!("{}", "Did you mean one of these directories?".yellow());
                        for dir in similar {
                            println!("  {}", dir.blue());
                        }
                        return;
                    }
                }
            }
        }

        // Check if it's a natural language navigation command
        if input.starts_with("go to ") || input.starts_with("open ") || input.starts_with("change to ") {
            let path = input.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
            let cd_command = format!("cd {}", path);
            self.execute_direct_command(&cd_command).await;
        }
        // If it's a direct command, execute it
        else if self.command_executor.is_valid_command(input) {
            self.execute_direct_command(input).await;
        } else {
            self.process_ai_command(input).await;
        }
    }

    async fn execute_direct_command(&mut self, command: &str) {
        match self.command_executor.execute(command).await {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
                // Ensure output is flushed
                std::io::stdout().flush().unwrap_or_default();
            }
            Err(e) => {
                eprintln!("{}", e.to_string().red());
                std::io::stderr().flush().unwrap_or_default();
            }
        }
    }

    async fn process_ai_command(&mut self, input: &str) {
        println!("{}", "🤖 Processing with AI...".yellow().bold());
        println!("{} {}", "Request:".blue(), input);
        println!("{}", "Analyzing request and generating command...".yellow());
        
        match self.ai_processor.process(input, String::new().as_str()).await {
            Ok(suggested_command) => self.handle_ai_suggestion(suggested_command.as_str()).await,
            Err(e) => self.handle_ai_error(e),
        }
    }

    async fn handle_ai_suggestion(&mut self, suggested_command: &str) {
        // Parse command and explanation from the AI response
        let mut command = String::new();
        let mut explanation = String::new();
    
        for line in suggested_command.lines() {
            if line.starts_with("COMMAND:") {
                command = line.trim_start_matches("COMMAND:").trim().to_string();
            } else if line.starts_with("EXPLANATION:") {
                explanation = line.trim_start_matches("EXPLANATION:").trim().to_string();
            }
        }
    
        if command.is_empty() {
            eprintln!("{}", "❌ AI could not generate a suitable command for your request.".red().bold());
            eprintln!("{}", "Try rephrasing your request or use more specific terms.".yellow());
            return;
        }
    
        println!("{}", "\n=== Command Suggestion =====".green().bold());
        println!("{} {}", "📎 Command:".blue(), command.yellow());
        if !explanation.is_empty() {
            println!("{} {}", "💡 Details:".blue(), explanation);
        }
    
        // Only show execution prompt if we have a valid command
        println!("{}", "\nWould you like to execute this command? [y/N/e(explain)]:".yellow());
    
        if let Ok(confirmation) = self.editor.readline("") {
            match confirmation.trim().to_lowercase().as_str() {
                "y" => self.execute_direct_command(&command).await,
                "e" => {
                    if !explanation.is_empty() {
                        println!("{}", "\n=== Command Explanation ====".blue().bold());
                        println!("{}", explanation);
                        println!("{}", "\nWould you like to execute this command now? [y/N]:".yellow());
                        if let Ok(second_confirmation) = self.editor.readline("") {
                            if second_confirmation.trim().to_lowercase() == "y" {
                                self.execute_direct_command(&command).await;
                            }
                        }
                    } else {
                        println!("{}", "No detailed explanation available for this command.".yellow());
                    }
                }
                _ => println!("{}", "Command execution cancelled.".yellow())
            }
        }
    }

    fn handle_ai_error(&self, error: DiracError) {
        eprintln!("{}", "Error processing with AI:".red());
        eprintln!("{}", error.to_string().red());
        eprintln!("{}", "Please ensure the Ollama service is running correctly.".yellow());
    }
}