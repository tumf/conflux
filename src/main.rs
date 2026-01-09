mod agent;
mod cli;
mod config;
mod error;
mod hooks;
mod opencode;
mod openspec;
mod orchestrator;
mod progress;
mod tui;

use clap::Parser;
use cli::{Cli, Commands};
use config::OrchestratorConfig;
use error::Result;
use orchestrator::Orchestrator;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand: launch TUI (default behavior)
        None => {
            // Don't initialize tracing subscriber for TUI mode
            // (TUI handles its own output)
            let openspec_cmd = cli.openspec_cmd;
            let opencode_path = cli.opencode_path;

            // Get initial changes
            let changes = openspec::list_changes(&openspec_cmd).await?;

            // Load config (uses default paths)
            let config = OrchestratorConfig::load(None)?;

            // Run TUI
            tui::run_tui(changes, openspec_cmd, opencode_path, config).await?;
        }

        // Explicit TUI subcommand
        Some(Commands::Tui(args)) => {
            // Get initial changes
            let changes = openspec::list_changes(&args.openspec_cmd).await?;

            // Load config
            let config = OrchestratorConfig::load(args.config.as_deref())?;

            // Run TUI
            tui::run_tui(changes, args.openspec_cmd, args.opencode_path, config).await?;
        }

        // Run subcommand: non-interactive orchestration
        Some(Commands::Run(args)) => {
            // Initialize tracing for non-interactive mode
            tracing_subscriber::fmt().with_max_level(Level::INFO).init();

            info!("Starting orchestrator");
            let mut orchestrator = Orchestrator::new(&args.openspec_cmd, args.change, args.config)?;
            orchestrator.run().await?;
        }
    }

    Ok(())
}
