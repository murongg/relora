use anyhow::Result;
use clap::Parser;
use relora::config::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(command) = cli.command.clone() {
        return relora::commands::run(command);
    }
    let config = cli.into_config()?;
    relora::tui::run(config)
}
