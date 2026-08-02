use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use tracing::debug;

/// Build metadata included in versioned user-facing logs and output.
pub const VERSION_WITH_BUILD: &str = concat!(
    "v",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("BUILD_NUMBER"),
    ")"
);

/// Get version string with build number
const fn get_version_string() -> &'static str {
    VERSION_WITH_BUILD
}

fn parse_push_remote(value: &str) -> std::result::Result<String, String> {
    if value.contains(':') {
        return Err("branch selection is not supported for --push; use a remote name only".into());
    }
    Ok(value.to_string())
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
  --web-auth-token TOKEN     Bearer token for the /api/v2 remote-control API
                             (visible in process listings; prefer the -env form)
  --web-auth-token-env VAR   Environment variable holding that bearer token
  --web-allowed-origin ORIGIN  Exact extra CORS origin for /api/v2 (repeatable)
    logs                 View persistent Conflux log files without mutating them

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

    /// Literal bearer token for the `/api/v2` remote-control API.
    ///
    /// Required for any non-loopback `--web-bind`. Prefer
    /// `--web-auth-token-env`: a literal value here is visible to anything that
    /// can inspect this process's arguments.
    #[arg(long, conflicts_with = "web_auth_token_env")]
    pub web_auth_token: Option<String>,

    /// Environment variable holding the bearer token for the `/api/v2` API.
    #[arg(long, conflicts_with = "web_auth_token")]
    pub web_auth_token_env: Option<String>,

    /// Exact additional origin allowed to make cross-origin `/api/v2` requests.
    ///
    /// Repeatable. Exact `scheme://host[:port]` values only — wildcards are
    /// rejected, and forwarded headers never widen this list, so a reverse proxy
    /// that changes the external origin must name it here.
    #[arg(long = "web-allowed-origin", value_name = "ORIGIN")]
    pub web_allowed_origins: Vec<String>,

    /// Push completed parallel TUI change branches to a remote instead of merging to base.
    #[arg(long, num_args = 0..=1, default_missing_value = "origin", value_parser = parse_push_remote)]
    pub push: Option<String>,

    /// Publish every completed change's verified cumulative base to the selected
    /// remote's same-name branch before that change succeeds.
    ///
    /// Same contract as `cflx run -u`: `-u` and value-less `--integrate-upstream`
    /// select `origin`, a named remote requires `--integrate-upstream=<remote>`,
    /// and `--upstream-verify-command` is mandatory. Local TUI only.
    #[arg(
        long,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = crate::upstream::DEFAULT_UPSTREAM_REMOTE,
        value_parser = crate::upstream::options::parse_upstream_remote
    )]
    pub integrate_upstream: Option<String>,

    /// Publish to `origin`; short value-less spelling of `--integrate-upstream`.
    ///
    /// A named remote is available only as `--integrate-upstream=<remote>`, so
    /// `-u` takes no value at all and `-u=<remote>` is a parse error.
    #[arg(short = 'u', action = clap::ArgAction::SetTrue, conflicts_with = "integrate_upstream")]
    pub integrate_upstream_default_remote: bool,

    /// Complete repository verification command run before every publication.
    ///
    /// Requires `-u`/`--integrate-upstream`; rejected without it.
    #[arg(long)]
    pub upstream_verify_command: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Rejection of a top-level upstream option that an explicit subcommand ignores.
///
/// The top-level `-u` / `--integrate-upstream` / `--upstream-verify-command`
/// options exist only to give bare `cflx` the same contract as `cflx tui`. When
/// an explicit subcommand follows, the subcommand parses its own options and the
/// top-level values are never read — so accepting them would silently drop an
/// opt-in whose publication is part of the success contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelUpstreamOptionError {
    /// Spelling the operator used, as it must be repositioned.
    pub option: &'static str,
    /// Subcommand that would have ignored it.
    pub subcommand: &'static str,
}

impl std::fmt::Display for TopLevelUpstreamOptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} must follow the '{}' subcommand: 'cflx {} {}' applies to the subcommand, while 'cflx {} {}' would be ignored",
            self.option, self.subcommand, self.subcommand, self.option, self.option, self.subcommand
        )
    }
}

impl std::error::Error for TopLevelUpstreamOptionError {}

impl Cli {
    /// Name of the subcommand for diagnostics, when one was given.
    fn subcommand_name(&self) -> Option<&'static str> {
        match self.command.as_ref()? {
            Commands::Run(_) => Some("run"),
            Commands::Tui(_) => Some("tui"),
            Commands::Init(_) => Some("init"),
            Commands::CheckConflicts(_) => Some("check-conflicts"),
            Commands::InstallSkills(_) => Some("install-skills"),
            Commands::Logs(_) => Some("logs"),
            Commands::Openspec(_) => Some("openspec"),
            Commands::Completion(_) => Some("completion"),
            Commands::Complete(_) => Some("__complete"),
        }
    }

    /// Reject top-level upstream options that an explicit subcommand would drop.
    ///
    /// Called before any orchestration, logging, or workspace mutation.
    pub fn validate_upstream_option_placement(
        &self,
    ) -> std::result::Result<(), TopLevelUpstreamOptionError> {
        let Some(subcommand) = self.subcommand_name() else {
            return Ok(());
        };
        let option = if self.integrate_upstream_default_remote {
            "-u"
        } else if self.integrate_upstream.is_some() {
            "--integrate-upstream"
        } else if self.upstream_verify_command.is_some() {
            "--upstream-verify-command"
        } else {
            return Ok(());
        };
        Err(TopLevelUpstreamOptionError { option, subcommand })
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the OpenSpec change orchestration loop (non-interactive)
    Run(RunArgs),

    /// Launch the interactive TUI dashboard
    ///
    /// Key bindings: Space (select), F5 (start by default), Esc (stop), Tab (switch view), q (quit).
    /// Override the default start key in ~/.config/cflx/tui.jsonc.
    Tui(TuiArgs),

    /// Initialize a new configuration file
    Init(InitArgs),

    /// Check for conflicts between spec delta files across changes
    CheckConflicts(CheckConflictsArgs),

    /// Install agent skills into .agents/skills or .claude/skills
    ///
    /// Installs bundled agent skills into the standard target location.
    ///
    /// EXAMPLES:
    ///   cflx install-skills                    # Install bundled skills to .agents (project scope)
    ///   cflx install-skills --global           # Install bundled skills to .agents (global scope)
    ///   cflx install-skills --claude           # Install bundled skills to .claude (project scope)
    ///   cflx install-skills --claude --global  # Install bundled skills to .claude (global scope)
    #[command(name = "install-skills")]
    InstallSkills(InstallSkillsArgs),

    /// View persistent Conflux logs without creating, appending, or cleaning log files
    ///
    /// EXAMPLES:
    ///   cflx logs --path                    # Print selected log file path
    ///   cflx logs --last 50                 # Print the last 50 log lines
    ///   cflx logs --follow                  # Print recent lines, then stream appended lines
    ///   cflx logs --today --project my-slug # Select today's log for an explicit project
    Logs(LogsArgs),

    /// OpenSpec utility commands for repository-scoped operations
    ///
    /// Provides native subcommands for listing, inspecting, validating, and
    /// archiving OpenSpec changes and specs — replacing the former Python helper.
    ///
    /// EXAMPLES:
    ///   cflx openspec list                          # List active changes
    ///   cflx openspec list --specs                  # List canonical specs
    ///   cflx openspec show my-change                # Show change details
    ///   cflx openspec show my-change --json         # JSON output
    ///   cflx openspec validate --strict             # Validate all changes
    ///   cflx openspec archive my-change --yes       # Archive a change
    Openspec(OpenspecArgs),

    /// Generate shell completion scripts
    Completion(CompletionArgs),

    /// Hidden internal completion candidate commands
    #[command(name = "__complete", hide = true)]
    Complete(InternalCompleteArgs),
}

