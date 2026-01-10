use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// OpenSpec Orchestrator - Automate OpenSpec workflow
#[derive(Parser, Debug)]
#[command(name = "openspec-orchestrator")]
#[command(version)]
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

    /// Initialize a new configuration file
    Init(InitArgs),

    /// Manage change approval status
    Approve(ApproveArgs),
}

/// Arguments for the run subcommand
#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Process only the specified changes (comma-separated, e.g., --change a,b,c)
    #[arg(long, value_delimiter = ',')]
    pub change: Option<Vec<String>>,

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

    /// Maximum number of iterations for the orchestration loop (overrides config, 0 = no limit)
    #[arg(long)]
    pub max_iterations: Option<u32>,

    /// Enable parallel execution mode using jj workspaces (requires jj repository)
    #[arg(long)]
    pub parallel: bool,

    /// Maximum number of concurrent workspaces for parallel execution
    #[arg(long)]
    pub max_concurrent: Option<usize>,

    /// Preview parallelization groups without executing (dry run)
    #[arg(long)]
    pub dry_run: bool,
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

/// Arguments for the approve subcommand
#[derive(Parser, Debug)]
pub struct ApproveArgs {
    #[command(subcommand)]
    pub action: ApproveAction,
}

/// Approve subcommand actions
#[derive(Subcommand, Debug)]
pub enum ApproveAction {
    /// Approve a change (create approved file with checksums)
    Set {
        /// The change ID to approve
        change_id: String,
    },

    /// Unapprove a change (remove approved file)
    Unset {
        /// The change ID to unapprove
        change_id: String,
    },

    /// Check approval status of a change
    Status {
        /// The change ID to check
        change_id: String,
    },
}

/// Check if the current directory is a jj repository
pub fn check_jj_directory() -> bool {
    std::path::Path::new(".jj").exists()
}

/// Check if jj CLI is available
pub fn check_jj_available() -> bool {
    std::process::Command::new("jj")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::env;

    // Note: These tests need to run sequentially due to environment variable manipulation
    // Run with: cargo test --test-threads=1

    #[test]
    fn test_default_openspec_cmd() {
        // Save original env value
        let original = env::var("OPENSPEC_CMD").ok();

        // Remove environment variable to test default
        env::remove_var("OPENSPEC_CMD");

        // When neither CLI arg nor env var is set, default value is used
        let cli = Cli::parse_from(["openspec-orchestrator"]);

        // Restore original env value
        if let Some(val) = original {
            env::set_var("OPENSPEC_CMD", val);
        }

        assert_eq!(cli.openspec_cmd, "npx @fission-ai/openspec@latest");
    }

    #[test]
    fn test_cli_arg_openspec_cmd() {
        // CLI argument should override default
        let cli = Cli::parse_from(["openspec-orchestrator", "--openspec-cmd", "./my-openspec"]);
        assert_eq!(cli.openspec_cmd, "./my-openspec");
    }

    #[test]
    fn test_run_subcommand_openspec_cmd() {
        // Run subcommand should also accept --openspec-cmd
        let cli = Cli::parse_from([
            "openspec-orchestrator",
            "run",
            "--openspec-cmd",
            "/usr/local/bin/openspec",
        ]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.openspec_cmd, "/usr/local/bin/openspec");
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_tui_subcommand_openspec_cmd() {
        // TUI subcommand should also accept --openspec-cmd
        let cli = Cli::parse_from([
            "openspec-orchestrator",
            "tui",
            "--openspec-cmd",
            "/custom/openspec",
        ]);

        match cli.command {
            Some(Commands::Tui(args)) => {
                assert_eq!(args.openspec_cmd, "/custom/openspec");
            }
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn test_env_var_openspec_cmd() {
        // Save original env value
        let original = env::var("OPENSPEC_CMD").ok();

        // Set environment variable
        env::set_var("OPENSPEC_CMD", "/usr/local/bin/openspec");

        // Parse CLI without --openspec-cmd argument
        // Note: clap caches env vars at parse time
        let cli = Cli::try_parse_from(["openspec-orchestrator"]);

        // Restore original env value
        if let Some(val) = original {
            env::set_var("OPENSPEC_CMD", val);
        } else {
            env::remove_var("OPENSPEC_CMD");
        }

        // Verify environment variable is used
        let cli = cli.unwrap();
        assert_eq!(cli.openspec_cmd, "/usr/local/bin/openspec");
    }

    #[test]
    fn test_cli_arg_overrides_env_var() {
        // Save original env value
        let original = env::var("OPENSPEC_CMD").ok();

        // Set environment variable
        env::set_var("OPENSPEC_CMD", "/env/openspec");

        // Parse CLI with --openspec-cmd argument
        let cli =
            Cli::try_parse_from(["openspec-orchestrator", "--openspec-cmd", "./cli-openspec"]);

        // Restore original env value
        if let Some(val) = original {
            env::set_var("OPENSPEC_CMD", val);
        } else {
            env::remove_var("OPENSPEC_CMD");
        }

        // Verify CLI argument takes precedence over env var
        let cli = cli.unwrap();
        assert_eq!(cli.openspec_cmd, "./cli-openspec");
    }

    #[test]
    fn test_run_subcommand_config_option() {
        let cli = Cli::parse_from([
            "openspec-orchestrator",
            "run",
            "--config",
            "/path/to/config.jsonc",
        ]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.config, Some(PathBuf::from("/path/to/config.jsonc")));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_change_option() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--change", "add-feature-x"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.change, Some(vec!["add-feature-x".to_string()]));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_multiple_changes_comma_separated() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--change", "a,b,c"]);

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
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--change", "a, b, c"]);

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
        let cli = Cli::parse_from(["openspec-orchestrator", "run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.change.is_none());
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_no_subcommand() {
        let cli = Cli::parse_from(["openspec-orchestrator"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_global_openspec_cmd_available_to_subcommands() {
        // Global flag should be available even with subcommand
        let cli = Cli::parse_from([
            "openspec-orchestrator",
            "--openspec-cmd",
            "/global/openspec",
            "run",
        ]);

        // The global --openspec-cmd is parsed at the Cli level
        assert_eq!(cli.openspec_cmd, "/global/openspec");
    }

    #[test]
    fn test_init_subcommand_default_template() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init"]);

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
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "--template", "opencode"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Opencode));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_claude_template() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "--template", "claude"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Claude));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_codex_template() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "--template", "codex"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Codex));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_short_template_flag() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "-t", "opencode"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(matches!(args.template, Template::Opencode));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_force_flag() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "--force"]);

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(args.force);
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_init_subcommand_short_force_flag() {
        let cli = Cli::parse_from(["openspec-orchestrator", "init", "-f"]);

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
        let result = Cli::try_parse_from(["openspec-orchestrator", "--version"]);
        assert!(result.is_err());

        let err = result.unwrap_err();
        // clap returns DisplayVersion error kind for --version
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_short_version_flag() {
        // -V flag should also display version
        let result = Cli::try_parse_from(["openspec-orchestrator", "-V"]);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_run_subcommand_max_iterations_default() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.max_iterations.is_none());
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_custom() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--max-iterations", "100"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(100));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_zero() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--max-iterations", "0"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(0));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_parallel_flag_default() {
        let cli = Cli::parse_from(["openspec-orchestrator", "run"]);

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
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--parallel"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_concurrent() {
        let cli = Cli::parse_from([
            "openspec-orchestrator",
            "run",
            "--parallel",
            "--max-concurrent",
            "5",
        ]);

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
        let cli = Cli::parse_from(["openspec-orchestrator", "run", "--parallel", "--dry-run"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
                assert!(args.dry_run);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }
}
