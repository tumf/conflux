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

    /// Initialize a new configuration file
    Init(InitArgs),
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
                assert_eq!(args.change, Some("add-feature-x".to_string()));
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
}