/// Supported shells for generated completion scripts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Zsh,
}

impl From<CompletionShell> for Shell {
    fn from(shell: CompletionShell) -> Self {
        match shell {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::PowerShell => Shell::PowerShell,
            CompletionShell::Zsh => Shell::Zsh,
        }
    }
}

/// Arguments for the completion subcommand
#[derive(Parser, Debug)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

/// Hidden internal completion candidate commands.
#[derive(Parser, Debug)]
pub struct InternalCompleteArgs {
    #[command(subcommand)]
    pub command: InternalCompleteCommands,
}

#[derive(Subcommand, Debug)]
pub enum InternalCompleteCommands {
    /// Print OpenSpec change ID completion candidates, one per line
    #[command(name = "change-ids")]
    ChangeIds(ChangeIdCompletionArgs),
}

#[derive(Parser, Debug)]
pub struct ChangeIdCompletionArgs {
    /// Include active OpenSpec changes
    #[arg(long)]
    pub active: bool,

    /// Include archived OpenSpec changes
    #[arg(long)]
    pub archived: bool,

    /// Only include candidates beginning with this prefix
    #[arg(long)]
    pub prefix: Option<String>,
}

/// Arguments for the logs subcommand
#[derive(Parser, Debug)]
#[command(
    long_about = "View persistent Conflux log files without initializing runtime logging.

By default, prints the last 200 lines from the latest existing log for the current project.
Use --path to inspect the selected path without requiring the file to exist.

EXAMPLES:
  cflx logs --path
  cflx logs --last 50
  cflx logs --follow
  cflx logs --today --project conflux-a1b2c3d4"
)]
pub struct LogsArgs {
    /// Print the selected log path instead of reading log content
    #[arg(long)]
    pub path: bool,

    /// Print at most the last N lines from the selected log file
    #[arg(long, value_name = "N")]
    pub last: Option<usize>,

    /// Print the selected tail and then stream appended lines until interrupted
    #[arg(long)]
    pub follow: bool,

    /// Prefer today's log file instead of the latest existing dated log file
    #[arg(long)]
    pub today: bool,

    /// Select an explicit log project slug under the Conflux log root
    #[arg(long, value_name = "SLUG")]
    pub project: Option<String>,
}

/// Arguments for the run subcommand
#[derive(Parser, Debug)]
#[command(
    group(
        ArgGroup::new("run_target")
            .required(true)
            .multiple(false)
            .args(["all", "change", "changes"])
    ),
    long_about = "Execute the OpenSpec change orchestration loop in non-interactive mode.

This mode requires an explicit target: --all for every current change, positional
change IDs for selected changes, or legacy --change for comma-separated IDs.

PARALLEL EXECUTION:
  --parallel enables concurrent processing using git worktrees. Changes are
  analyzed for dependencies and executed in optimal parallel groups.

WEB MONITORING:
  --web enables remote monitoring via HTTP. Access progress from any browser
  while orchestration runs in background.

EXAMPLES:
  cflx run --all                           # Process all current changes
  cflx run my-feature other-change         # Process selected changes
  cflx run --change my-feature,other-change  # Legacy selected changes
  cflx run --all --parallel --max-concurrent 5  # Parallel with 5 workers
  cflx run my-feature --parallel --dry-run      # Preview selected parallel plan
  cflx run --all --web --web-port 8080     # Enable web monitoring on port 8080"
)]
pub struct RunArgs {
    /// Process all current eligible changes explicitly
    #[arg(long)]
    pub all: bool,

    /// Process only the specified changes (comma-separated, e.g., --change a,b,c)
    #[arg(long, value_delimiter = ',')]
    pub change: Option<Vec<String>>,

    /// Positional change IDs to process
    pub changes: Vec<String>,

    /// Path to custom configuration file (JSONC format)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Maximum number of iterations for the orchestration loop (overrides config, 0 = no limit)
    #[arg(long)]
    pub max_iterations: Option<u32>,

    /// Enable parallel execution mode using git worktrees
    #[arg(long)]
    pub parallel: bool,

    /// Push completed parallel change branches to a remote instead of merging to base.
    #[arg(long, num_args = 0..=1, default_missing_value = "origin", value_parser = parse_push_remote)]
    pub push: Option<String>,

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

    /// Literal bearer token for the `/api/v2` remote-control API.
    ///
    /// Required for any non-loopback `--web-bind`. Prefer
    /// `--web-auth-token-env`: a literal value here is visible to anything that
    /// can inspect this process's arguments.
    #[arg(long, conflicts_with = "web_auth_token_env")]
    pub web_auth_token: Option<String>,

    /// Environment variable holding the bearer token for the `/api/v2` API.
    #[arg(long, conflicts_with = "web_auth_token")]
    pub web_auth_token_env: Option<String>,

    /// Exact additional origin allowed to make cross-origin `/api/v2` requests.
    ///
    /// Repeatable. Exact `scheme://host[:port]` values only — wildcards are
    /// rejected, and forwarded headers never widen this list, so a reverse proxy
    /// that changes the external origin must name it here.
    #[arg(long = "web-allowed-origin", value_name = "ORIGIN")]
    pub web_allowed_origins: Vec<String>,

    /// Integrate the selected remote's same-name base branch into the cumulative
    /// base during the run, and push the verified result once at completion.
    ///
    /// `-u` and value-less `--integrate-upstream` select `origin`. A named remote
    /// requires `=`, as in `--integrate-upstream=upstream`; `-u <remote>` is not
    /// supported and never consumes a following positional change ID.
    ///
    /// Cumulative parallel run mode only. Requires `--upstream-verify-command`.
    #[arg(
        long,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = crate::upstream::DEFAULT_UPSTREAM_REMOTE,
        value_parser = crate::upstream::options::parse_upstream_remote
    )]
    pub integrate_upstream: Option<String>,

    /// Publish to `origin`; short value-less spelling of `--integrate-upstream`.
    ///
    /// A named remote is available only as `--integrate-upstream=<remote>`, so
    /// `-u` takes no value at all and `-u=<remote>` is a parse error.
    #[arg(short = 'u', action = clap::ArgAction::SetTrue, conflicts_with = "integrate_upstream")]
    pub integrate_upstream_default_remote: bool,

    /// Complete repository verification command run after every cumulative base
    /// tree change and immediately before the final push.
    ///
    /// Requires `-u`/`--integrate-upstream`; rejected without it.
    #[arg(long)]
    pub upstream_verify_command: Option<String>,
}

impl RunArgs {
    /// Resolve the invocation-scoped upstream integration configuration.
    ///
    /// Returns `Ok(None)` for the default-off path, which installs no upstream
    /// behavior at all.
    pub fn upstream_integration(
        &self,
    ) -> std::result::Result<
        Option<crate::upstream::UpstreamIntegrationConfig>,
        crate::upstream::UpstreamOptionError,
    > {
        let selected = crate::upstream::options::selected_upstream_remote(
            self.integrate_upstream_default_remote,
            self.integrate_upstream.as_deref(),
        );
        crate::upstream::options::resolve_frontend_upstream_config(
            selected.as_deref(),
            self.upstream_verify_command.as_deref(),
            self.push.as_deref(),
        )
    }

