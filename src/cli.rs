use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::debug;

/// Get version string with build number
const fn get_version_string() -> &'static str {
    concat!(
        "v",
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("BUILD_NUMBER"),
        ")"
    )
}

/// OpenSpec Orchestrator - Automate OpenSpec workflow
#[derive(Parser, Debug)]
#[command(name = "cflx")]
#[command(version = get_version_string())]
#[command(about = "Automates OpenSpec change workflow (list → apply → archive)")]
#[command(long_about = "Conflux - OpenSpec Change Orchestrator

Automates the OpenSpec change workflow:
  1. Lists pending changes in openspec/changes/
  2. Applies changes using configured AI agent
  3. Archives completed changes to openspec/specs/

SUBCOMMANDS:
  run      Execute orchestration loop (non-interactive)
  tui      Launch interactive TUI dashboard (default)
  init     Generate configuration template

KEY OPTIONS:
  --parallel            Enable parallel execution using git worktrees
  --max-concurrent N    Limit concurrent workspaces (default: 3)
  --dry-run             Preview parallelization groups without execution
  --vcs BACKEND         VCS backend: auto, git (default: auto)
  --web                 Enable web monitoring server
  --web-port PORT       Web server port (default: 0 = auto-assign)
  --web-bind ADDR       Web server bind address (default: 127.0.0.1)
  --server URL          Connect TUI to a remote Conflux server
  --server-token TOKEN  Bearer token for remote server authentication
  --server-token-env VAR  Environment variable holding the bearer token

Use 'cflx <subcommand> --help' for more information on a specific command.")]
#[command(subcommand_required(false))]
pub struct Cli {
    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Enable web monitoring server for remote status viewing
    #[arg(long)]
    pub web: bool,

    /// Port for web monitoring server (default: 0 = auto-assign by OS)
    #[arg(long, default_value = "0")]
    pub web_port: u16,

    /// Bind address for web monitoring server (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub web_bind: String,

    /// Remote server endpoint URL (e.g., http://host:9876). When set, TUI connects to
    /// a remote Conflux server instead of the local workspace.
    #[arg(long)]
    pub server: Option<String>,

    /// Bearer token for authenticating with the remote server
    #[arg(long)]
    pub server_token: Option<String>,

    /// Name of the environment variable that holds the bearer token for the remote server
    #[arg(long)]
    pub server_token_env: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the OpenSpec change orchestration loop (non-interactive)
    Run(RunArgs),

    /// Launch the interactive TUI dashboard
    ///
    /// Key bindings: Space (select), F5 (start), Esc (stop), Tab (switch view), q (quit)
    Tui(TuiArgs),

    /// Initialize a new configuration file
    Init(InitArgs),

    /// Check for conflicts between spec delta files across changes
    CheckConflicts(CheckConflictsArgs),

    /// Start the multi-project server daemon
    ///
    /// Manages multiple projects via a REST API (API v1).
    /// Requires bearer token authentication when binding to non-loopback addresses.
    Server(ServerArgs),

    /// Manage projects on a Conflux server
    ///
    /// Interacts with the server's project management API without authentication.
    /// When --server is not specified, the server URL is resolved from global config
    /// (server.bind / server.port).
    ///
    /// EXAMPLES:
    ///   cflx project add https://github.com/org/repo             # auto-resolve default branch
    ///   cflx project add https://github.com/org/repo/tree/main  # branch from URL path
    ///   cflx project add https://github.com/org/repo#develop    # branch from fragment
    ///   cflx project add https://github.com/org/repo main       # explicit branch argument
    ///   cflx project status                                      # list all projects
    ///   cflx project status <project-id>                         # show specific project
    ///   cflx project remove <project-id>                         # remove a project
    ///   cflx project sync <project-id>                           # trigger git sync
    Project(ProjectArgs),

    /// Manage `cflx server` as a background service
    ///
    /// Installs, uninstalls, starts, stops, restarts, and queries the status of the
    /// `cflx server` daemon using the native service manager for your OS:
    ///   - macOS: launchd user agent
    ///   - Linux: systemd user service
    ///   - Windows: Scheduled Task
    ///
    /// EXAMPLES:
    ///   cflx service install    # Install and enable the service
    ///   cflx service start      # Start the service
    ///   cflx service status     # Show current status
    ///   cflx service stop       # Stop the service
    ///   cflx service restart    # Restart the service
    ///   cflx service uninstall  # Remove the service
    Service(ServiceArgs),
}

