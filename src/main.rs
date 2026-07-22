#![cfg(not(test))]

mod acceptance;
mod agent;
mod ai_command_runner;
mod analyzer;
mod archive_layout;
mod embedded_skills;
mod install_skills;

mod cli;
mod command_queue;
mod completion;
mod config;
mod dependency_targets;
mod error;
mod error_history;
mod events;
mod execution;
mod history;
mod hooks;
mod log_viewer;
mod openspec;
mod openspec_cmd;
mod orchestration;
mod orchestrator;
mod parallel;
mod parallel_run_service;
mod permission;
mod process_manager;
mod progress;
mod remote;
#[allow(dead_code, unused_imports)]
mod runtime;
mod serial_run_service;
#[cfg(feature = "web-monitoring")]
mod server;
mod service;
mod shell_command;
mod spec_delta;
#[cfg(test)]
mod spec_test_annotations;
mod stall;
mod stream_json_textifier;
mod task_parser;
mod templates;
mod tui;
mod vcs;
#[cfg(feature = "web-monitoring")]
mod web;
mod worktree_ops;

#[cfg(test)]
mod test_support;

use clap::{CommandFactory, Parser};
use cli::{
    install_skills_legacy_error, Cli, Commands, InstallSkillsTarget, InternalCompleteCommands,
    LogsArgs, ProjectCommands, TuiArgs, VERSION_WITH_BUILD,
};
use config::OrchestratorConfig;
use error::Result;
use install_skills::{run_install_skills, InstallSkillsOptions};
use orchestrator::Orchestrator;
use parallel::PostArchiveAction;
use std::path::Path;
use tracing::{error, info, warn, Level};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

/// Helper: resolve the bearer token from CLI args for a TUI invocation.
fn resolve_tui_token(args: &TuiArgs) -> Option<String> {
    remote::RemoteClient::resolve_token(args.server_token.clone(), args.server_token_env.as_deref())
}

fn tui_post_archive_action(args: &TuiArgs) -> Result<PostArchiveAction> {
    if args.push.is_some() && args.server.is_some() {
        return Err(error::OrchestratorError::ConfigLoad(
            "--push is not supported with TUI --server mode".to_string(),
        ));
    }

    Ok(args
        .push
        .clone()
        .map(|remote| PostArchiveAction::PushToRemote { remote })
        .unwrap_or_default())
}

/// Helper: load the initial change list for remote mode.
///
/// Fetches the project+change list from the remote server and maps it to the
/// local `Change` type so the TUI can display it unchanged.
async fn load_remote_changes(args: &TuiArgs) -> Result<Vec<openspec::Change>> {
    let endpoint = args.server.as_deref().unwrap_or_default();
    let token = resolve_tui_token(args);
    let client = remote::RemoteClient::new(endpoint, token);
    let projects = client.list_projects().await?;
    Ok(remote::group_changes_by_project(&projects))
}

async fn launch_tui(args: TuiArgs) -> Result<()> {
    let post_archive_action = tui_post_archive_action(&args)?;

    init_logging(false)?;
    log_startup("tui");

    let config = OrchestratorConfig::load(args.config.as_deref())?;
    tui::log_deduplicator::configure_logging(config.get_logging());

    let changes = if args.server.is_some() {
        info!(
            "Remote TUI mode: connecting to {}",
            args.server.as_deref().unwrap_or("")
        );
        load_remote_changes(&args).await?
    } else {
        openspec::list_changes_native()?
    };

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

    let remote_client = args.server.clone().map(|endpoint| {
        let token = resolve_tui_token(&args);
        remote::RemoteClient::new(endpoint, token)
    });

    tui::run_tui_with_remote(
        changes,
        config,
        web_url,
        #[cfg(feature = "web-monitoring")]
        web_state_opt,
        remote_client,
        post_archive_action,
    )
    .await
}

/// Resolve the server URL for `cflx project` commands.
///
/// Priority:
/// 1. Explicit `--server <url>` argument
/// 2. Global config `server.bind` / `server.port`
/// 3. Default: `http://127.0.0.1:39876`
fn resolve_project_server_url(explicit: Option<&str>) -> String {
    if let Some(url) = explicit {
        return url.to_string();
    }
    let server_config = OrchestratorConfig::load_server_config_from_global();
    format!("http://{}:{}", server_config.bind, server_config.port)
}

