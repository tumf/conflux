mod agent;
mod approval;
mod cli;
mod config;
mod error;
mod history;
mod hooks;
mod opencode;
mod openspec;
mod orchestrator;
mod progress;
mod task_parser;
mod templates;
mod tui;

use clap::Parser;
use cli::{ApproveAction, Cli, Commands};
use config::OrchestratorConfig;
use error::Result;
use orchestrator::Orchestrator;
use std::path::Path;
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

            // Get initial changes using native implementation
            let changes = openspec::list_changes_native()?;

            // Load config (uses default paths)
            let config = OrchestratorConfig::load(None)?;

            // Run TUI
            tui::run_tui(changes, openspec_cmd, opencode_path, config).await?;
        }

        // Explicit TUI subcommand
        Some(Commands::Tui(args)) => {
            // Get initial changes using native implementation
            let changes = openspec::list_changes_native()?;

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
            let mut orchestrator =
                Orchestrator::new(args.change, args.config, args.max_iterations)?;
            orchestrator.run().await?;
        }

        // Init subcommand: generate configuration file
        Some(Commands::Init(args)) => {
            let config_path = Path::new(".openspec-orchestrator.jsonc");

            if config_path.exists() && !args.force {
                eprintln!(
                    "Error: Configuration file '{}' already exists.",
                    config_path.display()
                );
                eprintln!("Use --force to overwrite the existing file.");
                std::process::exit(1);
            }

            let content = templates::get_template_content(args.template);
            std::fs::write(config_path, content)?;

            println!(
                "Created configuration file '{}' with {:?} template.",
                config_path.display(),
                args.template
            );
        }

        // Approve subcommand: manage change approval status
        Some(Commands::Approve(args)) => match args.action {
            ApproveAction::Set { change_id } => {
                // Check if change exists
                let change_dir = Path::new("openspec/changes").join(&change_id);
                if !change_dir.exists() {
                    eprintln!("Error: Change '{}' does not exist.", change_id);
                    std::process::exit(1);
                }

                match approval::approve_change(&change_id) {
                    Ok(_) => {
                        println!("Approved change '{}'.", change_id);
                    }
                    Err(e) => {
                        eprintln!("Error approving change '{}': {}", change_id, e);
                        std::process::exit(1);
                    }
                }
            }
            ApproveAction::Unset { change_id } => {
                // Check if change exists
                let change_dir = Path::new("openspec/changes").join(&change_id);
                if !change_dir.exists() {
                    eprintln!("Error: Change '{}' does not exist.", change_id);
                    std::process::exit(1);
                }

                match approval::unapprove_change(&change_id) {
                    Ok(_) => {
                        println!("Unapproved change '{}'.", change_id);
                    }
                    Err(e) => {
                        eprintln!("Error unapproving change '{}': {}", change_id, e);
                        std::process::exit(1);
                    }
                }
            }
            ApproveAction::Status { change_id } => {
                // Check if change exists
                let change_dir = Path::new("openspec/changes").join(&change_id);
                if !change_dir.exists() {
                    eprintln!("Error: Change '{}' does not exist.", change_id);
                    std::process::exit(1);
                }

                match approval::check_approval(&change_id) {
                    Ok(approved) => {
                        if approved {
                            println!("Change '{}' is approved.", change_id);
                        } else {
                            println!("Change '{}' is not approved.", change_id);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error checking approval status for '{}': {}", change_id, e);
                        std::process::exit(1);
                    }
                }
            }
        },
    }

    Ok(())
}
