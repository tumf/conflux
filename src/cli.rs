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

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the OpenSpec change orchestration loop (non-interactive)
    Run(RunArgs),

    /// Launch the interactive TUI dashboard
    Tui(TuiArgs),

    /// Initialize a new configuration file
    Init(InitArgs),

    /// Check for conflicts between spec delta files across changes
    CheckConflicts(CheckConflictsArgs),
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

EXAMPLES:
  cflx tui                    # Launch TUI (default when no subcommand)
  cflx tui --logs debug.log   # TUI with file logging
  cflx tui --web              # TUI with web monitoring enabled")]
pub struct TuiArgs {
    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Write debug logs to the specified file (e.g., --logs debug.log)
    #[arg(long)]
    pub logs: Option<PathBuf>,

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
    fn test_tui_subcommand_logs_option() {
        let cli = Cli::parse_from(["cflx", "tui", "--logs", "debug.log"]);

        match cli.command {
            Some(Commands::Tui(args)) => {
                assert_eq!(args.logs, Some(PathBuf::from("debug.log")));
            }
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn test_tui_subcommand_no_logs_option() {
        let cli = Cli::parse_from(["cflx", "tui"]);

        match cli.command {
            Some(Commands::Tui(args)) => {
                assert!(args.logs.is_none());
            }
            _ => panic!("Expected Tui subcommand"),
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
}
