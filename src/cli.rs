use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// OpenSpec Orchestrator - Automate OpenSpec workflow
#[derive(Parser, Debug)]
#[command(name = "openspec-orchestrator")]
#[command(about = "Automates OpenSpec change workflow (list → apply → archive)", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to opencode binary (used when no subcommand is provided, deprecated - use config file instead)
    #[arg(long, default_value = "opencode", global = true)]
    pub opencode_path: String,

    /// OpenSpec command (can include arguments, e.g., "npx @fission-ai/openspec@latest")
    #[arg(
        long,
        env = "OPENSPEC_CMD",
        default_value = "npx @fission-ai/openspec@latest",
        global = true
    )]
    pub openspec_cmd: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the OpenSpec change orchestration loop (non-interactive)
    Run(RunArgs),

    /// Launch the interactive TUI dashboard
    Tui(TuiArgs),
}

/// Arguments for the run subcommand
#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Process only the specified change
    #[arg(long)]
    pub change: Option<String>,

    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// OpenSpec command (can include arguments, e.g., "npx @fission-ai/openspec@latest")
    #[arg(
        long,
        env = "OPENSPEC_CMD",
        default_value = "npx @fission-ai/openspec@latest"
    )]
    pub openspec_cmd: String,
}

/// Arguments for the TUI subcommand
#[derive(Parser, Debug)]
pub struct TuiArgs {
    /// Path to opencode binary (deprecated - use config file instead)
    #[arg(long, default_value = "opencode")]
    pub opencode_path: String,

    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// OpenSpec command (can include arguments, e.g., "npx @fission-ai/openspec@latest")
    #[arg(
        long,
        env = "OPENSPEC_CMD",
        default_value = "npx @fission-ai/openspec@latest"
    )]
    pub openspec_cmd: String,
}
