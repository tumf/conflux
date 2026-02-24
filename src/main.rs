mod acceptance;
mod agent;
mod ai_command_runner;
mod analyzer;

mod cli;
mod command_queue;
mod config;
mod error;
mod error_history;
mod events;
mod execution;
mod history;
mod hooks;
mod merge_stall_monitor;
mod openspec;
mod orchestration;
mod orchestrator;
mod parallel;
mod parallel_run_service;
mod permission;
mod process_manager;
mod progress;
mod serial_run_service;
mod spec_delta;
#[cfg(test)]
mod spec_test_annotations;
mod stall;
mod task_parser;
mod templates;
mod tui;
mod vcs;
#[cfg(feature = "web-monitoring")]
mod web;
mod worktree_ops;

#[cfg(test)]
mod test_support;

use clap::Parser;
use cli::{Cli, Commands, TuiArgs};
use config::OrchestratorConfig;
use error::Result;
use orchestrator::Orchestrator;
use std::path::Path;
use tracing::{error, info, warn, Level};
use tracing_subscriber::prelude::*;

/// Initialize logging.
///
/// - Always enables file logging with automatic log rotation and cleanup.
/// - Optionally enables stdout logging (for non-TUI modes).
///
/// Logs are written to XDG_STATE_HOME/cflx/logs/<project_slug>/<YYYY-MM-DD>.log.
/// Old logs are automatically cleaned up (7-day retention).
fn init_logging(enable_stdout: bool) -> Result<()> {
    use config::defaults::{cleanup_old_logs, get_log_file_path};
    use std::fs::{create_dir_all, File};
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    // Get current directory as repo root
    let repo_root = std::env::current_dir().ok();

    // Get log file path
    let log_path = get_log_file_path(repo_root.as_deref());

    // Create parent directory if it doesn't exist
    if let Some(parent) = log_path.parent() {
        create_dir_all(parent).map_err(|e| {
            error::OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to create log directory '{}': {}",
                parent.display(),
                e
            )))
        })?;
    }

    // Clean up old logs (7-day retention)
    if let Err(e) = cleanup_old_logs(repo_root.as_deref(), 7) {
        tracing::warn!("Failed to clean up old logs: {}", e);
    }

    // Create or append to log file
    let file = File::options()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| {
            error::OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to open log file '{}': {}",
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

    let registry = tracing_subscriber::registry().with(file_layer);

    if enable_stdout {
        let stdout_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false);

        registry.with(stdout_layer).init();
    } else {
        registry.init();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand: launch TUI (default behavior)
        None => {
            // Initialize logging: file only (avoid stdout noise in TUI)
            init_logging(false)?;

            // Build TuiArgs from global flags
            let tui_args = TuiArgs {
                config: cli.config,
                web: cli.web,
                web_port: cli.web_port,
                web_bind: cli.web_bind,
            };

            // Load config
            let config = OrchestratorConfig::load(tui_args.config.as_deref())?;
            tui::log_deduplicator::configure_logging(config.get_logging());

            // Get initial changes using native implementation
            let changes = openspec::list_changes_native()?;

            // Start web monitoring server if enabled and build URL
            #[cfg(feature = "web-monitoring")]
            let (web_url, web_state_opt) = if tui_args.web {
                let web_state = std::sync::Arc::new(web::WebState::new(&changes));
                let web_config =
                    web::WebConfig::enabled(tui_args.web_port, tui_args.web_bind.clone());
                match web::spawn_server_with_url(web_config, web_state.clone()).await {
                    Ok((_web_handle, url)) => (Some(url), Some(web_state)),
                    Err(e) => {
                        tracing::warn!("Failed to start web monitoring server: {}", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
            };

            #[cfg(not(feature = "web-monitoring"))]
            let web_url: Option<String> = {
                if tui_args.web {
                    eprintln!(
                        "Warning: Web monitoring is not enabled. Compile with --features web-monitoring"
                    );
                }
                None
            };

            // Run TUI
            tui::run_tui(
                changes,
                config,
                web_url,
                #[cfg(feature = "web-monitoring")]
                web_state_opt,
            )
            .await?;
        }

        // Explicit TUI subcommand
        Some(Commands::Tui(args)) => {
            // Initialize logging: file only (avoid stdout noise in TUI)
            init_logging(false)?;

            // Load config
            let config = OrchestratorConfig::load(args.config.as_deref())?;
            tui::log_deduplicator::configure_logging(config.get_logging());

            // Get initial changes using native implementation
            let changes = openspec::list_changes_native()?;

            // Start web monitoring server if enabled and build URL
            #[cfg(feature = "web-monitoring")]
            let (web_url, web_state_opt) = if args.web {
                let web_state = std::sync::Arc::new(web::WebState::new(&changes));
                let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone());
                match web::spawn_server_with_url(web_config, web_state.clone()).await {
                    Ok((_web_handle, url)) => (Some(url), Some(web_state)),
                    Err(e) => {
                        tracing::warn!("Failed to start web monitoring server: {}", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
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
                config,
                web_url,
                #[cfg(feature = "web-monitoring")]
                web_state_opt,
            )
            .await?;
        }

        // Run subcommand: non-interactive orchestration
        Some(Commands::Run(args)) => {
            // Initialize logging: include stdout for run mode
            init_logging(true)?;

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

            let config = OrchestratorConfig::load(args.config.as_deref())?;
            let git_dir_exists = cli::check_git_directory();
            let use_parallel = config.resolve_parallel_mode(args.parallel, git_dir_exists);

            if use_parallel {
                let backend = vcs_override.unwrap_or(vcs::VcsBackend::Auto);
                let git_available = cli::check_git_available();

                if !git_dir_exists {
                    let message = if matches!(backend, vcs::VcsBackend::Git) {
                        "git repository not found (.git directory missing)"
                    } else {
                        "Error: parallel mode requires a git repository (.git directory not found)"
                    };
                    eprintln!("{}", message);
                    std::process::exit(1);
                }

                if !git_available {
                    eprintln!("Error: git command not available");
                    std::process::exit(1);
                }
            }

            // Run mode control state for web control integration
            // Run mode now supports retry and resume via outer loop.
            use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
            use std::sync::Arc;

            // Control state: 0 = Stopped, 1 = Running, 2 = Stopping
            let run_state = Arc::new(AtomicU8::new(1)); // Start in Running state
            let graceful_stop_flag = Arc::new(AtomicBool::new(false));
            let force_stop_flag = Arc::new(AtomicBool::new(false));
            let restart_requested = Arc::new(AtomicBool::new(false));

            // Set web state for broadcasting updates and wire control channel
            #[cfg(feature = "web-monitoring")]
            if let Some(web_state) = &web_state_arc {
                // Create unbounded channel for web control commands
                let (control_tx, mut control_rx) =
                    tokio::sync::mpsc::unbounded_channel::<web::state::ControlCommand>();

                // Set the control channel in WebState
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        web_state.set_control_channel(control_tx).await;
                    })
                });

                // Spawn bridge task to handle control commands
                let bridge_run_state = run_state.clone();
                let bridge_graceful_stop = graceful_stop_flag.clone();
                let bridge_force_stop = force_stop_flag.clone();
                let bridge_restart = restart_requested.clone();
                let bridge_web_state = web_state.clone();
                tokio::spawn(async move {
                    loop {
                        if let Some(control_cmd) = control_rx.recv().await {
                            use crate::events::ExecutionEvent;
                            use web::state::ControlCommand;
                            match control_cmd {
                                ControlCommand::Start => {
                                    let current_state = bridge_run_state.load(Ordering::SeqCst);
                                    if current_state == 2 {
                                        // Stopping -> Running (acts like CancelStop)
                                        info!("Web control: Start requested, canceling stop and resuming");
                                        bridge_graceful_stop.store(false, Ordering::SeqCst);
                                        bridge_run_state.store(1, Ordering::SeqCst);
                                    } else if current_state == 1 {
                                        info!("Web control: Start requested but already running");
                                    } else {
                                        // State 0 (Stopped) - request restart in outer loop
                                        info!("Web control: Start requested after stop, will restart orchestrator");
                                        bridge_restart.store(true, Ordering::SeqCst);
                                        bridge_run_state.store(1, Ordering::SeqCst);
                                    }
                                }
                                ControlCommand::Stop => {
                                    info!("Web control: Graceful stop requested");
                                    bridge_graceful_stop.store(true, Ordering::SeqCst);
                                    bridge_run_state.store(2, Ordering::SeqCst);
                                    // Immediately broadcast stopping mode to web UI
                                    bridge_web_state
                                        .apply_execution_event(&ExecutionEvent::Stopping)
                                        .await;
                                }
                                ControlCommand::CancelStop => {
                                    let current_state = bridge_run_state.load(Ordering::SeqCst);
                                    if current_state == 2 {
                                        // Stopping -> Running
                                        info!("Web control: Cancel stop requested");
                                        bridge_graceful_stop.store(false, Ordering::SeqCst);
                                        bridge_run_state.store(1, Ordering::SeqCst);
                                        // Broadcast running mode immediately
                                        bridge_web_state
                                            .apply_execution_event(
                                                &ExecutionEvent::ProcessingStarted("".to_string()),
                                            )
                                            .await;
                                    } else {
                                        warn!("Web control: Cancel stop requested but not in stopping state");
                                    }
                                }
                                ControlCommand::ForceStop => {
                                    info!("Web control: Force stop requested");
                                    bridge_force_stop.store(true, Ordering::SeqCst);
                                    bridge_graceful_stop.store(true, Ordering::SeqCst);
                                    bridge_run_state.store(0, Ordering::SeqCst);
                                    // Broadcast stopped mode immediately
                                    bridge_web_state
                                        .apply_execution_event(&ExecutionEvent::Stopped)
                                        .await;
                                }
                                ControlCommand::Retry => {
                                    let current_state = bridge_run_state.load(Ordering::SeqCst);
                                    if current_state == 2 {
                                        // Stopping -> Running (resume)
                                        info!("Web control: Retry requested, canceling stop and resuming");
                                        bridge_graceful_stop.store(false, Ordering::SeqCst);
                                        bridge_run_state.store(1, Ordering::SeqCst);
                                    } else if current_state == 1 {
                                        info!("Web control: Retry requested during execution, will restart after completion");
                                        bridge_restart.store(true, Ordering::SeqCst);
                                    } else {
                                        // State 0 (Stopped) - request restart
                                        info!("Web control: Retry requested after stop, will restart orchestrator");
                                        bridge_restart.store(true, Ordering::SeqCst);
                                        bridge_run_state.store(1, Ordering::SeqCst);
                                    }
                                }
                            }
                        }
                    }
                });
            }

            // Signal handler flags (shared across all iterations)
            let signal_stop = Arc::new(AtomicBool::new(false));

            // Spawn signal handler tasks
            #[cfg(unix)]
            {
                let signal_stop_sigterm = signal_stop.clone();
                tokio::spawn(async move {
                    let mut sigterm =
                        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                            .expect("Failed to install SIGTERM handler");
                    sigterm.recv().await;
                    info!("Received SIGTERM, shutting down gracefully...");
                    signal_stop_sigterm.store(true, Ordering::SeqCst);
                });
            }

            {
                let signal_stop_sigint = signal_stop.clone();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    info!("Received SIGINT (Ctrl+C), shutting down gracefully...");
                    signal_stop_sigint.store(true, Ordering::SeqCst);
                });
            }

            // Clone args for use in restart loop
            let change_ids = args.change.clone();
            let config_path = args.config.clone();
            let max_iterations = args.max_iterations;
            let max_concurrent = args.max_concurrent;
            let dry_run = args.dry_run;
            let no_resume = args.no_resume;

            // Outer loop for retry/restart support in Run mode
            loop {
                // Check for signal stop before starting new iteration
                if signal_stop.load(Ordering::SeqCst) {
                    info!("Signal stop detected, exiting");
                    break;
                }

                info!("Starting orchestrator");
                let mut orchestrator = Orchestrator::new(
                    change_ids.clone(),
                    config_path.clone(),
                    max_iterations,
                    use_parallel,
                    max_concurrent,
                    dry_run,
                    vcs_override,
                    no_resume,
                )?;

                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = web_state_arc {
                    orchestrator.set_web_state(web_state.clone()).await;
                }

                // Create a fresh cancel token for this run iteration
                let cancel_token = tokio_util::sync::CancellationToken::new();

                // Monitor stop flags and trigger cancellation for this iteration
                // Note: graceful_stop is NOT monitored here - it's checked directly in orchestrator loop
                // This allows CancelStop to clear the flag before orchestrator sees it
                let monitor_token = cancel_token.clone();
                let monitor_force = force_stop_flag.clone();
                let monitor_signal = signal_stop.clone();
                let monitor_handle = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if monitor_signal.load(Ordering::SeqCst)
                            || monitor_force.load(Ordering::SeqCst)
                        {
                            if monitor_force.load(Ordering::SeqCst) {
                                info!("Force stop detected, cancelling orchestrator");
                            } else {
                                info!("Signal received, cancelling orchestrator");
                            }
                            monitor_token.cancel();
                            break;
                        }
                    }
                });

                let result = orchestrator
                    .run(cancel_token, Some(graceful_stop_flag.clone()))
                    .await;

                // Cancel monitor task
                monitor_handle.abort();

                // After orchestrator completes, update state
                run_state.store(0, Ordering::SeqCst); // Stopped

                // Handle result - wait for restart requests in both error and stopped states
                match result {
                    Err(e) => {
                        error!("Orchestrator error: {}", e);

                        // Wait for retry request in error state
                        // Keep checking restart_requested flag until user requests retry or signals stop
                        loop {
                            // Check if restart was requested
                            if restart_requested.load(Ordering::SeqCst) {
                                info!("Retry requested after error, will restart orchestrator");
                                break;
                            }

                            // Check if force stop or signal was received (exit on those)
                            if force_stop_flag.load(Ordering::SeqCst)
                                || signal_stop.load(Ordering::SeqCst)
                            {
                                info!("Stop requested in error state, exiting");
                                return Err(e);
                            }

                            // Wait a bit before checking again (100ms polling interval)
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }

                        info!("Continuing after error due to retry request");
                    }
                    Ok(()) => {
                        // Successful completion or graceful stop
                        info!("Orchestrator completed successfully");

                        // Wait for restart request in stopped state (to support resume from stop)
                        loop {
                            // Check if restart was requested
                            if restart_requested.load(Ordering::SeqCst) {
                                info!("Restart requested after stop, will restart orchestrator");
                                break;
                            }

                            // Check if force stop or signal was received (exit on those)
                            if force_stop_flag.load(Ordering::SeqCst)
                                || signal_stop.load(Ordering::SeqCst)
                            {
                                info!("Stop signal received, exiting");
                                break; // Exit outer loop below
                            }

                            // Wait a bit before checking again (100ms polling interval)
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                    }
                }

                // Check if restart was requested (Start/Retry from web UI or post-error/stop retry)
                if restart_requested.swap(false, Ordering::SeqCst) {
                    info!("Restarting orchestrator due to web control request");
                    run_state.store(1, Ordering::SeqCst); // Back to Running
                                                          // Reset stop flags for new run
                    graceful_stop_flag.store(false, Ordering::SeqCst);
                    force_stop_flag.store(false, Ordering::SeqCst);
                    continue; // Restart loop
                }

                // No restart requested, exit loop
                break;
            }
        }

        // Init subcommand: generate configuration file
        Some(Commands::Init(args)) => {
            let config_path = Path::new(".cflx.jsonc");

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

        // CheckConflicts subcommand: detect conflicts between spec delta files
        Some(Commands::CheckConflicts(args)) => {
            // Get list of all non-archived changes
            let changes = openspec::list_changes_native()?;

            // Collect all deltas from all changes
            let mut all_deltas = Vec::new();
            for change in &changes {
                match spec_delta::parse_change_deltas(&change.id) {
                    Ok(deltas) => all_deltas.extend(deltas),
                    Err(e) => {
                        eprintln!("Error parsing deltas for change '{}': {}", change.id, e);
                        std::process::exit(1);
                    }
                }
            }

            // Detect conflicts
            let conflicts = spec_delta::detect_conflicts(&all_deltas);

            // Output results
            if args.json {
                match spec_delta::format_conflicts_json(&conflicts) {
                    Ok(json) => {
                        println!("{}", json);
                    }
                    Err(e) => {
                        eprintln!("Error formatting JSON output: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let output = spec_delta::format_conflicts_human(&conflicts);
                println!("{}", output);
            }

            // Exit with non-zero status if conflicts found
            if !conflicts.is_empty() {
                std::process::exit(2);
            }
        }
    }

    Ok(())
}