/// Guard: `cflx project` does not support bearer token authentication.
///
/// Returns `Err` if the caller supplied `--server-token` / `--server-token-env`,
/// or if the global config has `server.auth.mode=bearer_token` for the resolved server.
fn check_project_auth_not_required(
    server_url: &str,
    explicit_server: bool,
) -> std::result::Result<(), String> {
    // Only check global-config auth when the URL was resolved from config (not explicit)
    if !explicit_server {
        let server_config = OrchestratorConfig::load_server_config_from_global();
        if matches!(server_config.auth.mode, config::ServerAuthMode::BearerToken) {
            return Err(format!(
                "The server at '{}' requires bearer token authentication, \
                 which is not supported by 'cflx project'. \
                 Use the TUI or provide an unauthenticated server URL with --server.",
                server_url
            ));
        }
    }
    Ok(())
}

/// Print project JSON value in human-readable form.
fn print_projects_human(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(projects) => {
            if projects.is_empty() {
                println!("No projects registered.");
                return;
            }
            for p in projects {
                print_project_human(p);
            }
        }
        serde_json::Value::Null => {
            println!("Done.");
        }
        other => print_project_human(other),
    }
}

fn print_project_human(p: &serde_json::Value) {
    let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("-");
    let url = p.get("remote_url").and_then(|v| v.as_str()).unwrap_or("-");
    let branch = p.get("branch").and_then(|v| v.as_str()).unwrap_or("-");
    let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("-");
    println!("id:         {}", id);
    println!("remote_url: {}", url);
    println!("branch:     {}", branch);
    println!("status:     {}", status);
    println!();
}

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
            .with_line_number(false)
            .with_filter(LevelFilter::INFO);

        registry.with(stdout_layer).init();
    } else {
        registry.init();
    }

    Ok(())
}

fn log_startup(mode: &str) {
    info!("Starting cflx {} mode={}.", VERSION_WITH_BUILD, mode);
}

fn run_completion_subcommand(args: cli::CompletionArgs) {
    let shell = clap_complete::Shell::from(args.shell);
    let mut command = Cli::command();
    let mut stdout = std::io::stdout();
    clap_complete::generate(shell, &mut command, "cflx", &mut stdout);
    print_dynamic_completion_hooks(args.shell);
}

fn print_dynamic_completion_hooks(shell: cli::CompletionShell) {
    match shell {
        cli::CompletionShell::Bash => print!("{}", BASH_DYNAMIC_COMPLETION_HOOK),
        cli::CompletionShell::Zsh => print!("{}", ZSH_DYNAMIC_COMPLETION_HOOK),
        cli::CompletionShell::Fish => print!("{}", FISH_DYNAMIC_COMPLETION_HOOK),
        cli::CompletionShell::PowerShell => print!("{}", POWERSHELL_DYNAMIC_COMPLETION_HOOK),
    }
}

fn run_internal_complete_subcommand(args: cli::InternalCompleteArgs) {
    match args.command {
        InternalCompleteCommands::ChangeIds(change_args) => {
            let scope = completion::ChangeIdCandidateScope::from_flags(
                change_args.active,
                change_args.archived,
            );
            let cwd = match std::env::current_dir() {
                Ok(cwd) => cwd,
                Err(_) => return,
            };
            for candidate in completion::discover_change_id_candidates(
                &cwd,
                scope,
                change_args.prefix.as_deref(),
            ) {
                println!("{candidate}");
            }
        }
    }
}

const BASH_DYNAMIC_COMPLETION_HOOK: &str = r#"

