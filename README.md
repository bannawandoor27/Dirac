# Dirac - AI-Powered Terminal

Dirac is an intelligent terminal interface that understands natural language commands, making command-line operations more intuitive and accessible.

## Features

- 🧠 Natural language command processing
- 🔍 Smart command completion and suggestions
- 📁 File path completion
- 📝 Command history with search
- 🔌 Plugin system for extended functionality
- 🎨 Colorful and intuitive interface
- ⌨️ Emacs-style key bindings
- 🔄 Signal handling (CTRL-C, CTRL-Z)

## Installation

### Prerequisites

- Rust and Cargo (latest stable version)
- [Ollama](https://ollama.ai/) for AI processing

### Building from Source

```bash
# Clone the repository
git clone https://github.com/bannawandoor27/Dirac.git
cd Dirac

# Build the project
cargo build --release

# Run Dirac
cargo run
```

## Usage

Dirac accepts both natural language commands and traditional shell syntax:

```bash
# Natural language examples
dirac> show me all text files in the current directory
dirac> create a new directory called projects
dirac> go to the downloads folder

# Traditional syntax also works
dirac> ls *.txt
dirac> mkdir projects
dirac> cd ~/Downloads
```

### Key Features

1. **Natural Language Processing**
   - Describe what you want to do in plain English
   - AI translates your request into the appropriate command
   - Review and confirm before execution

2. **Smart Completion**
   - Tab completion for files and directories
   - Command history suggestions
   - Bracket matching and syntax highlighting

3. **Plugin System**
   - Extend functionality with custom plugins
   - Easy integration with existing tools

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with Rust 🦀
- Powered by Ollama for AI processing
- Inspired by modern AI-powered developer tools