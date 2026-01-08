use clap::{Parser, Subcommand};

/// OpenSpec Orchestrator - Automate OpenSpec workflow
#[derive(Parser, Debug)]
#[command(name = "openspec-orchestrator")]
#[command(about = "Automates OpenSpec change workflow (list → apply → archive)", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the orchestration loop
    Run {
        /// Perform a dry run without executing changes
        #[arg(long)]
        dry_run: bool,

        /// Process only the specified change
        #[arg(long)]
        change: Option<String>,

        /// Path to opencode binary
        #[arg(long, default_value = "opencode")]
        opencode_path: String,

        /// Path to openspec binary
        #[arg(long, default_value = "openspec")]
        openspec_path: String,
    },

    /// Show current orchestration status
    Status,

    /// Reset orchestration state
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}