/// Arguments for the run subcommand
#[derive(Parser, Debug)]
#[command(
    long_about = "Execute the OpenSpec change orchestration loop in non-interactive mode.

This mode processes changes sequentially or in parallel (with --parallel flag),
applying each change using the configured AI agent and archiving when complete.

PARALLEL EXECUTION:
  --parallel enables concurrent processing using git worktrees. Changes are
  analyzed for dependencies and executed in optimal parallel groups.

WEB MONITORING:
  --web enables remote monitoring via HTTP. Access progress from any browser
  while orchestration runs in background.

EXAMPLES:
  cflx run                           # Process all changes
  cflx run --change my-feature       # Process specific change
  cflx run --parallel --max-concurrent 5  # Parallel with 5 workers
  cflx run --parallel --dry-run      # Preview parallelization plan
  cflx run --web --web-port 8080     # Enable web monitoring on port 8080"
)]
pub struct RunArgs {
    /// Process only the specified changes (comma-separated, e.g., --change a,b,c)
    #[arg(long, value_delimiter = ',')]
    pub change: Option<Vec<String>>,

    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Maximum number of iterations for the orchestration loop (overrides config, 0 = no limit)
    #[arg(long)]
    pub max_iterations: Option<u32>,

    /// Enable parallel execution mode using git worktrees
    #[arg(long)]
    pub parallel: bool,

    /// Maximum number of concurrent workspaces for parallel execution
    #[arg(long)]
    pub max_concurrent: Option<usize>,

    /// Preview parallelization groups without executing (dry run)
    #[arg(long)]
    pub dry_run: bool,

    /// VCS backend for parallel execution: auto or git
    /// Default: auto (detects git repository)
    #[arg(long, default_value = "auto")]
    pub vcs: String,

    /// Disable automatic workspace resume. When set, always create new
    /// workspaces instead of reusing existing ones from interrupted runs.
    #[arg(long)]
    pub no_resume: bool,

    /// Enable web monitoring server for remote status viewing
    #[arg(long)]
    pub web: bool,

    /// Port for web monitoring server (default: 0 = auto-assign by OS)
    #[arg(long, default_value = "0")]
    pub web_port: u16,

    /// Bind address for web monitoring server (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub web_bind: String,
}

/// Arguments for the TUI subcommand
#[derive(Parser, Debug)]
#[command(long_about = "Launch the interactive Terminal UI dashboard.

The TUI provides real-time visualization of change processing with:
  • Change selection and queue management
  • Live progress tracking with task completion percentages
  • Streaming logs from AI agent execution

  • Git worktree visualization and management
  • Parallel execution monitoring

KEY BINDINGS:
  Space     Toggle change selection/queue status
  F5        Start/resume processing
  Esc       Stop processing (press twice to force)
  Tab       Switch between Changes/Worktrees view
  q         Quit

WEB MONITORING:
  --web enables simultaneous web-based monitoring alongside the TUI.

REMOTE SERVER:
  --server connects the TUI to a remote Conflux server instead of the local workspace.
  --server-token provides the bearer token for authentication.
  --server-token-env reads the token from the named environment variable.

EXAMPLES:
  cflx tui                                        # Launch TUI (default when no subcommand)
  cflx tui --web                                  # TUI with web monitoring enabled
  cflx tui --server http://host:9876              # Connect to remote server
  cflx tui --server http://host:9876 --server-token mytoken  # With bearer auth")]
pub struct TuiArgs {
    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Enable web monitoring server for remote status viewing
    #[arg(long)]
    pub web: bool,

    /// Port for web monitoring server (default: 0 = auto-assign by OS)
    #[arg(long, default_value = "0")]
    pub web_port: u16,

    /// Bind address for web monitoring server (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub web_bind: String,

    /// Remote server endpoint URL (e.g., http://host:9876). When set, TUI connects to
    /// a remote Conflux server instead of the local workspace.
    #[arg(long)]
    pub server: Option<String>,

    /// Bearer token for authenticating with the remote server
    #[arg(long)]
    pub server_token: Option<String>,

    /// Name of the environment variable that holds the bearer token for the remote server
    #[arg(long)]
    pub server_token_env: Option<String>,
}