    /// Returns None for --all and Some(ids) for explicit selected targets.
    pub fn normalized_target_changes(&self) -> Option<Vec<String>> {
        if self.all {
            None
        } else if !self.changes.is_empty() {
            Some(self.changes.clone())
        } else {
            self.change.clone()
        }
    }
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
  F5        Start/resume processing (default; override in ~/.config/cflx/tui.jsonc)
  Esc       Stop processing (press twice to force)
  Tab       Switch between Changes/Worktrees view
  q         Quit

TUI USER CONFIG:
  Set keybindings.start in ~/.config/cflx/tui.jsonc, for example:
    { \"keybindings\": { \"start\": [\"F5\", \"!\"] } }
  The help text documents defaults only and does not render dynamic user config values.

WEB MONITORING:
  --web enables simultaneous web-based monitoring alongside the TUI.

EXAMPLES:
  cflx tui                                        # Launch TUI (default when no subcommand)
  cflx tui --web                                  # TUI with web monitoring enabled")]
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

    /// Literal bearer token for the `/api/v2` remote-control API.
    ///
    /// Required for any non-loopback `--web-bind`. Prefer
    /// `--web-auth-token-env`: a literal value here is visible to anything that
    /// can inspect this process's arguments.
    #[arg(long, conflicts_with = "web_auth_token_env")]
    pub web_auth_token: Option<String>,

    /// Environment variable holding the bearer token for the `/api/v2` API.
    #[arg(long, conflicts_with = "web_auth_token")]
    pub web_auth_token_env: Option<String>,

    /// Exact additional origin allowed to make cross-origin `/api/v2` requests.
    ///
    /// Repeatable. Exact `scheme://host[:port]` values only — wildcards are
    /// rejected, and forwarded headers never widen this list, so a reverse proxy
    /// that changes the external origin must name it here.
    #[arg(long = "web-allowed-origin", value_name = "ORIGIN")]
    pub web_allowed_origins: Vec<String>,

    /// Push completed parallel TUI change branches to a remote instead of merging to base.
    #[arg(long, num_args = 0..=1, default_missing_value = "origin", value_parser = parse_push_remote)]
    pub push: Option<String>,

    /// Publish every completed change's verified cumulative base to the selected
    /// remote's same-name branch before that change succeeds.
    ///
    /// Same contract as `cflx run -u`: `-u` and value-less `--integrate-upstream`
    /// select `origin`, a named remote requires `--integrate-upstream=<remote>`,
    /// and `--upstream-verify-command` is mandatory.
    #[arg(
        long,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = crate::upstream::DEFAULT_UPSTREAM_REMOTE,
        value_parser = crate::upstream::options::parse_upstream_remote
    )]
    pub integrate_upstream: Option<String>,

    /// Publish to `origin`; short value-less spelling of `--integrate-upstream`.
    ///
    /// A named remote is available only as `--integrate-upstream=<remote>`, so
    /// `-u` takes no value at all and `-u=<remote>` is a parse error.
    #[arg(short = 'u', action = clap::ArgAction::SetTrue, conflicts_with = "integrate_upstream")]
    pub integrate_upstream_default_remote: bool,

    /// Complete repository verification command run before every publication.
    ///
    /// Requires `-u`/`--integrate-upstream`; rejected without it.
    #[arg(long)]
    pub upstream_verify_command: Option<String>,
}

impl TuiArgs {
    /// Resolve the invocation-scoped upstream integration configuration.
    ///
    /// Identical normalization to [`RunArgs::upstream_integration`], plus the
    /// TUI-only `--push` rejection. Returns `Ok(None)` for the default-off path,
    /// which installs no upstream behavior.
    pub fn upstream_integration(
        &self,
    ) -> std::result::Result<
        Option<crate::upstream::UpstreamIntegrationConfig>,
        crate::upstream::UpstreamOptionError,
    > {
        let selected = crate::upstream::options::selected_upstream_remote(
            self.integrate_upstream_default_remote,
            self.integrate_upstream.as_deref(),
        );
        crate::upstream::options::resolve_frontend_upstream_config(
            selected.as_deref(),
            self.upstream_verify_command.as_deref(),
            self.push.as_deref(),
        )
    }
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

/// Install target family for `install-skills`.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallSkillsTarget {
    Agents,
    Claude,
}

/// Arguments for the `install-skills` subcommand
#[derive(Parser, Debug)]
#[command(
    long_about = "Install bundled agent skills into the standard skills location.

Skills are embedded into the cflx binary at compile time and installed directly
without requiring a skills/ directory to be present. When no embedded skills are
available (uncommon), the command falls back to discovering skills from a local
skills/ directory at the project root.

TARGETS:
  Default target: .agents (existing behavior)
  --claude:       .claude

SCOPE (.agents target):
  Project scope (default): installs to ./.agents/skills
                            lock file:  ./.agents/.skill-lock.json
  Global scope (--global):  installs to ~/.agents/skills
                            lock file:  ~/.agents/.skill-lock.json

SCOPE (.claude target):
  Project scope (default): installs to ./.claude/skills
                            lock file:  ./.claude/.skill-lock.json
  Global scope (--global):  installs to ~/.claude/skills
                            lock file:  ~/.claude/.skill-lock.json

EXAMPLES:
  cflx install-skills
  cflx install-skills --global
  cflx install-skills --claude
  cflx install-skills --claude --global"
)]
pub struct InstallSkillsArgs {
    /// Install into global scope (~/.agents/skills or ~/.claude/skills) instead of project scope
    #[arg(long)]
    pub global: bool,

    /// Install bundled skills into .claude/skills instead of .agents/skills
    #[arg(long, default_value = "false")]
    pub claude: bool,

    /// Hidden positional argument to detect and reject legacy source forms (e.g. "self", "local:...").
    #[arg(hide = true)]
    pub legacy_source: Option<String>,
}

impl InstallSkillsArgs {
    pub fn target(&self) -> InstallSkillsTarget {
        if self.claude {
            InstallSkillsTarget::Claude
        } else {
            InstallSkillsTarget::Agents
        }
    }
}

/// Return a migration guidance error message when a legacy source argument is detected.
pub fn install_skills_legacy_error(src: &str) -> String {
    format!(
        "error: unrecognized argument '{src}'\n\n\
         The source argument is no longer accepted.\n\
         Use:\n  \
         cflx install-skills           # project scope\n  \
         cflx install-skills --global  # global scope"
    )
}

/// Arguments for the `openspec` subcommand group
#[derive(Parser, Debug)]
#[command(about = "OpenSpec utility commands")]
pub struct OpenspecArgs {
    #[command(subcommand)]
    pub command: OpenspecCommands,
}

/// Subcommands for `cflx openspec`
#[derive(Subcommand, Debug)]
pub enum OpenspecCommands {
    /// List active changes or canonical specs
    List(OpenspecListArgs),

    /// Show detailed information about a change
    Show(OpenspecShowArgs),

    /// Validate change structure and spec deltas
    ///
    /// Use `--archive-gate` to run the local archive-readiness equivalent
    /// (`--strict --evidence error`) so evidence findings fail before archive.
    Validate(OpenspecValidateArgs),

    /// Archive a deployed change and promote spec deltas
    Archive(OpenspecArchiveArgs),
}

/// Arguments for `cflx openspec list`
#[derive(Parser, Debug)]
pub struct OpenspecListArgs {
    /// List canonical specs instead of changes
    #[arg(long)]
    pub specs: bool,
}

/// Arguments for `cflx openspec show`
#[derive(Parser, Debug)]
pub struct OpenspecShowArgs {
    /// Change ID to show
    pub change_id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show only spec deltas
    #[arg(long)]
    pub deltas_only: bool,
}

/// Evidence checking mode for validation
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum EvidenceMode {
    /// Do not check for evidence hints
    #[default]
    Off,
    /// Warn on missing evidence hints
    Warn,
    /// Error on missing evidence hints
    Error,
}

