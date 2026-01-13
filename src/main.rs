mod agent;
mod analyzer;
mod approval;
mod cli;
mod config;
mod error;
mod events;
mod execution;
mod history;
mod hooks;
mod openspec;
mod orchestration;
mod orchestrator;
mod parallel;
mod parallel_run_service;
mod process_manager;
mod progress;
mod task_parser;
mod templates;
mod tui;
mod vcs;
#[cfg(feature = "web-monitoring")]
mod web;

use clap::Parser;
use cli::{ApproveAction, Cli, Commands};
use config::OrchestratorConfig;
use error::Result;
use orchestrator::Orchestrator;
use std::path::Path;
use tracing::{info, Level};
use tracing_subscriber::prelude::*;

/// Initialize file-based logging for TUI mode
fn init_file_logging(log_path: &Path) -> Result<()> {
    use std::fs::File;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    let file = File::create(log_path).map_err(|e| {
        error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to create log file '{}': {}",
            log_path.display(),
            e
        )))
    })?;

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file.with_max_level(Level::DEBUG))
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry().with(file_layer).init();

    Ok(())
}

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

            // Run TUI (no web server in default mode)
            tui::run_tui(changes, openspec_cmd, opencode_path, config, None).await?;
        }

        // Explicit TUI subcommand
        Some(Commands::Tui(args)) => {
            // Initialize file logging if --logs is specified
            if let Some(log_path) = &args.logs {
                init_file_logging(log_path)?;
            }

            // Get initial changes using native implementation
            let changes = openspec::list_changes_native()?;

            // Load config
            let config = OrchestratorConfig::load(args.config.as_deref())?;

            // Start web monitoring server if enabled and build URL
            #[cfg(feature = "web-monitoring")]
            let web_url = if args.web {
                let web_state = std::sync::Arc::new(web::WebState::new(&changes));
                let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone());
                match web::spawn_server_with_url(web_config, web_state).await {
                    Ok((_web_handle, url)) => Some(url),
                    Err(e) => {
                        tracing::warn!("Failed to start web monitoring server: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            #[cfg(not(feature = "web-monitoring"))]
            let web_url: Option<String> = {
                if args.web {
                    eprintln!(
                        "Warning: Web monitoring is not enabled. Compile with --features web-monitoring"
                    );
                }
                None
            };

            // Run TUI
            tui::run_tui(
                changes,
                args.openspec_cmd,
                args.opencode_path,
                config,
                web_url,
            )
            .await?;
        }

        // Run subcommand: non-interactive orchestration
        Some(Commands::Run(args)) => {
            // Initialize tracing for non-interactive mode
            tracing_subscriber::fmt().with_max_level(Level::INFO).init();

            // Start web monitoring server if enabled
            #[cfg(feature = "web-monitoring")]
            let web_state_arc = if args.web {
                let initial_changes = openspec::list_changes_native()?;
                let web_state = std::sync::Arc::new(web::WebState::new(&initial_changes));
                let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone());
                match web::spawn_server_with_url(web_config, web_state.clone()).await {
                    Ok((_handle, url)) => {
                        info!("Web monitoring available at: {}", url);
                        Some(web_state)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to start web monitoring server: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            #[cfg(not(feature = "web-monitoring"))]
            if args.web {
                eprintln!(
                    "Warning: Web monitoring is not enabled. Compile with --features web-monitoring"
                );
            }

            // Parse VCS backend from CLI option
            let vcs_override = match args.vcs.parse::<vcs::VcsBackend>() {
                Ok(backend) => Some(backend),
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            };

            if args.parallel {
                let backend = vcs_override.unwrap_or(vcs::VcsBackend::Auto);
                let git_dir_exists = cli::check_git_directory();
                let git_available = cli::check_git_available();

                if !git_dir_exists {
                    let message = if matches!(backend, vcs::VcsBackend::Git) {
                        "git repository not found (.git directory missing)"
                    } else {
                        "Error: --parallel requires a git repository (.git directory not found)"
                    };
                    eprintln!("{}", message);
                    std::process::exit(1);
                }

                if !git_available {
                    eprintln!("Error: git command not available");
                    std::process::exit(1);
                }
            }

            info!("Starting orchestrator");
            let mut orchestrator = Orchestrator::new(
                args.change,
                args.config,
                args.max_iterations,
                args.parallel,
                args.max_concurrent,
                args.dry_run,
                vcs_override,
                args.no_resume,
            )?;

            // Set web state for broadcasting updates
            #[cfg(feature = "web-monitoring")]
            if let Some(web_state) = web_state_arc {
                orchestrator.set_web_state(web_state);
            }

            // Setup signal handling for graceful shutdown
            let cancel_token = tokio_util::sync::CancellationToken::new();
            let cancel_for_signal = cancel_token.clone();

            // Spawn signal handler task
            #[cfg(unix)]
            {
                let cancel_for_sigterm = cancel_for_signal.clone();
                tokio::spawn(async move {
                    let mut sigterm =
                        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                            .expect("Failed to install SIGTERM handler");
                    sigterm.recv().await;
                    info!("Received SIGTERM, shutting down gracefully...");
                    cancel_for_sigterm.cancel();
                });
            }

            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                info!("Received SIGINT (Ctrl+C), shutting down gracefully...");
                cancel_for_signal.cancel();
            });

            orchestrator.run(cancel_token).await?;
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
