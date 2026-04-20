use anyhow::Result;
use clap::Parser;
use relora::config::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = cli.into_config()?;
    relora::tui::run(config)
}