/// Arguments for `cflx openspec validate`
#[derive(Parser, Debug)]
pub struct OpenspecValidateArgs {
    /// Change ID to validate (omit to validate all)
    pub change_id: Option<String>,

    /// Enable strict validation mode
    #[arg(long)]
    pub strict: bool,

    /// Run archive-readiness validation locally (`--strict --evidence error`)
    #[arg(long)]
    pub archive_gate: bool,

    /// How to treat missing implementation evidence in tasks.md
    #[arg(long, value_enum, default_value_t = EvidenceMode::Off)]
    pub evidence: EvidenceMode,
}

/// Arguments for `cflx openspec archive`
#[derive(Parser, Debug)]
pub struct OpenspecArchiveArgs {
    /// Change ID to archive
    pub change_id: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,

    /// Skip spec updates during archive
    #[arg(long)]
    pub skip_specs: bool,
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

    fn run_args(argv: &[&str]) -> RunArgs {
        match Cli::parse_from(argv).command {
            Some(Commands::Run(args)) => args,
            _ => panic!("expected run subcommand for {:?}", argv),
        }
    }

    #[test]
    fn upstream_integration_is_absent_by_default() {
        let args = run_args(&["cflx", "run", "--all", "--parallel"]);
        assert_eq!(args.integrate_upstream, None);
        assert_eq!(args.upstream_verify_command, None);
        assert_eq!(args.upstream_integration().unwrap(), None);
    }