/// Template options for init command
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum Template {
    /// Claude Code agent (claude --dangerously-skip-permissions)
    #[default]
    Claude,
    /// OpenCode agent
    Opencode,
    /// Codex agent
    Codex,
}

/// Arguments for the init subcommand
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Template to use for configuration
    #[arg(long, short = 't', value_enum, default_value_t = Template::Claude)]
    pub template: Template,

    /// Overwrite existing configuration file
    #[arg(long, short = 'f')]
    pub force: bool,
}

/// Arguments for the check-conflicts subcommand
#[derive(Parser, Debug)]
pub struct CheckConflictsArgs {
    /// Output results in JSON format
    #[arg(long, short = 'j')]
    pub json: bool,
}

/// Arguments for the server subcommand
#[derive(Parser, Debug)]
#[command(long_about = "Start the multi-project server daemon.

The server daemon runs independently of any particular directory and manages
multiple projects via a REST API. Projects are identified by remote_url + branch.

SECURITY:
  When binding to a non-loopback address, bearer token authentication is required.
  The server will refuse to start if --auth-token is not provided for non-loopback binds.

EXAMPLES:
  cflx server                                    # Start on 127.0.0.1:9876
  cflx server --port 9876                        # Explicit port
  cflx server --bind 0.0.0.0 --auth-token mytoken  # Public bind with auth
  cflx server --data-dir /var/lib/cflx           # Custom data directory")]
pub struct ServerArgs {
    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<std::path::PathBuf>,

    /// Bind address for the server (overrides global config; default from global config or 127.0.0.1)
    #[arg(long)]
    pub bind: Option<String>,

    /// Port for the server (overrides global config; default from global config or 9876)
    #[arg(long)]
    pub port: Option<u16>,

    /// Bearer token for authentication (required for non-loopback bind addresses)
    #[arg(long)]
    pub auth_token: Option<String>,

    /// Maximum number of concurrent project executions globally
    #[arg(long)]
    pub max_concurrent_total: Option<usize>,

    /// Directory for persistent server data (projects registry, etc.)
    #[arg(long)]
    pub data_dir: Option<std::path::PathBuf>,
}

/// Arguments for the project subcommand
#[derive(Parser, Debug)]
#[command(long_about = "Manage projects on a Conflux server.

Connects to a Conflux server and manages projects via the REST API.
When --server is not specified, the URL is resolved from the global
configuration (server.bind / server.port, defaulting to 127.0.0.1:9876).

Authentication is not supported by this command. If the server requires
bearer token authentication, an explicit error is returned.

EXAMPLES:
  cflx project add https://github.com/org/repo.git main
  cflx project status
  cflx project status <project-id>
  cflx project remove <project-id>
  cflx project sync <project-id>
  cflx project --server http://host:9876 status")]
pub struct ProjectArgs {
    /// Remote server endpoint URL (e.g., http://host:9876).
    /// When not set, resolved from global config server.bind/server.port.
    #[arg(long)]
    pub server: Option<String>,

    /// Output results in JSON format
    #[arg(long, short = 'j')]
    pub json: bool,

    #[command(subcommand)]
    pub command: ProjectCommands,
}

/// Subcommands for the project command
#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Add a new project to the server
    Add(ProjectAddArgs),

    /// Remove a project from the server
    Remove(ProjectRemoveArgs),

    /// Show project status (all projects or a specific one)
    Status(ProjectStatusArgs),

    /// Trigger a git sync (pull + push) for a project
    Sync(ProjectSyncArgs),
}

/// Arguments for `cflx project add`
#[derive(Parser, Debug)]
#[command(about = "Add a project to the server")]
#[command(long_about = "Add a project to the Conflux server.

Accepts repository URLs with optional branch specification embedded in the URL:
  cflx project add https://github.com/org/repo             # auto-resolve default branch
  cflx project add https://github.com/org/repo/tree/main  # branch from /tree/<branch> path
  cflx project add https://github.com/org/repo#develop    # branch from #<branch> fragment
  cflx project add https://github.com/org/repo main       # explicit branch argument

When both a branch is embedded in the URL and an explicit branch argument is given,
the explicit argument takes precedence.")]
pub struct ProjectAddArgs {
    /// Repository URL (may include branch as /tree/<branch> or #<branch>)
    pub remote_url: String,

    /// Branch name (overrides any branch embedded in the URL; auto-resolved if omitted)
    pub branch: Option<String>,
}