# cflx dynamic OpenSpec change-id completion hook
_cflx_static_completion() {
    _cflx "$@"
}
_cflx_dynamic_change_ids() {
    local scope="$1"
    local prefix="$2"
    local -a cmd=(cflx __complete change-ids --prefix "$prefix")
    case "$scope" in
        active) cmd+=(--active) ;;
        all) cmd+=(--active --archived) ;;
    esac
    mapfile -t COMPREPLY < <("${cmd[@]}" 2>/dev/null)
}
_cflx_dynamic_run_change_ids() {
    local current="${COMP_WORDS[COMP_CWORD]}"
    local prefix="${current##*,}"
    local before="${current%,*}"
    _cflx_dynamic_change_ids active "$prefix"
    if [[ "$before" != "$current" ]]; then
        local i
        for i in "${!COMPREPLY[@]}"; do COMPREPLY[$i]="$before,${COMPREPLY[$i]}"; done
    fi
}
_cflx_dynamic_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local prev="${COMP_WORDS[COMP_CWORD-1]}"
    if [[ "$prev" == "--change" ]]; then
        _cflx_dynamic_run_change_ids
        return
    fi
    if [[ ${COMP_CWORD} -ge 3 && "${COMP_WORDS[1]}" == "openspec" ]]; then
        case "${COMP_WORDS[2]}" in
            show)
                if [[ "$cur" != -* ]]; then
                    _cflx_dynamic_change_ids all "$cur"
                    return
                fi
                ;;
            validate|archive)
                if [[ "$cur" != -* ]]; then
                    _cflx_dynamic_change_ids active "$cur"
                    return
                fi
                ;;
        esac
    fi
    _cflx_static_completion "$@"
}
complete -F _cflx_dynamic_completion -o bashdefault -o default cflx
# Surfaces: cflx run --change -> _cflx_dynamic_run_change_ids; cflx openspec show -> active+archived;
# cflx openspec validate/archive -> active. Candidate command: cflx __complete change-ids
"#;

const ZSH_DYNAMIC_COMPLETION_HOOK: &str = r#"

# cflx dynamic OpenSpec change-id completion hook
_cflx_static_completion() {
    _cflx "$@"
}
_cflx_dynamic_change_ids() {
    local scope="$1"
    local prefix="$2"
    local -a cmd=(cflx __complete change-ids --prefix "$prefix")
    case "$scope" in
        active) cmd+=(--active) ;;
        all) cmd+=(--active --archived) ;;
    esac
    compadd -- "${(@f)$(${cmd[@]} 2>/dev/null)}"
}
_cflx_dynamic_run_change_ids() {
    local current="${words[CURRENT]}"
    local prefix="${current##*,}"
    local before="${current%,*}"
    local -a candidates
    candidates=("${(@f)$(cflx __complete change-ids --active --prefix "$prefix" 2>/dev/null)}")
    if [[ "$before" != "$current" ]]; then
        candidates=("${(@)^candidates/#/$before,}")
    fi
    compadd -- "${candidates[@]}"
}
_cflx_dynamic_completion() {
    if [[ "${words[CURRENT-1]}" == "--change" ]]; then
        _cflx_dynamic_run_change_ids
        return
    fi
    if [[ ${CURRENT} -ge 4 && "${words[2]}" == "openspec" ]]; then
        case "${words[3]}" in
            show)
                [[ "${words[CURRENT]}" == -* ]] || { _cflx_dynamic_change_ids all "${words[CURRENT]}"; return; }
                ;;
            validate|archive)
                [[ "${words[CURRENT]}" == -* ]] || { _cflx_dynamic_change_ids active "${words[CURRENT]}"; return; }
                ;;
        esac
    fi
    _cflx_static_completion "$@"
}
compdef _cflx_dynamic_completion cflx
# Surfaces: cflx run --change -> _cflx_dynamic_run_change_ids; cflx openspec show -> active+archived;
# cflx openspec validate/archive -> active. Candidate command: cflx __complete change-ids
"#;

const FISH_DYNAMIC_COMPLETION_HOOK: &str = r#"

# cflx dynamic OpenSpec change-id completion hook
function __cflx_dynamic_change_ids
    set -l scope $argv[1]
    set -l prefix $argv[2]
    set -l cmd cflx __complete change-ids --prefix "$prefix"
    switch $scope
        case active
            set cmd $cmd --active
        case all
            set cmd $cmd --active --archived
    end
    $cmd 2>/dev/null
end
complete -c cflx -n '__fish_seen_subcommand_from run; and __fish_seen_argument --change' -a '(__cflx_dynamic_change_ids active (string split -r -m1 , (commandline -ct))[-1])'
complete -c cflx -n '__fish_seen_subcommand_from openspec; and __fish_seen_subcommand_from show' -a '(__cflx_dynamic_change_ids all (commandline -ct))'
complete -c cflx -n '__fish_seen_subcommand_from openspec; and __fish_seen_subcommand_from validate archive' -a '(__cflx_dynamic_change_ids active (commandline -ct))'
"#;

const POWERSHELL_DYNAMIC_COMPLETION_HOOK: &str = r#"

