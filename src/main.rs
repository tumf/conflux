mod cli;
mod error;
mod opencode;
mod openspec;
mod orchestrator;
mod progress;
mod state;

use clap::Parser;
use cli::{Cli, Commands};
use error::Result;
use orchestrator::Orchestrator;
use state::OrchestratorState;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            dry_run,
            change,
            opencode_path,
            openspec_path,
        } => {
            info!("Starting orchestrator");
            let mut orchestrator =
                Orchestrator::new(&opencode_path, &openspec_path, dry_run, change)?;
            orchestrator.run().await?;
        }

        Commands::Status => {
            if let Some(state) = OrchestratorState::load()? {
                println!("=== Orchestrator Status ===");
                println!("Started at: {}", state.started_at);
                println!("Last update: {}", state.last_update);
                println!("Total iterations: {}", state.total_iterations);
                println!("\nCurrent change: {:?}", state.current_change);
                println!("\nProcessed changes: {}", state.processed_changes.len());
                for change in &state.processed_changes {
                    println!("  - {}", change);
                }
                println!("\nArchived changes: {}", state.archived_changes.len());
                for change in &state.archived_changes {
                    println!("  - {}", change);
                }
                println!("\nFailed changes: {}", state.failed_changes.len());
                for change in &state.failed_changes {
                    println!("  - {}", change);
                }
            } else {
                println!("No state found. Run 'orchestrator run' to start.");
            }
        }

        Commands::Reset { yes } => {
            if !yes {
                println!("This will reset the orchestrator state. Are you sure? (y/N)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
            OrchestratorState::reset()?;
            println!("State reset successfully.");
        }
    }

    Ok(())
}