/// Arguments for `cflx project remove`
#[derive(Parser, Debug)]
pub struct ProjectRemoveArgs {
    /// Project ID to remove
    pub project_id: String,
}

/// Arguments for `cflx project status`
#[derive(Parser, Debug)]
pub struct ProjectStatusArgs {
    /// Optional project ID (if omitted, lists all projects)
    pub project_id: Option<String>,
}

/// Arguments for `cflx project sync`
#[derive(Parser, Debug)]
pub struct ProjectSyncArgs {
    /// Sync all registered projects. Mutually exclusive with PROJECT_ID.
    #[arg(long, conflicts_with = "project_id")]
    pub all: bool,

    /// Project ID to sync. Mutually exclusive with --all.
    pub project_id: Option<String>,

    /// Remote server endpoint URL (default: http://127.0.0.1:9876)
    #[arg(long, default_value = "http://127.0.0.1:9876")]
    pub server: String,
}

/// Subcommands for the `cflx service` command group.
#[derive(Subcommand, Debug)]
pub enum ServiceSubcommand {
    /// Install `cflx server` as a background service (macOS: launchd, Linux: systemd, Windows: schtasks)
    Install,
    /// Uninstall the background service
    Uninstall,
    /// Show the current status of the background service
    Status,
    /// Start the background service
    Start,
    /// Stop the background service
    Stop,
    /// Restart the background service
    Restart,
}

/// Arguments for the `service` subcommand group.
#[derive(Parser, Debug)]
#[command(about = "Manage cflx server as a background service")]
pub struct ServiceArgs {
    #[command(subcommand)]
    pub command: ServiceSubcommand,
}

/// Check if git directory exists
pub fn check_git_directory() -> bool {
    std::path::Path::new(".git").exists()
}