# cflx dynamic OpenSpec change-id completion hook
function __CflxDynamicChangeIds($Scope, $Prefix) {
    $args = @('__complete', 'change-ids', '--prefix', $Prefix)
    if ($Scope -eq 'active') { $args += '--active' }
    if ($Scope -eq 'all') { $args += @('--active', '--archived') }
    & cflx @args 2>$null
}
function __CflxDynamicRunChangeIds($WordToComplete) {
    $prefix = $WordToComplete -replace '^.*,', ''
    $before = $WordToComplete -replace ',?[^,]*$', ''
    foreach ($candidate in (__CflxDynamicChangeIds active $prefix)) {
        if ($before) { "$before,$candidate" } else { $candidate }
    }
}
Register-ArgumentCompleter -Native -CommandName 'cflx' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $elements = @($commandAst.CommandElements | ForEach-Object { $_.Extent.Text })
    $commandText = $elements -join ' '
    if ($commandText -match '^cflx\s+run\b' -and ($elements -contains '--change')) {
        __CflxDynamicRunChangeIds $wordToComplete | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, 'OpenSpec active change ID')
        }
        return
    }
    if ($commandText -match '^cflx\s+openspec\s+show\b' -and $wordToComplete -notlike '-*') {
        __CflxDynamicChangeIds all $wordToComplete | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, 'OpenSpec change ID')
        }
        return
    }
    if ($commandText -match '^cflx\s+openspec\s+(validate|archive)\b' -and $wordToComplete -notlike '-*') {
        __CflxDynamicChangeIds active $wordToComplete | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, 'OpenSpec active change ID')
        }
        return
    }
}
# Surfaces: cflx run --change -> active comma-token candidates; cflx openspec show -> active+archived;
# cflx openspec validate/archive -> active. Candidate command: cflx __complete change-ids
"#;