    #[test]
    fn upstream_integration_short_and_long_aliases_are_equivalent() {
        let short = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "-u",
            "--upstream-verify-command",
            "cargo test",
        ]);
        let long = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--integrate-upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        // `-u` is its own value-less argument, so equivalence is asserted on the
        // remote both spellings select rather than on one shared raw field.
        assert!(short.integrate_upstream_default_remote);
        assert_eq!(short.integrate_upstream, None);
        assert_eq!(long.integrate_upstream.as_deref(), Some("origin"));
        assert_eq!(
            short.upstream_integration().unwrap(),
            long.upstream_integration().unwrap()
        );
        assert_eq!(
            short.upstream_integration().unwrap(),
            Some(crate::upstream::UpstreamIntegrationConfig::new(
                "origin",
                "cargo test"
            ))
        );
    }

    #[test]
    fn upstream_integration_short_option_does_not_consume_change_id() {
        let args = run_args(&[
            "cflx",
            "run",
            "--parallel",
            "-u",
            "--upstream-verify-command",
            "cargo test",
            "my-change",
        ]);
        assert!(args.integrate_upstream_default_remote);
        assert_eq!(args.changes, vec!["my-change".to_string()]);
        assert_eq!(
            args.normalized_target_changes(),
            Some(vec!["my-change".to_string()])
        );
    }

    #[test]
    fn upstream_integration_named_remote_requires_equals() {
        let args = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(args.integrate_upstream.as_deref(), Some("upstream"));
        assert_eq!(
            args.upstream_integration().unwrap(),
            Some(crate::upstream::UpstreamIntegrationConfig::new(
                "upstream",
                "cargo test"
            ))
        );

        // Space-separated values are not option values; they stay positional.
        let spaced = run_args(&[
            "cflx",
            "run",
            "--parallel",
            "--integrate-upstream",
            "upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(spaced.integrate_upstream.as_deref(), Some("origin"));
        assert_eq!(spaced.changes, vec!["upstream".to_string()]);
    }

    #[test]
    fn upstream_integration_rejects_invalid_remote_value() {
        let err = Cli::try_parse_from([
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--integrate-upstream=origin:main",
        ])
        .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn upstream_integration_requires_verify_command() {
        let args = run_args(&["cflx", "run", "--all", "--parallel", "-u"]);
        assert_eq!(
            args.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::MissingVerifyCommand)
        );
    }

    #[test]
    fn upstream_integration_verify_command_alone_is_rejected() {
        let args = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(
            args.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::VerifyCommandWithoutOption)
        );
    }

    fn tui_args(argv: &[&str]) -> TuiArgs {
        match Cli::parse_from(argv).command {
            Some(Commands::Tui(args)) => args,
            _ => panic!("expected tui subcommand for {:?}", argv),
        }
    }

    /// Bare `cflx` has no subcommand; it forwards its own options into `TuiArgs`,
    /// exactly as `main` does.
    fn bare_tui_args(argv: &[&str]) -> TuiArgs {
        let cli = Cli::parse_from(argv);
        assert!(cli.command.is_none(), "expected bare invocation");
        TuiArgs {
            config: cli.config,
            web: cli.web,
            web_port: cli.web_port,
            web_bind: cli.web_bind,
            web_auth_token: cli.web_auth_token,
            web_auth_token_env: cli.web_auth_token_env,
            web_allowed_origins: cli.web_allowed_origins,
            push: cli.push,
            integrate_upstream: cli.integrate_upstream,
            integrate_upstream_default_remote: cli.integrate_upstream_default_remote,
            upstream_verify_command: cli.upstream_verify_command,
        }
    }

    #[test]
    fn per_change_upstream_is_absent_by_default_for_tui() {
        let bare = bare_tui_args(&["cflx"]);
        assert_eq!(bare.integrate_upstream, None);
        assert_eq!(bare.upstream_verify_command, None);
        assert_eq!(bare.upstream_integration().unwrap(), None);

        let explicit = tui_args(&["cflx", "tui"]);
        assert_eq!(explicit.upstream_integration().unwrap(), None);
    }

    #[test]
    fn per_change_upstream_run_bare_tui_and_explicit_tui_are_equivalent() {
        let run = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "-u",
            "--upstream-verify-command",
            "cargo test",
        ]);
        let bare = bare_tui_args(&["cflx", "-u", "--upstream-verify-command", "cargo test"]);
        let explicit = tui_args(&[
            "cflx",
            "tui",
            "-u",
            "--upstream-verify-command",
            "cargo test",
        ]);

        let expected = Some(crate::upstream::UpstreamIntegrationConfig::new(
            "origin",
            "cargo test",
        ));
        assert_eq!(run.upstream_integration().unwrap(), expected);
        assert_eq!(bare.upstream_integration().unwrap(), expected);
        assert_eq!(explicit.upstream_integration().unwrap(), expected);
    }

    #[test]
    fn per_change_upstream_tui_accepts_explicit_remote_with_equals_only() {
        let explicit = tui_args(&[
            "cflx",
            "tui",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(
            explicit.upstream_integration().unwrap(),
            Some(crate::upstream::UpstreamIntegrationConfig::new(
                "upstream",
                "cargo test"
            ))
        );

        let bare = bare_tui_args(&[
            "cflx",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(bare.integrate_upstream.as_deref(), Some("upstream"));
        assert_eq!(bare.push, None, "upstream must not configure push mode");

        // A space-separated remote is not an option value for the value-less alias.
        let spaced = tui_args(&[
            "cflx",
            "tui",
            "--integrate-upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        assert_eq!(spaced.integrate_upstream.as_deref(), Some("origin"));
    }

    #[test]
    fn per_change_upstream_tui_rejects_push() {
        let with_push = tui_args(&[
            "cflx",
            "tui",
            "-u",
            "--upstream-verify-command",
            "cargo test",
            "--push",
        ]);
        assert_eq!(
            with_push.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::ConflictsWithPush)
        );

        let bare_push = bare_tui_args(&[
            "cflx",
            "-u",
            "--upstream-verify-command",
            "cargo test",
            "--push",
        ]);
        assert_eq!(
            bare_push.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::ConflictsWithPush)
        );
    }

    #[test]
    fn per_change_upstream_tui_requires_verify_command() {
        let missing = tui_args(&["cflx", "tui", "-u"]);
        assert_eq!(
            missing.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::MissingVerifyCommand)
        );

        let orphan_command = tui_args(&["cflx", "tui", "--upstream-verify-command", "cargo test"]);
        assert_eq!(
            orphan_command.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::VerifyCommandWithoutOption)
        );
    }

    #[test]
    fn per_change_upstream_run_rejects_push_combination_at_parse_time() {
        let args = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "-u",
            "--upstream-verify-command",
            "cargo test",
            "--push",
        ]);
        assert_eq!(
            args.upstream_integration(),
            Err(crate::upstream::UpstreamOptionError::ConflictsWithPush)
        );
    }

    #[test]
    fn per_change_upstream_short_flag_never_carries_a_remote() {
        // A named remote is spec-restricted to `--integrate-upstream=<remote>`,
        // so the short spelling must take no value on any of the three
        // entrypoints while the long spelling keeps accepting one.
        for argv in [
            vec!["cflx", "run", "--all", "--parallel", "-u=upstream"],
            vec!["cflx", "-u=upstream"],
            vec!["cflx", "tui", "-u=upstream"],
        ] {
            let err = Cli::try_parse_from(&argv)
                .err()
                .unwrap_or_else(|| panic!("-u must not accept a remote value: {:?}", argv));
            assert_ne!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp,
                "rejection must be a parse error, not help: {:?}",
                argv
            );
        }

        // The equals form still parses everywhere and selects the named remote.
        let run = run_args(&[
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        let bare = bare_tui_args(&[
            "cflx",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        let explicit = tui_args(&[
            "cflx",
            "tui",
            "--integrate-upstream=upstream",
            "--upstream-verify-command",
            "cargo test",
        ]);
        let expected = Some(crate::upstream::UpstreamIntegrationConfig::new(
            "upstream",
            "cargo test",
        ));
        assert_eq!(run.upstream_integration().unwrap(), expected);
        assert_eq!(bare.upstream_integration().unwrap(), expected);
        assert_eq!(explicit.upstream_integration().unwrap(), expected);
    }

    #[test]
    fn per_change_upstream_short_flag_still_selects_the_default_remote() {
        // Removing the short alias from the valued argument must not change what
        // a value-less `-u` resolves to.
        for args in [
            run_args(&[
                "cflx",
                "run",
                "--all",
                "--parallel",
                "-u",
                "--upstream-verify-command",
                "cargo test",
            ])
            .upstream_integration(),
            bare_tui_args(&["cflx", "-u", "--upstream-verify-command", "cargo test"])
                .upstream_integration(),
            tui_args(&[
                "cflx",
                "tui",
                "-u",
                "--upstream-verify-command",
                "cargo test",
            ])
            .upstream_integration(),
        ] {
            assert_eq!(
                args.unwrap(),
                Some(crate::upstream::UpstreamIntegrationConfig::new(
                    "origin",
                    "cargo test"
                ))
            );
        }
    }

    #[test]
    fn per_change_upstream_top_level_options_are_rejected_before_a_subcommand() {
        // `cflx -u run ...` parses, but `Commands::Run` reads its own options, so
        // the opt-in would be dropped and the run would succeed in merged mode.
        for (argv, option) in [
            (vec!["cflx", "-u", "run", "--all", "--parallel"], "-u"),
            (
                vec!["cflx", "--integrate-upstream=upstream", "run", "--all"],
                "--integrate-upstream",
            ),
            (
                vec![
                    "cflx",
                    "--upstream-verify-command",
                    "cargo test",
                    "run",
                    "--all",
                ],
                "--upstream-verify-command",
            ),
            (vec!["cflx", "-u", "tui"], "-u"),
        ] {
            let cli = Cli::try_parse_from(&argv)
                .unwrap_or_else(|e| panic!("expected {:?} to parse: {e}", argv));
            let error = cli
                .validate_upstream_option_placement()
                .expect_err(&format!("{:?} must not silently drop the opt-in", argv));
            assert_eq!(error.option, option, "{:?}", argv);
            assert!(
                error.to_string().contains(error.subcommand),
                "the diagnostic must name the subcommand: {error}"
            );
        }
    }

    #[test]
    fn per_change_upstream_top_level_options_stay_valid_for_bare_invocation() {
        let bare = Cli::try_parse_from(["cflx", "-u", "--upstream-verify-command", "cargo test"])
            .expect("bare invocation parses");
        assert_eq!(bare.validate_upstream_option_placement(), Ok(()));

        let plain_subcommand =
            Cli::try_parse_from(["cflx", "run", "--all", "--parallel"]).expect("run parses");
        assert_eq!(
            plain_subcommand.validate_upstream_option_placement(),
            Ok(())
        );
    }

    #[test]
    fn per_change_upstream_verify_command_help_states_it_is_rejected_alone() {
        use clap::CommandFactory;
        let mut command = Cli::command();
        let run = command
            .find_subcommand_mut("run")
            .expect("run subcommand")
            .render_long_help()
            .to_string();
        let tui = command
            .find_subcommand_mut("tui")
            .expect("tui subcommand")
            .render_long_help()
            .to_string();
        let top = Cli::command().render_long_help().to_string();

        for (entrypoint, help) in [("run", &run), ("tui", &tui), ("bare", &top)] {
            assert!(
                help.contains("Requires `-u`/`--integrate-upstream`; rejected without it."),
                "{entrypoint} help must not claim the verification command is ignored: {help}"
            );
            assert!(
                !help.contains("ignored otherwise"),
                "{entrypoint} help retains the stale wording: {help}"
            );
        }
    }

    #[test]
    fn per_change_upstream_is_exposed_in_tui_help() {
        use clap::CommandFactory;
        let help = Cli::command()
            .find_subcommand_mut("tui")
            .expect("tui subcommand")
            .render_long_help()
            .to_string();
        assert!(help.contains("--integrate-upstream"), "help: {}", help);
        assert!(help.contains("--upstream-verify-command"), "help: {}", help);

        let top = Cli::command().render_long_help().to_string();
        assert!(top.contains("--integrate-upstream"), "help: {}", top);
    }

    #[test]
    fn test_completion_subcommand_supported_shells() {
        for (shell_name, expected) in [
            ("zsh", CompletionShell::Zsh),
            ("bash", CompletionShell::Bash),
            ("fish", CompletionShell::Fish),
            ("powershell", CompletionShell::PowerShell),
        ] {
            let cli = Cli::parse_from(["cflx", "completion", shell_name]);
            match cli.command {
                Some(Commands::Completion(args)) => assert_eq!(args.shell, expected),
                _ => panic!("Expected Completion subcommand for {shell_name}"),
            }
        }
    }

    #[test]
    fn test_completion_subcommand_rejects_unsupported_shell() {
        for shell in ["tcsh", "elvish"] {
            let err = Cli::try_parse_from(["cflx", "completion", shell]).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
        }
    }

    #[test]
    fn test_internal_change_id_completion_defaults() {
        let cli = Cli::parse_from(["cflx", "__complete", "change-ids"]);
        match cli.command {
            Some(Commands::Complete(args)) => match args.command {
                InternalCompleteCommands::ChangeIds(change_args) => {
                    assert!(!change_args.active);
                    assert!(!change_args.archived);
                    assert_eq!(change_args.prefix, None);
                }
            },
            _ => panic!("Expected hidden Complete subcommand"),
        }
    }

    #[test]
    fn test_internal_change_id_completion_flags() {
        let cli = Cli::parse_from([
            "cflx",
            "__complete",
            "change-ids",
            "--active",
            "--archived",
            "--prefix",
            "add-",
        ]);
        match cli.command {
            Some(Commands::Complete(args)) => match args.command {
                InternalCompleteCommands::ChangeIds(change_args) => {
                    assert!(change_args.active);
                    assert!(change_args.archived);
                    assert_eq!(change_args.prefix.as_deref(), Some("add-"));
                }
            },
            _ => panic!("Expected hidden Complete subcommand"),
        }
    }

    #[test]
    fn test_internal_complete_rejects_invalid_mode() {
        let err = Cli::try_parse_from(["cflx", "__complete", "spec-ids"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn test_logs_subcommand_flags() {
        let cli = Cli::parse_from([
            "cflx",
            "logs",
            "--path",
            "--last",
            "50",
            "--follow",
            "--today",
            "--project",
            "conflux-test",
        ]);

        match cli.command {
            Some(Commands::Logs(args)) => {
                assert!(args.path);
                assert_eq!(args.last, Some(50));
                assert!(args.follow);
                assert!(args.today);
                assert_eq!(args.project.as_deref(), Some("conflux-test"));
            }
            _ => panic!("Expected Logs subcommand"),
        }
    }

    #[test]
    fn test_logs_help_documents_flags() {
        let err = Cli::try_parse_from(["cflx", "logs", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(help.contains("--path"));
        assert!(help.contains("--last <N>"));
        assert!(help.contains("--follow"));
        assert!(help.contains("--today"));
        assert!(help.contains("--project <SLUG>"));
    }

    #[test]
    fn test_logs_subcommand_default_mode() {
        let cli = Cli::parse_from(["cflx", "logs"]);

        match cli.command {
            Some(Commands::Logs(args)) => {
                assert!(!args.path);
                assert_eq!(args.last, None);
                assert!(!args.follow);
                assert!(!args.today);
                assert!(args.project.is_none());
            }
            _ => panic!("Expected Logs subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_config_option() {
        let cli = Cli::parse_from(["cflx", "run", "--all", "--config", "/path/to/config.jsonc"]);

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
    fn test_run_subcommand_requires_explicit_target() {
        let err = Cli::try_parse_from(["cflx", "run"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_run_subcommand_all_target() {
        let cli = Cli::parse_from(["cflx", "run", "--all"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.all);
                assert_eq!(args.normalized_target_changes(), None);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_positional_changes() {
        let cli = Cli::parse_from(["cflx", "run", "a", "b"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.changes, vec!["a".to_string(), "b".to_string()]);
                assert_eq!(
                    args.normalized_target_changes(),
                    Some(vec!["a".to_string(), "b".to_string()])
                );
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_rejects_target_mode_combinations() {
        for argv in [
            vec!["cflx", "run", "--all", "a"],
            vec!["cflx", "run", "--all", "--change", "a"],
            vec!["cflx", "run", "--change", "a", "b"],
        ] {
            let err = Cli::try_parse_from(argv).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn test_no_subcommand() {
        let cli = Cli::parse_from(["cflx"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn tui_default_push_defaults_to_origin() {
        let cli = Cli::parse_from(["cflx", "--push"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.push.as_deref(), Some("origin"));
    }

    #[test]
    fn tui_default_push_accepts_remote_name() {
        let cli = Cli::parse_from(["cflx", "--push", "upstream"]);
        assert!(cli.command.is_none());
        assert_eq!(cli.push.as_deref(), Some("upstream"));
    }

    #[test]
    fn tui_default_push_rejects_branch_selection() {
        let err = Cli::try_parse_from(["cflx", "--push", "origin:main"]).unwrap_err();
        assert!(err
            .to_string()
            .contains("branch selection is not supported"));
    }

    #[test]
    fn tui_subcommand_push_defaults_to_origin() {
        let cli = Cli::parse_from(["cflx", "tui", "--push"]);
        match cli.command {
            Some(Commands::Tui(args)) => assert_eq!(args.push.as_deref(), Some("origin")),
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn tui_subcommand_push_accepts_remote_name() {
        let cli = Cli::parse_from(["cflx", "tui", "--push", "upstream"]);
        match cli.command {
            Some(Commands::Tui(args)) => assert_eq!(args.push.as_deref(), Some("upstream")),
            _ => panic!("Expected Tui subcommand"),
        }
    }

    #[test]
    fn tui_subcommand_push_rejects_branch_selection() {
        let err = Cli::try_parse_from(["cflx", "tui", "--push", "origin:main"]).unwrap_err();
        assert!(err
            .to_string()
            .contains("branch selection is not supported"));
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
        let cli = Cli::parse_from(["cflx", "run", "--all"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.max_iterations.is_none());
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_custom() {
        let cli = Cli::parse_from(["cflx", "run", "--all", "--max-iterations", "100"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(100));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_max_iterations_zero() {
        let cli = Cli::parse_from(["cflx", "run", "--all", "--max-iterations", "0"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.max_iterations, Some(0));
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn test_run_subcommand_parallel_flag_default() {
        let cli = Cli::parse_from(["cflx", "run", "--all"]);

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
        let cli = Cli::parse_from(["cflx", "run", "--all", "--parallel"]);

        match cli.command {
            Some(Commands::Run(args)) => {
                assert!(args.parallel);
            }
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn cli_push_defaults_to_origin() {
        let cli = Cli::parse_from(["cflx", "run", "--all", "--parallel", "--push"]);
        match cli.command {
            Some(Commands::Run(args)) => assert_eq!(args.push.as_deref(), Some("origin")),
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn cli_push_accepts_remote_name() {
        let cli = Cli::parse_from(["cflx", "run", "--all", "--parallel", "--push", "upstream"]);
        match cli.command {
            Some(Commands::Run(args)) => assert_eq!(args.push.as_deref(), Some("upstream")),
            _ => panic!("Expected Run subcommand"),
        }
    }

    #[test]
    fn cli_push_rejects_branch_selection() {
        let err = Cli::try_parse_from([
            "cflx",
            "run",
            "--all",
            "--parallel",
            "--push",
            "origin:main",
        ])
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("branch selection is not supported"));
    }

    #[test]
    fn test_run_subcommand_max_concurrent() {
        let cli = Cli::parse_from([
            "cflx",
            "run",
            "--all",
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
        let cli = Cli::parse_from(["cflx", "run", "--all", "--parallel", "--dry-run"]);

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
        let cli = Cli::parse_from(["cflx", "run", "--all", "--web"]);

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
        let cli = Cli::parse_from(["cflx", "run", "--all", "--web", "--web-port", "9000"]);

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

    // Removed multi-project server surfaces must fail at parse time.
    //
    // Clap rejection is the whole contract: an operator who still has the old
    // flags in a script gets a usage error before any logging, repository lock,
    // network connection, or orchestration side effect can run.
    #[test]
    fn removed_top_level_server_options_are_rejected() {
        for argv in [
            vec!["cflx", "--server", "http://127.0.0.1:39876"],
            vec!["cflx", "--server-token", "mytoken"],
            vec!["cflx", "--server-token-env", "MY_TOKEN_VAR"],
        ] {
            let err = Cli::try_parse_from(&argv)
                .expect_err("removed top-level server option must not parse");
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::UnknownArgument,
                "unexpected error for {:?}: {}",
                argv,
                err
            );
        }
    }

    #[test]
    fn removed_tui_server_options_are_rejected() {
        for argv in [
            vec!["cflx", "tui", "--server", "http://127.0.0.1:39876"],
            vec!["cflx", "tui", "--server-token", "mytoken"],
            vec!["cflx", "tui", "--server-token-env", "MY_TOKEN_VAR"],
        ] {
            let err =
                Cli::try_parse_from(&argv).expect_err("removed tui server option must not parse");
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::UnknownArgument,
                "unexpected error for {:?}: {}",
                argv,
                err
            );
        }
    }

    #[test]
    fn removed_server_service_and_project_subcommands_are_rejected() {
        for argv in [
            vec!["cflx", "server"],
            vec!["cflx", "server", "--port", "39876"],
            vec!["cflx", "service", "install"],
            vec!["cflx", "service", "status"],
            vec!["cflx", "project", "status"],
            vec!["cflx", "project", "add", "https://github.com/org/repo.git"],
            vec!["cflx", "project", "sync", "--all"],
        ] {
            let err = Cli::try_parse_from(&argv).expect_err("removed subcommand must not parse");
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::InvalidSubcommand,
                "unexpected error for {:?}: {}",
                argv,
                err
            );
        }
    }

    #[test]
    fn help_no_longer_advertises_removed_server_surfaces() {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        let long_help = cmd.render_long_help().to_string();
        for needle in [
            "--server",
            "--server-token",
            "--server-token-env",
            "cflx server",
            "cflx project",
            "cflx service",
        ] {
            assert!(
                !long_help.contains(needle),
                "top-level help still advertises '{}'",
                needle
            );
        }

        let mut tui = cmd
            .find_subcommand_mut("tui")
            .expect("tui subcommand")
            .clone();
        let tui_help = tui.render_long_help().to_string();
        for needle in ["--server", "--server-token", "--server-token-env"] {
            assert!(
                !tui_help.contains(needle),
                "tui help still advertises '{}'",
                needle
            );
        }
        // The retained local web surface must still be advertised.
        assert!(tui_help.contains("--web"));
        assert!(tui_help.contains("--web-auth-token"));
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
        let cli = Cli::try_parse_from(["cflx", "run", "--all", "--web"]).unwrap();
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

    // ── install-skills subcommand tests ──────────────────────────────────────

    #[test]
    fn test_install_skills_no_args() {
        let cli = Cli::parse_from(["cflx", "install-skills"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert!(!args.global);
                assert!(!args.claude);
                assert_eq!(args.target(), InstallSkillsTarget::Agents);
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    #[test]
    fn test_install_skills_global_flag() {
        let cli = Cli::parse_from(["cflx", "install-skills", "--global"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert!(args.global);
                assert!(!args.claude);
                assert_eq!(args.target(), InstallSkillsTarget::Agents);
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    #[test]
    fn test_install_skills_claude_flag() {
        let cli = Cli::parse_from(["cflx", "install-skills", "--claude"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert!(!args.global);
                assert!(args.claude);
                assert_eq!(args.target(), InstallSkillsTarget::Claude);
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    #[test]
    fn test_install_skills_claude_and_global_flags() {
        let cli = Cli::parse_from(["cflx", "install-skills", "--claude", "--global"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert!(args.global);
                assert!(args.claude);
                assert_eq!(args.target(), InstallSkillsTarget::Claude);
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    #[test]
    fn test_install_skills_legacy_self_arg_captured() {
        // Legacy "self" positional argument is captured so we can emit migration guidance
        let cli = Cli::parse_from(["cflx", "install-skills", "self"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert_eq!(args.legacy_source.as_deref(), Some("self"));
                let msg = install_skills_legacy_error("self");
                assert!(
                    msg.contains("cflx install-skills"),
                    "Migration guidance must mention 'cflx install-skills'"
                );
                assert!(
                    msg.contains("--global"),
                    "Migration guidance must mention '--global'"
                );
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    #[test]
    fn test_install_skills_legacy_local_arg_captured() {
        // Legacy "local:..." positional argument is captured so we can emit migration guidance
        let cli = Cli::parse_from(["cflx", "install-skills", "local:../my-skills"]);
        match cli.command {
            Some(Commands::InstallSkills(args)) => {
                assert_eq!(args.legacy_source.as_deref(), Some("local:../my-skills"));
                let msg = install_skills_legacy_error("local:../my-skills");
                assert!(
                    msg.contains("cflx install-skills"),
                    "Migration guidance must mention 'cflx install-skills'"
                );
                assert!(
                    msg.contains("--global"),
                    "Migration guidance must mention '--global'"
                );
            }
            _ => panic!("Expected InstallSkills subcommand"),
        }
    }

    // ── openspec subcommand tests ──────────────────────────────────────────

    #[test]
    fn test_openspec_list_default() {
        let cli = Cli::parse_from(["cflx", "openspec", "list"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::List(list_args) => {
                    assert!(!list_args.specs);
                }
                _ => panic!("Expected List subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_list_specs_flag() {
        let cli = Cli::parse_from(["cflx", "openspec", "list", "--specs"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::List(list_args) => {
                    assert!(list_args.specs);
                }
                _ => panic!("Expected List subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_show_basic() {
        let cli = Cli::parse_from(["cflx", "openspec", "show", "my-change"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Show(show_args) => {
                    assert_eq!(show_args.change_id, "my-change");
                    assert!(!show_args.json);
                    assert!(!show_args.deltas_only);
                }
                _ => panic!("Expected Show subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_show_json_deltas_only() {
        let cli = Cli::parse_from([
            "cflx",
            "openspec",
            "show",
            "my-change",
            "--json",
            "--deltas-only",
        ]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Show(show_args) => {
                    assert_eq!(show_args.change_id, "my-change");
                    assert!(show_args.json);
                    assert!(show_args.deltas_only);
                }
                _ => panic!("Expected Show subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_validate_all_default() {
        let cli = Cli::parse_from(["cflx", "openspec", "validate"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Validate(val_args) => {
                    assert!(val_args.change_id.is_none());
                    assert!(!val_args.strict);
                    assert!(!val_args.archive_gate);
                    assert!(matches!(val_args.evidence, super::EvidenceMode::Off));
                }
                _ => panic!("Expected Validate subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_validate_strict_with_change() {
        let cli = Cli::parse_from(["cflx", "openspec", "validate", "my-change", "--strict"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Validate(val_args) => {
                    assert_eq!(val_args.change_id, Some("my-change".to_string()));
                    assert!(val_args.strict);
                }
                _ => panic!("Expected Validate subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_validate_evidence_modes() {
        for (flag, expected) in [("off", "Off"), ("warn", "Warn"), ("error", "Error")] {
            let cli = Cli::parse_from(["cflx", "openspec", "validate", "--evidence", flag]);
            match cli.command {
                Some(Commands::Openspec(args)) => match args.command {
                    super::OpenspecCommands::Validate(val_args) => {
                        let actual = format!("{:?}", val_args.evidence);
                        assert_eq!(
                            actual, expected,
                            "Evidence mode mismatch for flag '{}'",
                            flag
                        );
                    }
                    _ => panic!("Expected Validate subcommand"),
                },
                _ => panic!("Expected Openspec subcommand"),
            }
        }
    }

    #[test]
    fn test_openspec_validate_rejects_strict_as_evidence_mode_name() {
        use clap::Parser;

        let parsed = Cli::try_parse_from(["cflx", "openspec", "validate", "--evidence", "strict"]);

        assert!(parsed.is_err(), "strict evidence mode should be rejected");
    }

    #[test]
    fn test_openspec_validate_archive_gate_flag() {
        let cli = Cli::parse_from([
            "cflx",
            "openspec",
            "validate",
            "my-change",
            "--archive-gate",
        ]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Validate(val_args) => {
                    assert_eq!(val_args.change_id, Some("my-change".to_string()));
                    assert!(val_args.archive_gate);
                }
                _ => panic!("Expected Validate subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_archive_basic() {
        let cli = Cli::parse_from(["cflx", "openspec", "archive", "my-change", "--yes"]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Archive(arc_args) => {
                    assert_eq!(arc_args.change_id, "my-change");
                    assert!(arc_args.yes);
                    assert!(!arc_args.skip_specs);
                }
                _ => panic!("Expected Archive subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
        }
    }

    #[test]
    fn test_openspec_archive_skip_specs() {
        let cli = Cli::parse_from([
            "cflx",
            "openspec",
            "archive",
            "my-change",
            "--yes",
            "--skip-specs",
        ]);
        match cli.command {
            Some(Commands::Openspec(args)) => match args.command {
                super::OpenspecCommands::Archive(arc_args) => {
                    assert_eq!(arc_args.change_id, "my-change");
                    assert!(arc_args.yes);
                    assert!(arc_args.skip_specs);
                }
                _ => panic!("Expected Archive subcommand"),
            },
            _ => panic!("Expected Openspec subcommand"),
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
        assert!(
            help_text.contains("~/.config/cflx/tui.jsonc"),
            "Help should mention TUI user config path"
        );
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

/// Parser coverage for the `/api/v2` remote-control web options.
///
/// Named for the capability rather than for `cli` so the change's verification
/// filter (`cargo test --lib remote_control_api`) reaches the CLI surface too:
/// a config that only fails at runtime is not a safety property.
#[cfg(test)]
mod remote_control_api_cli_tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap_or_else(|e| panic!("{args:?} must parse: {e}"))
    }

    fn parse_err(args: &[&str]) -> String {
        Cli::try_parse_from(args)
            .err()
            .unwrap_or_else(|| panic!("{args:?} must be rejected"))
            .to_string()
    }

    #[test]
    fn web_auth_options_default_to_absent_in_every_web_enabled_scope() {
        let root = parse(&["cflx", "--web"]);
        assert_eq!(root.web_auth_token, None);
        assert_eq!(root.web_auth_token_env, None);
        assert!(root.web_allowed_origins.is_empty());

        match parse(&["cflx", "run", "--all", "--web"]).command {
            Some(Commands::Run(args)) => {
                assert_eq!(args.web_auth_token, None);
                assert_eq!(args.web_auth_token_env, None);
                assert!(args.web_allowed_origins.is_empty());
            }
            other => panic!("expected run, got {other:?}"),
        }

        match parse(&["cflx", "tui", "--web"]).command {
            Some(Commands::Tui(args)) => {
                assert_eq!(args.web_auth_token, None);
                assert_eq!(args.web_auth_token_env, None);
                assert!(args.web_allowed_origins.is_empty());
            }
            other => panic!("expected tui, got {other:?}"),
        }
    }

    #[test]
    fn a_literal_token_parses_in_every_web_enabled_scope() {
        assert_eq!(
            parse(&["cflx", "--web", "--web-auth-token", "abc"]).web_auth_token,
            Some("abc".to_string())
        );
        match parse(&["cflx", "run", "--all", "--web", "--web-auth-token", "abc"]).command {
            Some(Commands::Run(args)) => assert_eq!(args.web_auth_token, Some("abc".to_string())),
            other => panic!("expected run, got {other:?}"),
        }
        match parse(&["cflx", "tui", "--web", "--web-auth-token", "abc"]).command {
            Some(Commands::Tui(args)) => assert_eq!(args.web_auth_token, Some("abc".to_string())),
            other => panic!("expected tui, got {other:?}"),
        }
    }

    #[test]
    fn the_environment_form_parses_in_every_web_enabled_scope() {
        assert_eq!(
            parse(&["cflx", "--web", "--web-auth-token-env", "CFLX_TOKEN"]).web_auth_token_env,
            Some("CFLX_TOKEN".to_string())
        );
        match parse(&[
            "cflx",
            "run",
            "--all",
            "--web",
            "--web-auth-token-env",
            "CFLX_TOKEN",
        ])
        .command
        {
            Some(Commands::Run(args)) => {
                assert_eq!(args.web_auth_token_env, Some("CFLX_TOKEN".to_string()))
            }
            other => panic!("expected run, got {other:?}"),
        }
        match parse(&["cflx", "tui", "--web", "--web-auth-token-env", "CFLX_TOKEN"]).command {
            Some(Commands::Tui(args)) => {
                assert_eq!(args.web_auth_token_env, Some("CFLX_TOKEN".to_string()))
            }
            other => panic!("expected tui, got {other:?}"),
        }
    }

    #[test]
    fn token_sources_are_mutually_exclusive_in_every_web_enabled_scope() {
        for prefix in [
            vec!["cflx"],
            vec!["cflx", "run", "--all"],
            vec!["cflx", "tui"],
        ] {
            let mut args = prefix.clone();
            args.extend([
                "--web",
                "--web-auth-token",
                "a",
                "--web-auth-token-env",
                "V",
            ]);
            let error = parse_err(&args);
            assert!(
                error.contains("cannot be used with"),
                "{prefix:?} must reject both token sources, got: {error}"
            );
        }
    }

    #[test]
    fn allowed_origins_are_repeatable_in_every_web_enabled_scope() {
        let root = parse(&[
            "cflx",
            "--web",
            "--web-allowed-origin",
            "https://ops.example.com",
            "--web-allowed-origin",
            "http://localhost:5173",
        ]);
        assert_eq!(
            root.web_allowed_origins,
            vec![
                "https://ops.example.com".to_string(),
                "http://localhost:5173".to_string()
            ]
        );

        match parse(&[
            "cflx",
            "run",
            "--all",
            "--web",
            "--web-allowed-origin",
            "https://a.example",
            "--web-allowed-origin",
            "https://b.example",
        ])
        .command
        {
            Some(Commands::Run(args)) => assert_eq!(args.web_allowed_origins.len(), 2),
            other => panic!("expected run, got {other:?}"),
        }

        match parse(&[
            "cflx",
            "tui",
            "--web",
            "--web-allowed-origin",
            "https://a.example",
            "--web-allowed-origin",
            "https://b.example",
        ])
        .command
        {
            Some(Commands::Tui(args)) => assert_eq!(args.web_allowed_origins.len(), 2),
            other => panic!("expected tui, got {other:?}"),
        }
    }

    #[test]
    fn help_documents_the_literal_token_exposure_and_the_recommended_form() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--web-auth-token"));
        assert!(help.contains("--web-auth-token-env"));
        assert!(help.contains("--web-allowed-origin"));
        assert!(
            help.contains("inspect this process's arguments"),
            "the literal-token exposure must be documented where an operator will read it"
        );
        assert!(
            help.contains("wildcards are\nrejected") || help.contains("wildcards are rejected"),
            "the exact-origin rule must be documented"
        );
    }

    /// `WebConfig` only exists under `web-monitoring`, so the wiring assertion is
    /// gated while the parser assertions above stay feature-independent.
    #[test]
    #[cfg(feature = "web-monitoring")]
    fn parsed_web_options_flow_into_a_validated_web_config() {
        let args = parse(&[
            "cflx",
            "--web",
            "--web-bind",
            "0.0.0.0",
            "--web-port",
            "9000",
            "--web-auth-token",
            "abc",
            "--web-allowed-origin",
            "https://ops.example.com",
        ]);
        let config = crate::web::WebConfig::enabled(args.web_port, args.web_bind.clone())
            .with_auth(
                args.web_auth_token.clone(),
                args.web_auth_token_env.clone(),
                args.web_allowed_origins.clone(),
            );
        assert!(config.validate().is_ok());
        assert_eq!(config.resolve_auth_token().as_deref(), Some("abc"));

        let unsafe_args = parse(&["cflx", "--web", "--web-bind", "0.0.0.0"]);
        let unsafe_config =
            crate::web::WebConfig::enabled(unsafe_args.web_port, unsafe_args.web_bind.clone())
                .with_auth(None, None, Vec::new());
        assert!(
            unsafe_config.validate().is_err(),
            "a routable bind without credentials must never reach a listener"
        );
    }
}
