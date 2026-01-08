mod cli;
mod error;
mod opencode;
mod openspec;
mod orchestrator;
mod progress;

use clap::Parser;
use cli::Cli;
use error::Result;
use orchestrator::Orchestrator;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    info!("Starting orchestrator");
    let mut orchestrator =
        Orchestrator::new(&cli.opencode_path, &cli.openspec_cmd, cli.change)?;
    orchestrator.run().await?;

    Ok(())
}
