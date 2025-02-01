mod core;
mod services;
mod ui;

use clap::Parser;
use crate::ui::terminal::DiracTerminal;

#[derive(Parser)]
#[command(name = "dirac")]
#[command(about = "AI-powered terminal that understands natural language")]
struct Cli {}

#[tokio::main]
async fn main() {
    let _cli = Cli::parse();
    let mut terminal = DiracTerminal::new();
    
    terminal.run().await;
}
