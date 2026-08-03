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
mod lifecycle_integration;
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
#[allow(dead_code)]
mod repo_lock;
#[allow(dead_code, unused_imports)]
mod runtime;
mod serial_run_service;
mod shell_command;
mod spec_delta;
#[cfg(test)]
mod spec_test_annotations;
mod stall;
mod stream_json_textifier;
mod task_parser;
mod templates;
mod tui;
#[allow(dead_code, unused_imports)]
mod upstream;
mod vcs;
#[cfg(feature = "web-monitoring")]
mod web;
mod worktree_ops;

#[cfg(test)]
mod test_support;

use clap::{CommandFactory, Parser};
use cli::{
    install_skills_legacy_error, Cli, Commands, InstallSkillsTarget, InternalCompleteCommands,
    LogsArgs, TuiArgs, VERSION_WITH_BUILD,
};
use config::OrchestratorConfig;
use error::Result;
use install_skills::{run_install_skills, InstallSkillsOptions};
use lifecycle_integration::{
    LifecycleContext, LifecycleEvent, LifecycleExecutionMode, LifecycleIntegration, LifecycleState,
};
use orchestrator::Orchestrator;
use parallel::PostArchiveAction;
use std::path::Path;
use tracing::{error, info, Level};
use tracing_subscriber::{filter::LevelFilter, prelude::*};

fn tui_post_archive_action(args: &TuiArgs) -> PostArchiveAction {
    args.push
        .clone()
        .map(|remote| PostArchiveAction::PushToRemote { remote })
        .unwrap_or_default()
}

/// Privacy-safe lifecycle context identifying this cflx process.
///
/// Only the workspace root is reported; no environment, configuration, or
/// command content is ever included.
fn lifecycle_process_context() -> LifecycleContext {
    match std::env::current_dir() {
        Ok(dir) => LifecycleContext::workspace(dir.display().to_string()),
        Err(_) => LifecycleContext::default(),
    }
}

/// Validate and construct the local TUI's upstream runtime before any TUI or
/// orchestration state exists.
///
/// This is deliberately the same contract as `cflx run`: it resolves the option
/// through the shared frontend normalizer, then runs the same static and
/// initial-fetch validation.
///
/// The default-off path additionally refuses to start while repository evidence
/// proves an unpublished opted-in integration, which is the option-less restart
/// refusal. That scan is offline, so a disabled TUI gains no network access.
async fn resolve_tui_upstream_runtime(
    args: &TuiArgs,
    config: &OrchestratorConfig,
) -> Option<upstream::UpstreamRuntime> {
    let upstream_config = match args.upstream_integration() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };

    let git_dir_exists = cli::check_git_directory();
    // Local TUI upstream publication is a cumulative parallel capability; the
    // effective mode is resolved exactly as the TUI itself resolves it.
    let use_parallel = config.resolve_parallel_mode(false, git_dir_exists);

    let repo_root = match std::env::current_dir() {
        Ok(root) => root,
        Err(err) => {
            if upstream_config.is_some() {
                eprintln!("Error: upstream integration requires a readable workspace: {err}");
                std::process::exit(1);
            }
            return None;
        }
    };

    match upstream_config {
        Some(upstream_config) => {
            match upstream::prepare_upstream_integration(
                upstream_config,
                &repo_root,
                use_parallel,
                args.push.clone(),
                git_dir_exists,
                false,
            )
            .await
            {
                Ok(runtime) => Some(runtime),
                Err(err) => {
                    eprintln!("Error: {err}");
                    std::process::exit(1);
                }
            }
        }
        None => {
            if use_parallel && git_dir_exists {
                if let Err(err) = upstream::ensure_no_unpushed_upstream_recovery(&repo_root).await {
                    if matches!(err, upstream::UpstreamStartupError::Invalid(_)) {
                        eprintln!("Error: {err}");
                        std::process::exit(1);
                    }
                    tracing::debug!("Upstream recovery scan unavailable, continuing: {}", err);
                }
            }
            None
        }
    }
}