/// Check if git CLI is available
pub fn check_git_available() -> bool {
    debug!(
        module = module_path!(),
        "Executing git command: git --version (cwd: {:?})",
        std::env::current_dir().ok()
    );
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if parallel execution is available (git)
pub fn check_parallel_available() -> bool {
    check_git_directory() && check_git_available()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_run_subcommand_config_option() {
        let cli = Cli::parse_from(["cflx", "run", "--config", "/path/to/config.jsonc"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.config, Some(PathBuf::from("/path/to/config.jsonc")));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_change_option() {
        let cli = Cli::parse_from(["cflx", "run", "--change", "add-feature-x"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.change, Some(vec!["add-feature-x".to_string()]));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_multiple_changes_comma_separated() {
        let cli = Cli::parse_from(["cflx", "run", "--change", "a,b,c"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(
                    args.change,
                    Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
                );
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_multiple_changes_with_spaces() {
        // Test that spaces around commas are handled
        let cli = Cli::parse_from(["cflx", "run", "--change", "a, b, c"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                // clap preserves spaces - trimming should be done by application logic if needed
                assert!(args.change.is_some());
                let changes = args.change.unwrap();
                assert_eq!(changes.len(), 3);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_no_change_option() {
        let cli = Cli::parse_from(["cflx", "run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.change.is_none());
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_no_subcommand() {
        let cli = Cli::parse_from(["cflx"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_init_subcommand_default_template() {
        let cli = Cli::parse_from(["cflx", "init"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Claude));
                assert!(!args.force);
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_opencode_template() {
        let cli = Cli::parse_from(["cflx", "init", "--template", "opencode"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Opencode));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_claude_template() {
        let cli = Cli::parse_from(["cflx", "init", "--template", "claude"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Claude));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_codex_template() {
        let cli = Cli::parse_from(["cflx", "init", "--template", "codex"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Codex));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_short_template_flag() {
        let cli = Cli::parse_from(["cflx", "init", "-t", "opencode"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Opencode));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_force_flag() {
        let cli = Cli::parse_from(["cflx", "init", "--force"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(args.force);
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_short_force_flag() {
        let cli = Cli::parse_from(["cflx", "init", "-f"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(args.force);
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_version_flag_exits_with_display_version() {
        // --version flag should cause parse to return an error (DisplayVersion)
        let result = Cli::try_parse_from(["cflx", "--version"]);
        assert!(result.is_err());

        let err = result.unwrap_err();
        // clap returns DisplayVersion error kind for --version
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_short_version_flag() {
        // -V flag should also display version
        let result = Cli::try_parse_from(["cflx", "-V"]);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_run_subcommand_max_iterations_default() {
        let cli = Cli::parse_from(["cflx", "run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.max_iterations.is_none());
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_custom() {
        let cli = Cli::parse_from(["cflx", "run", "--max-iterations", "100"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(100));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_zero() {
        let cli = Cli::parse_from(["cflx", "run", "--max-iterations", "0"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(0));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_parallel_flag_default() {
        let cli = Cli::parse_from(["cflx", "run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(!args.parallel);
                assert!(args.max_concurrent.is_none());
                assert!(!args.dry_run);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_parallel_flag_enabled() {
        let cli = Cli::parse_from(["cflx", "run", "--parallel"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_concurrent() {
        let cli = Cli::parse_from(["cflx", "run", "--parallel", "--max-concurrent", "5"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
                assert_eq!(args.max_concurrent, Some(5));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_dry_run() {
        let cli = Cli::parse_from(["cflx", "run", "--parallel", "--dry-run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
                assert!(args.dry_run);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_web_port_default_auto_assign() {
        let cli = Cli::parse_from(["cflx", "run", "--web"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.web);
                assert_eq!(args.web_port, 0); // Default: OS auto-assigns port
                assert_eq!(args.web_bind, "127.0.0.1");
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_web_port_explicit() {
        let cli = Cli::parse_from(["cflx", "run", "--web", "--web-port", "9000"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.web);
                assert_eq!(args.web_port, 9000);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_tui_subcommand_web_port_default_auto_assign() {
        let cli = Cli::parse_from(["cflx", "tui", "--web"]);

        match cli.command {
            Some(Commands::Tui(args)) => {
                assert!(args.web);
                assert_eq!(args.web_port, 0); // Default: OS auto-assigns port
                assert_eq!(args.web_bind, "127.0.0.1");
            }
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn test_no_subcommand_with_web() {
        // Note: Current CLI design requires explicit subcommand for web options.
        // The --web flag is only valid with 'run' or 'tui' subcommands.
        // This test verifies that web options work correctly with TUI subcommand.

        let cli = Cli::parse_from(["cflx", "tui", "--web"]);

        match cli.command {
            Some(Commands::Tui(args)) => {
                assert!(args.web);
                assert_eq!(args.web_port, 0); // Default: OS auto-assigns port
                assert_eq!(args.web_bind, "127.0.0.1");
            }
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn test_check_conflicts_subcommand_default() {
        let cli = Cli::parse_from(["cflx", "check-conflicts"]);

        match cli.command {
            Some(Commands::CheckConflicts(args)) => {
                assert!(!args.json);
            }
            _ => panic!("Expected CheckConflicts subcommand"),
        }
    }

    #[test]
    fn test_check_conflicts_subcommand_json_flag() {
        let cli = Cli::parse_from(["cflx", "check-conflicts", "--json"]);

        match cli.command {
            Some(Commands::CheckConflicts(args)) => {
                assert!(args.json);
            }
            _ => panic!("Expected CheckConflicts subcommand"),
        }
    }

    #[test]
    fn test_check_conflicts_subcommand_short_json_flag() {
        let cli = Cli::parse_from(["cflx", "check-conflicts", "-j"]);

        match cli.command {
            Some(Commands::CheckConflicts(args)) => {
                assert!(args.json);
            }
            _ => panic!("Expected CheckConflicts subcommand"),
        }
    }

    // Tests for top-level --server / --server-token / --server-token-env options
    #[test]
    fn test_top_level_server_option() {
        // Regression: `cflx --server http://...` must not fail with "unexpected argument"
        let cli = Cli::try_parse_from(["cflx", "--server", "http://127.0.0.1:9876"]).unwrap();
        assert_eq!(cli.server, Some("http://127.0.0.1:9876".to_string()));
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_top_level_server_token_option() {
        let cli = Cli::try_parse_from([
            "cflx",
            "--server",
            "http://host:9876",
            "--server-token",
            "mytoken",
        ])
        .unwrap();
        assert_eq!(cli.server, Some("http://host:9876".to_string()));
        assert_eq!(cli.server_token, Some("mytoken".to_string()));
    }

    #[test]
    fn test_top_level_server_token_env_option() {
        let cli = Cli::try_parse_from([
            "cflx",
            "--server",
            "http://host:9876",
            "--server-token-env",
            "MY_TOKEN_VAR",
        ])
        .unwrap();
        assert_eq!(cli.server, Some("http://host:9876".to_string()));
        assert_eq!(cli.server_token_env, Some("MY_TOKEN_VAR".to_string()));
    }

    #[test]
    fn test_top_level_no_server_defaults_to_none() {
        let cli = Cli::try_parse_from(["cflx"]).unwrap();
        assert!(cli.server.is_none());
        assert!(cli.server_token.is_none());
        assert!(cli.server_token_env.is_none());
    }

    // Additional tests for web flag parsing behavior
    #[test]
    fn test_case_1_cflx() {
        // Case 1: cflx -> No subcommand (will trigger parse_tui_args in main.rs)
        let cli = Cli::try_parse_from(["cflx"]).unwrap();
        assert!(cli.command.is_none());
        println!("Case 1: 'cflx' -> No subcommand (TUI with web=false via parse_tui_args)");
    }

    #[test]
    fn test_case_2_cflx_web() {
        // Case 2: cflx --web -> No subcommand (--web is a top-level flag, should succeed)
        let cli = Cli::try_parse_from(["cflx", "--web"]).unwrap();
        assert!(cli.web);
        assert!(cli.command.is_none());
        println!("Case 2: 'cflx --web' -> No subcommand with web=true (TUI with web)");
    }

    #[test]
    fn test_case_3_cflx_tui_web() {
        // Case 3: cflx tui --web -> TUI subcommand with web=true
        let cli = Cli::try_parse_from(["cflx", "tui", "--web"]).unwrap();
        match &cli.command {
            Some(Commands::Tui(args)) => {
                assert!(args.web);
                println!("Case 3: 'cflx tui --web' -> TuiArgs with web=true");
            }
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn test_case_4_cflx_run_web() {
        // Case 4: cflx run --web -> Run subcommand with web=true
        let cli = Cli::try_parse_from(["cflx", "run", "--web"]).unwrap();
        match &cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.web);
                println!("Case 4: 'cflx run --web' -> RunArgs with web=true");
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_parse_tui_args_with_web_simulation() {
        // Simulate parse_tui_args logic for "cflx --web" from main.rs
        // This is what happens when Cli::parse() returns None
        // Note: TuiArgs is a subcommand struct, so it expects arguments starting with program name
        // The parse_tui_args function prepends "cflx", "tui" to simulate this behavior

        let args: Vec<String> = vec!["--web".to_string()];
        let full_args = {
            let mut v = vec!["cflx".to_string(), "tui".to_string()];
            v.extend(args);
            v
        };

        // Parse via full CLI path (simulating the behavior)
        let cli_result = Cli::try_parse_from(full_args.clone());
        match cli_result {
            Ok(cli) => match &cli.command {
                Some(Commands::Tui(tui_args)) => {
                    assert!(tui_args.web);
                    println!("Case 5 (parse_tui_args simulation): 'cflx --web' -> via Cli -> TuiArgs with web=true");
                }
                _ => panic!("Expected Tui subcommand"),
            },
            Err(e) => {
                panic!("Expected successful parse: {}", e);
            }
        }
    }

    // ── project subcommand tests ──────────────────────────────────────────────

    #[test]
    fn test_project_add_subcommand() {
        let cli = Cli::parse_from([
            "cflx",
            "project",
            "add",
            "https://github.com/org/repo.git",
            "main",
        ]);
        match cli.command {
            Some(Commands::Project(args)) => {
                assert!(!args.json);
                assert!(args.server.is_none());
                match args.command {
                    ProjectCommands::Add(a) => {
                        assert_eq!(a.remote_url, "https://github.com/org/repo.git");
                        assert_eq!(a.branch, Some("main".to_string()));
                    }
                    _ => panic!("Expected Add"),
                }
            }
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_remove_subcommand() {
        let cli = Cli::parse_from(["cflx", "project", "remove", "proj-abc123"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Remove(a) => {
                    assert_eq!(a.project_id, "proj-abc123");
                }
                _ => panic!("Expected Remove"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_status_no_id() {
        let cli = Cli::parse_from(["cflx", "project", "status"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Status(a) => {
                    assert!(a.project_id.is_none());
                }
                _ => panic!("Expected Status"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_status_with_id() {
        let cli = Cli::parse_from(["cflx", "project", "status", "proj-abc123"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Status(a) => {
                    assert_eq!(a.project_id, Some("proj-abc123".to_string()));
                }
                _ => panic!("Expected Status"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_sync_subcommand() {
        let cli = Cli::parse_from(["cflx", "project", "sync", "proj-abc123"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Sync(a) => {
                    assert_eq!(a.project_id, Some("proj-abc123".to_string()));
                }
                _ => panic!("Expected Sync"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_json_flag() {
        let cli = Cli::parse_from(["cflx", "project", "--json", "status"]);
        match cli.command {
            Some(Commands::Project(args)) => {
                assert!(args.json);
            }
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_json_short_flag() {
        let cli = Cli::parse_from(["cflx", "project", "-j", "status"]);
        match cli.command {
            Some(Commands::Project(args)) => {
                assert!(args.json);
            }
            _ => panic!("Expected Project subcommand"),
        }
    }

    #[test]
    fn test_project_server_flag() {
        let cli = Cli::parse_from(["cflx", "project", "--server", "http://host:9876", "status"]);
        match cli.command {
            Some(Commands::Project(args)) => {
                assert_eq!(args.server, Some("http://host:9876".to_string()));
            }
            _ => panic!("Expected Project subcommand"),
        }
    }

    // ── project sync --all tests ──────────────────────────────────────────────

    /// Task 3.1: `cflx project sync --all` must parse correctly.
    #[test]
    fn test_project_sync_all_flag() {
        let cli = Cli::parse_from(["cflx", "project", "sync", "--all"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Sync(sync_args) => {
                    assert!(sync_args.all);
                    assert!(sync_args.project_id.is_none());
                }
                _ => panic!("Expected Sync"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    /// `cflx project sync <id>` must parse correctly (project_id only, no --all).
    #[test]
    fn test_project_sync_project_id() {
        let cli = Cli::parse_from(["cflx", "project", "sync", "my-project-id"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Sync(sync_args) => {
                    assert!(!sync_args.all);
                    assert_eq!(sync_args.project_id, Some("my-project-id".to_string()));
                }
                _ => panic!("Expected Sync"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    /// `--all` and `project_id` together must be rejected by clap (conflicts_with).
    #[test]
    fn test_project_sync_all_and_project_id_conflict() {
        let result = Cli::try_parse_from(["cflx", "project", "sync", "--all", "proj-id"]);
        assert!(
            result.is_err(),
            "Expected parse error when --all and project_id are both set"
        );
    }

    /// Default server URL for `project sync --all`.
    #[test]
    fn test_project_sync_default_server() {
        let cli = Cli::parse_from(["cflx", "project", "sync", "--all"]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Sync(sync_args) => {
                    assert_eq!(sync_args.server, "http://127.0.0.1:9876");
                }
                _ => panic!("Expected Sync"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }

    /// Custom `--server` URL for `project sync --all`.
    #[test]
    fn test_project_sync_custom_server() {
        let cli = Cli::parse_from([
            "cflx",
            "project",
            "sync",
            "--all",
            "--server",
            "http://myhost:1234",
        ]);
        match cli.command {
            Some(Commands::Project(args)) => match args.command {
                ProjectCommands::Sync(sync_args) => {
                    assert_eq!(sync_args.server, "http://myhost:1234");
                }
                _ => panic!("Expected Sync"),
            },
            _ => panic!("Expected Project subcommand"),
        }
    }
    #[test]
    fn test_tui_help_displays_key_bindings() {
        // Regression test: Ensure TUI help output contains key bindings
        use clap::CommandFactory;

        let app = Cli::command();
        let tui_subcommand = app
            .find_subcommand("tui")
            .expect("tui subcommand should exist");

        // Get the long help text
        let mut help_output = Vec::new();
        tui_subcommand
            .clone()
            .write_long_help(&mut help_output)
            .unwrap();
        let help_text = String::from_utf8(help_output).unwrap();

        // Verify key bindings are documented
        assert!(help_text.contains("Space"), "Help should mention Space key");
        assert!(help_text.contains("F5"), "Help should mention F5 key");
        assert!(help_text.contains("Esc"), "Help should mention Esc key");
        assert!(help_text.contains("Tab"), "Help should mention Tab key");
        assert!(help_text.contains("q"), "Help should mention q key");

        // Verify the key binding section is present
        assert!(
            help_text.contains("Key bindings"),
            "Help should have 'Key bindings' section"
        );
    }
}
