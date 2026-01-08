use clap::Parser;

/// OpenSpec Orchestrator - Automate OpenSpec workflow
#[derive(Parser, Debug)]
#[command(name = "openspec-orchestrator")]
#[command(about = "Automates OpenSpec change workflow (list → apply → archive)", long_about = None)]
pub struct Cli {
    /// Process only the specified change
    #[arg(long)]
    pub change: Option<String>,

    /// Path to opencode binary
    #[arg(long, default_value = "opencode")]
    pub opencode_path: String,

    /// OpenSpec command (can include arguments, e.g., "npx @fission-ai/openspec@latest")
    #[arg(long, default_value = "npx @fission-ai/openspec@latest")]
    pub openspec_cmd: String,
}