async fn launch_tui(args: TuiArgs) -> Result<()> {
    let post_archive_action = tui_post_archive_action(&args);

    init_logging(false)?;
    log_startup("tui");

    let config = OrchestratorConfig::load(args.config.as_deref())?;
    tui::log_deduplicator::configure_logging(config.get_logging());

    // Startup validation runs before the terminal is taken over, so a rejected
    // invocation reports plainly and leaves no orchestration state behind.
    let upstream_runtime = resolve_tui_upstream_runtime(&args, &config).await;

    // Start the optional external lifecycle adapter before the interactive TUI
    // is presented. Failures here are observability-only and never block startup.
    let lifecycle = LifecycleIntegration::start(
        config.get_lifecycle_integration(),
        LifecycleExecutionMode::Tui,
    );
    lifecycle.handle().publish(LifecycleEvent::ProcessStarted {
        context: lifecycle_process_context(),
    });

    let result = launch_tui_inner(
        args,
        config,
        post_archive_action,
        upstream_runtime,
        lifecycle.handle(),
    )
    .await;

    lifecycle.shutdown().await;

    result
}

async fn launch_tui_inner(
    args: TuiArgs,
    config: OrchestratorConfig,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<upstream::UpstreamRuntime>,
    lifecycle: lifecycle_integration::LifecycleHandle,
) -> Result<()> {
    let changes = openspec::list_changes_native()?;

    #[cfg(feature = "web-monitoring")]
    let (web_url, web_state_opt) = if args.web {
        let web_state = std::sync::Arc::new(web::WebState::new(&changes));
        let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone()).with_auth(
            args.web_auth_token.clone(),
            args.web_auth_token_env.clone(),
            args.web_allowed_origins.clone(),
        );
        // An unsafe or contradictory web configuration is a hard startup error.
        // Degrading to "no web server" would leave an operator who asked for a
        // routable listener believing they had one.
        if let Err(error) = web_config.validate() {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
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

    tui::run_tui(
        changes,
        config,
        web_url,
        #[cfg(feature = "web-monitoring")]
        web_state_opt,
        post_archive_action,
        upstream_runtime,
        lifecycle,
    )
    .await
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

/// Classify the parsed CLI invocation for repository-lock purposes.
fn repository_lock_invocation(cli: &Cli) -> repo_lock::InvocationKind {
    match &cli.command {
        None => repo_lock::InvocationKind::DefaultTui,
        Some(Commands::Tui(_)) => repo_lock::InvocationKind::Tui,
        Some(Commands::Run(_)) => repo_lock::InvocationKind::Run,
        _ => repo_lock::InvocationKind::Other,
    }
}

/// Take the repository orchestration lock before any startup side effect.
///
/// Runs before logging adapters, listeners, AI subprocesses, and orchestration
/// so a rejected invocation leaves the repository and the existing owner
/// completely untouched. The acquired lock is retained for the process
/// lifetime; the OS releases it when the process exits, normally or not.
fn acquire_repository_lock(cli: &Cli) {
    let kind = repository_lock_invocation(cli);
    let mode = match repo_lock::classify_invocation(kind) {
        repo_lock::LockDecision::Bypass => return,
        repo_lock::LockDecision::Acquire(mode) => mode,
    };

    let Ok(workspace) = std::env::current_dir() else {
        return;
    };

    match repo_lock::acquire(&workspace, mode) {
        // No Git repository here, so there is no repository identity to guard.
        Ok(None) => {}
        Ok(Some(lock)) => repo_lock::install(lock),
        Err(err @ repo_lock::LockError::Conflict(_)) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
        // An unusable lock file proves no conflict, so refusing to start would
        // be a worse failure than continuing without exclusion.
        Err(err) => {
            eprintln!("Warning: repository lock unavailable, continuing: {err}");
        }
    }
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

    // Top-level upstream options only reach the bare local TUI. An explicit
    // subcommand parses its own copies, so accepting them here would silently
    // drop an opt-in whose publication is part of the success contract. This is
    // a pure usage check, so rejecting here still leaves the repository and any
    // existing lock owner untouched.
    if let Err(error) = cli.validate_upstream_option_placement() {
        eprintln!("Error: {error}");
        std::process::exit(2);
    }

    // Repository exclusion is decided before anything observable happens.
    acquire_repository_lock(&cli);

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
                web_auth_token: cli.web_auth_token,
                web_auth_token_env: cli.web_auth_token_env,
                web_allowed_origins: cli.web_allowed_origins,
                push: cli.push,
                // Bare local TUI carries the same upstream contract as `cflx tui`.
                integrate_upstream: cli.integrate_upstream,
                integrate_upstream_default_remote: cli.integrate_upstream_default_remote,
                upstream_verify_command: cli.upstream_verify_command,
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
                let web_config = web::WebConfig::enabled(args.web_port, args.web_bind.clone())
                    .with_auth(
                        args.web_auth_token.clone(),
                        args.web_auth_token_env.clone(),
                        args.web_allowed_origins.clone(),
                    );
                // An unsafe or contradictory web configuration is a hard startup error.
                // Degrading to "no web server" would leave an operator who asked for a
                // routable listener believing they had one.
                if let Err(error) = web_config.validate() {
                    eprintln!("Error: {error}");
                    std::process::exit(1);
                }
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

            // Opt-in upstream integration. When the option is absent no upstream
            // object is constructed, so the existing execution path performs no
            // additional fetch, merge, verification, event, or push.
            let upstream_integration = match args.upstream_integration() {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            };

            let upstream_runtime = match upstream_integration {
                Some(config) => {
                    let repo_root = std::env::current_dir()?;
                    match upstream::prepare_upstream_integration(
                        config,
                        &repo_root,
                        use_parallel,
                        args.push.clone(),
                        git_dir_exists,
                        args.dry_run,
                    )
                    .await
                    {
                        Ok(runtime) => Some(runtime),
                        Err(err) => {
                            eprintln!("Error: {}", err);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    // Default-off path: refuse to continue only when repository
                    // evidence proves an unpushed upstream merge is reachable.
                    // The scan is offline, so nothing new is fetched here.
                    if use_parallel && !args.dry_run && git_dir_exists {
                        let repo_root = std::env::current_dir()?;
                        if let Err(err) =
                            upstream::ensure_no_unpushed_upstream_recovery(&repo_root).await
                        {
                            if matches!(err, upstream::UpstreamStartupError::Invalid(_)) {
                                eprintln!("Error: {}", err);
                                std::process::exit(1);
                            }
                            tracing::debug!(
                                "Upstream recovery scan unavailable, continuing: {}",
                                err
                            );
                        }
                    }
                    None
                }
            };

            // Non-interactive run reports process and orchestration lifecycle
            // through the same observability-only contract as the TUI. Started
            // after startup validation so a rejected invocation never leaves an
            // adapter process behind.
            let lifecycle = LifecycleIntegration::start(
                config.get_lifecycle_integration(),
                LifecycleExecutionMode::Run,
            );
            let lifecycle_handle = lifecycle.handle();
            let workspace_context = lifecycle_process_context().workspace;
            lifecycle_handle.publish(LifecycleEvent::ProcessStarted {
                context: lifecycle_process_context(),
            });

            // Run mode control state for web control integration
            // Run mode now supports retry and resume via outer loop.
            use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
            use std::sync::Arc;

            // Control state: 0 = Stopped, 1 = Running, 2 = Stopping
            let run_state = Arc::new(AtomicU8::new(1)); // Start in Running state
            let graceful_stop_flag = Arc::new(AtomicBool::new(false));
            let force_stop_flag = Arc::new(AtomicBool::new(false));
            let restart_requested = Arc::new(AtomicBool::new(false));

            // `cflx run --web` has no control bridge: `/api/v2` executes every
            // lifecycle command through the shared run-control service instead of
            // enqueueing it onto a process-local channel, so there is nothing left
            // for a bridge task to receive.

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
                lifecycle_handle
                    .publish_state(LifecycleState::Working, lifecycle_process_context());
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

                orchestrator
                    .set_lifecycle_handle(lifecycle_handle.clone(), workspace_context.clone());

                // Invocation-scoped; reinstalled on every restart-loop iteration so
                // an enabled run keeps its selected remote, base branch, and
                // verification command across orchestrator reconstruction.
                if let Some(runtime) = upstream_runtime.clone() {
                    orchestrator.set_upstream_integration(runtime);
                }

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
                        // An error state waits for an operator retry decision.
                        lifecycle_handle
                            .publish_state(LifecycleState::Blocked, lifecycle_process_context());

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
                                lifecycle.shutdown().await;
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

            lifecycle.shutdown().await;
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