fn run_logs_subcommand(args: LogsArgs) {
    let options = log_viewer::LogViewerOptions {
        print_path: args.path,
        last: args.last,
        follow: args.follow,
        today: args.today,
        project: args.project,
        repo_root: std::env::current_dir().ok(),
    };

    if let Err(e) = log_viewer::run_logs_command(&options, &mut std::io::stdout()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Completion commands intentionally run before logging/config/orchestration paths.
        Some(Commands::Completion(args)) => {
            run_completion_subcommand(args);
        }

        // Hidden candidate command intentionally runs before logging/config/orchestration paths.
        Some(Commands::Complete(args)) => {
            run_internal_complete_subcommand(args);
        }

        // No subcommand: launch TUI (default behavior)
        None => {
            launch_tui(TuiArgs {
                config: cli.config,
                web: cli.web,
                web_port: cli.web_port,
                web_bind: cli.web_bind,
                push: cli.push,
                server: cli.server,
                server_token: cli.server_token,
                server_token_env: cli.server_token_env,
            })
            .await?;
        }

        // TUI subcommand: launch interactive TUI dashboard
        Some(Commands::Tui(tui_args)) => launch_tui(tui_args).await?,

        // Run subcommand: non-interactive orchestration
        Some(Commands::Run(args)) => {
            // Initialize logging: include stdout for run mode
            init_logging(true)?;
            log_startup("run");

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

            // Handle for the web control bridge task; aborted on run completion.
            #[cfg(feature = "web-monitoring")]
            let mut web_bridge_handle: Option<tokio::task::JoinHandle<()>> = None;

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
                web_bridge_handle = Some(tokio::spawn(async move {
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
                }));
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

            // Clone normalized targets for use in restart loop: None means explicit --all.
            let change_ids = args.normalized_target_changes();
            let config_path = args.config.clone();
            let max_iterations = args.max_iterations;
            let max_concurrent = args.max_concurrent;
            let dry_run = args.dry_run;
            let no_resume = args.no_resume;
            let post_archive_action = args
                .push
                .clone()
                .map(|remote| parallel::PostArchiveAction::PushToRemote { remote })
                .unwrap_or_default();

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
                    post_archive_action.clone(),
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
                        // Successful completion — exit run mode immediately
                        info!("Orchestrator completed successfully");
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

            // Abort run-scoped web bridge task explicitly so cleanup does not
            // depend on Tokio runtime teardown ordering.
            #[cfg(feature = "web-monitoring")]
            if let Some(handle) = web_bridge_handle {
                handle.abort();
            }
        }

        // Logs subcommand: read-only persistent log viewer. Intentionally runs before
        // init_logging() so viewing logs never creates, appends, or cleans log files.
        Some(Commands::Logs(args)) => {
            run_logs_subcommand(args);
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

        // Server subcommand: start the multi-project server daemon
        #[cfg(feature = "web-monitoring")]
        Some(Commands::Server(args)) => {
            // Initialize logging (file + stdout)
            init_logging(true)?;
            log_startup("server");

            // Build ServerConfig from global config and CLI overrides.
            // Server mode uses only global config (not project .cflx.jsonc).
            // Global config is loaded first, then CLI args override individual fields.
            let (mut server_config, resolve_command, proposal_session_config) =
                config::OrchestratorConfig::load_server_config_and_resolve_command_from_global();

            // Apply CLI overrides on top of global config values.
            // Only override fields that were explicitly specified on the CLI
            // (None means "not specified", so global config value is preserved).
            server_config.apply_cli_overrides(
                args.bind.as_deref(),
                args.port,
                args.auth_token.as_deref(),
                args.max_concurrent_total,
                args.data_dir.as_deref(),
            );

            info!(
                "Starting server daemon on {}:{} (data_dir: {:?})",
                server_config.bind, server_config.port, server_config.data_dir
            );

            server::run_server(server_config, resolve_command, proposal_session_config).await?;
        }

        // Server subcommand (web-monitoring feature disabled)
        #[cfg(not(feature = "web-monitoring"))]
        Some(Commands::Server(_)) => {
            eprintln!(
                "Error: Server daemon requires the 'web-monitoring' feature. Compile with --features web-monitoring"
            );
            std::process::exit(1);
        }

        // Project subcommand: manage projects on a remote Conflux server
        Some(Commands::Project(args)) => {
            // Guard: top-level --server-token / --server-token-env are not supported
            if cli.server_token.is_some() || cli.server_token_env.is_some() {
                eprintln!(
                    "Error: --server-token and --server-token-env are not supported by \
                     'cflx project'. Authentication is not supported by this command."
                );
                std::process::exit(1);
            }

            let explicit_server = args.server.is_some();
            let server_url = resolve_project_server_url(args.server.as_deref());

            // Auth guard: project commands do not support bearer token auth
            if let Err(msg) = check_project_auth_not_required(&server_url, explicit_server) {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }

            // Project commands use an unauthenticated client (no token)
            let client = remote::RemoteClient::new(&server_url, None);

            let result: crate::error::Result<serde_json::Value> = match args.command {
                ProjectCommands::Add(add_args) => {
                    // Resolve (base_url, branch) using URL parsing + default branch resolution
                    let (base_url, branch) = match remote::resolve_project_url_and_branch(
                        &add_args.remote_url,
                        add_args.branch.as_deref(),
                        |url| async move { remote::resolve_default_branch(&url).await },
                    )
                    .await
                    {
                        Ok(pair) => pair,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    };
                    client.add_project(&base_url, &branch).await
                }
                ProjectCommands::Remove(remove_args) => {
                    client.delete_project(&remove_args.project_id).await
                }
                ProjectCommands::Status(status_args) => {
                    if let Some(ref id) = status_args.project_id {
                        client.get_project(id).await
                    } else {
                        client.list_projects_management().await
                    }
                }
                ProjectCommands::Sync(sync_args) => {
                    if sync_args.all {
                        // Sync all registered projects
                        let sync_server = sync_args.server.clone();
                        let sync_client = remote::RemoteClient::new(&sync_server, None);
                        let projects = match sync_client.list_all_projects().await {
                            Ok(p) => p,
                            Err(e) => {
                                eprintln!("Error listing projects: {}", e);
                                std::process::exit(1);
                            }
                        };

                        if projects.is_empty() {
                            println!("No projects registered. Nothing to sync.");
                            std::process::exit(0);
                        }

                        let mut any_failed = false;
                        for project in &projects {
                            match sync_client.sync_project(&project.id).await {
                                Ok(()) => {
                                    println!(
                                        "project {}: synced ({}#{})",
                                        project.id, project.remote_url, project.branch
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "project {}: failed to sync ({}#{}) - {}",
                                        project.id, project.remote_url, project.branch, e
                                    );
                                    any_failed = true;
                                }
                            }
                        }
                        if any_failed {
                            std::process::exit(1);
                        }
                        std::process::exit(0);
                    } else if let Some(ref project_id) = sync_args.project_id {
                        client.git_sync(project_id).await
                    } else {
                        eprintln!("Error: specify --all or a PROJECT_ID");
                        std::process::exit(1);
                    }
                }
            };

            match result {
                Ok(value) => {
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&value).unwrap_or_default()
                        );
                    } else {
                        print_projects_human(&value);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // Service subcommand: manage cflx server as a background service
        Some(Commands::Service(args)) => {
            use cli::ServiceSubcommand;
            let result = match args.command {
                ServiceSubcommand::Install => service::install(),
                ServiceSubcommand::Uninstall => service::uninstall(),
                ServiceSubcommand::Status => service::status(),
                ServiceSubcommand::Start => service::start(),
                ServiceSubcommand::Stop => service::stop(),
                ServiceSubcommand::Restart => service::restart(),
            };
            if let Err(e) = result {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }

        // install-skills subcommand: install agent skills
        Some(Commands::InstallSkills(args)) => {
            if let Some(src) = &args.legacy_source {
                eprintln!("{}", install_skills_legacy_error(src));
                std::process::exit(1);
            }
            let target = match args.target() {
                InstallSkillsTarget::Agents => install_skills::InstallTarget::Agents,
                InstallSkillsTarget::Claude => install_skills::InstallTarget::Claude,
            };
            let opts = InstallSkillsOptions {
                global: args.global,
                target,
                project_root: None, // use CWD at runtime
            };
            if let Err(e) = run_install_skills(opts) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }

        // Openspec subcommand: native OpenSpec utility operations
        Some(Commands::Openspec(args)) => {
            use cli::{EvidenceMode, OpenspecCommands};

            match args.command {
                OpenspecCommands::List(list_args) => {
                    if let Err(e) = crate::openspec_cmd::cmd_list(list_args.specs) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                OpenspecCommands::Show(show_args) => {
                    if let Err(e) = crate::openspec_cmd::cmd_show(
                        &show_args.change_id,
                        show_args.json,
                        show_args.deltas_only,
                    ) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                OpenspecCommands::Validate(val_args) => {
                    let strict = val_args.strict || val_args.archive_gate;
                    let evidence = if val_args.archive_gate {
                        "error"
                    } else {
                        match val_args.evidence {
                            EvidenceMode::Off => "off",
                            EvidenceMode::Warn => "warn",
                            EvidenceMode::Error => "error",
                        }
                    };
                    let (is_valid, exit_code) = crate::openspec_cmd::cmd_validate(
                        val_args.change_id.as_deref(),
                        strict,
                        evidence,
                    );
                    if !is_valid {
                        std::process::exit(exit_code);
                    }
                }
                OpenspecCommands::Archive(arc_args) => {
                    if !arc_args.yes {
                        eprintln!("Error: --yes flag is required (non-interactive only)");
                        std::process::exit(1);
                    }
                    if let Err(e) =
                        crate::openspec_cmd::cmd_archive(&arc_args.change_id, arc_args.skip_specs)
                    {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
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

#[cfg(test)]
mod project_command_tests {
    use super::*;

    /// When `--server` is specified explicitly the URL is returned as-is.
    #[test]
    fn test_resolve_project_server_url_explicit() {
        let url = resolve_project_server_url(Some("http://custom:1234"));
        assert_eq!(url, "http://custom:1234");
    }

    /// Without global config the URL falls back to the default (127.0.0.1:39876).
    #[test]
    fn test_resolve_project_server_url_default_fallback() {
        let url = resolve_project_server_url(None);
        assert!(
            url.starts_with("http://"),
            "Expected http URL, got: {}",
            url
        );
        assert!(url.contains(':'), "URL should contain a port: {}", url);
    }

    /// When `explicit_server=true` the auth guard always passes (no global config check).
    #[test]
    fn test_check_project_auth_explicit_server_always_passes() {
        let result = check_project_auth_not_required("http://custom:1234", true);
        assert!(result.is_ok(), "Should pass for explicit server URL");
    }

    /// Without global config the auth mode defaults to None, so the guard passes.
    #[test]
    fn test_check_project_auth_default_no_auth_passes() {
        let result = check_project_auth_not_required("http://127.0.0.1:39876", false);
        assert!(result.is_ok(), "Should pass when auth mode is None");
    }
}
