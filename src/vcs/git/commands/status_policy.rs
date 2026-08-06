//! One child-command-local policy for Conflux-owned read-only `git status`.
//!
//! Git may take `.git/index.lock` during an ordinary `git status` purely to
//! persist a refreshed stat cache. That write is optional: the status answer is
//! already correct without it. A long-lived Conflux frontend polls status about
//! once a second, so that optional write can land exactly while an `on_merged`
//! hook, a release command, Apply/Archive finalization, or an operator holds the
//! same index — and the authorized *mutation* is the one that fails.
//!
//! Every read-only status observation Conflux owns therefore runs as:
//!
//! ```text
//! git --no-optional-locks status <observation arguments>
//! ```
//!
//! Two properties of that shape are load-bearing and are asserted by tests:
//!
//! 1. `--no-optional-locks` is a Git *global* option, so it must precede the
//!    `status` subcommand. Git rejects it once the subcommand has been parsed.
//! 2. Suppression is expressed as a child-command argument, never as a
//!    `GIT_OPTIONAL_LOCKS` environment variable. The environment would be
//!    process-wide and would weaken every repo-mutating Git command Conflux and
//!    its descendants run, which is the opposite of what this policy protects.
//!
//! The policy is status-specific. Other read-only Git commands (for example a
//! worktree-scoped `git diff`) may refresh index stat data through paths this
//! option does not gate; that is out of scope here rather than silently assumed.
//!
//! Each observation keeps its own argument set below instead of collapsing into
//! one helper, because the callers genuinely differ: trimmed booleans, untrimmed
//! porcelain columns, explicit untracked/ignored modes, pathspec scope,
//! human-readable text, and porcelain v2 are not interchangeable.

/// Git's global option that suppresses opportunistic index writes.
pub const NO_OPTIONAL_LOCKS: &str = "--no-optional-locks";

/// The Git subcommand this policy governs.
pub const STATUS_SUBCOMMAND: &str = "status";

/// Human-readable `git status`, captured as conflict-resolution prompt context.
pub const HUMAN_READABLE_STATUS_ARGS: &[&str] = &[];

/// Plain porcelain v1, used for trimmed clean/dirty classification.
pub const PORCELAIN_STATUS_ARGS: &[&str] = &["--porcelain"];

/// Porcelain v1 with untracked and ignored modes stated explicitly.
///
/// The modes are passed rather than inherited because `status.showUntrackedFiles`
/// in repository or user configuration would otherwise let a worktree full of
/// unselected work report as clean.
pub const DIRTY_STATE_STATUS_ARGS: &[&str] =
    &["--porcelain", "--untracked-files=normal", "--ignored=no"];

/// Porcelain v1 including untracked files, for exact-match-with-`HEAD` checks.
pub const PORCELAIN_UNTRACKED_STATUS_ARGS: &[&str] = &["--porcelain", "--untracked-files=normal"];

/// Porcelain v1 used by read-oriented change monitoring (TUI refresh, queue
/// filtering, parallel startup).
pub const CHANGE_MONITOR_STATUS_ARGS: &[&str] = &["--porcelain", "-u"];

/// Porcelain v1 terminated by `--` so the caller can append a pathspec.
pub const PATH_SCOPED_PORCELAIN_STATUS_ARGS: &[&str] = &["--porcelain", "--"];

/// Porcelain v2, used for structural upstream failure classification.
pub const PORCELAIN_V2_STATUS_ARGS: &[&str] = &["--porcelain=v2"];

/// Build the argv of a read-only `git status` observation.
///
/// `status_args` are the arguments that follow the subcommand; the global
/// option and the subcommand itself are supplied here so no call site can get
/// the ordering wrong.
pub fn read_only_status_argv<'a>(status_args: &[&'a str]) -> Vec<&'a str> {
    let mut argv = Vec::with_capacity(status_args.len() + 2);
    argv.push(NO_OPTIONAL_LOCKS);
    argv.push(STATUS_SUBCOMMAND);
    argv.extend_from_slice(status_args);
    argv
}

/// Render a read-only status observation as the command line it actually runs.
///
/// Operator-facing text that claims to show the exact query Conflux issued must
/// be built from this, so the description cannot drift from the argv.
pub fn read_only_status_command_display(status_args: &[&str]) -> String {
    let mut rendered = String::from("git");
    for arg in read_only_status_argv(status_args) {
        rendered.push(' ');
        rendered.push_str(arg);
    }
    rendered
}

/// Repository-scoped regressions for this policy: real Git fixtures, index-byte
/// safety, classification fidelity, and the production argv inventory.
#[cfg(test)]
#[path = "status_policy_repository_tests.rs"]
mod native_git_status_optional_locks_repository;

#[cfg(test)]
mod native_git_status_optional_locks {
    use super::*;

    /// Assert one argv is a well-formed read-only status command.
    fn assert_read_only_status_shape(argv: &[&str]) {
        let option_index = argv
            .iter()
            .position(|arg| *arg == NO_OPTIONAL_LOCKS)
            .unwrap_or_else(|| panic!("{argv:?} must disable optional index locks"));
        let subcommand_index = argv
            .iter()
            .position(|arg| *arg == STATUS_SUBCOMMAND)
            .unwrap_or_else(|| panic!("{argv:?} must run the status subcommand"));
        assert!(
            option_index < subcommand_index,
            "`{NO_OPTIONAL_LOCKS}` is a Git global option and must precede `status`: {argv:?}"
        );
        assert_eq!(
            option_index, 0,
            "the global option belongs before every other argument: {argv:?}"
        );
    }

    #[test]
    fn policy_builder_puts_the_global_option_before_the_subcommand() {
        assert_eq!(
            read_only_status_argv(&["--porcelain"]),
            vec!["--no-optional-locks", "status", "--porcelain"]
        );
        assert_eq!(
            read_only_status_argv(&[]),
            vec!["--no-optional-locks", "status"]
        );
        assert_read_only_status_shape(&read_only_status_argv(&["--porcelain"]));
        assert_read_only_status_shape(&read_only_status_argv(&[]));
    }

    #[test]
    fn every_declared_observation_keeps_its_exact_argv() {
        let cases: [(&str, &[&str], &[&str]); 7] = [
            (
                "human readable",
                HUMAN_READABLE_STATUS_ARGS,
                &["--no-optional-locks", "status"],
            ),
            (
                "porcelain",
                PORCELAIN_STATUS_ARGS,
                &["--no-optional-locks", "status", "--porcelain"],
            ),
            (
                "dirty state",
                DIRTY_STATE_STATUS_ARGS,
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "--untracked-files=normal",
                    "--ignored=no",
                ],
            ),
            (
                "porcelain untracked",
                PORCELAIN_UNTRACKED_STATUS_ARGS,
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "--untracked-files=normal",
                ],
            ),
            (
                "change monitor",
                CHANGE_MONITOR_STATUS_ARGS,
                &["--no-optional-locks", "status", "--porcelain", "-u"],
            ),
            (
                "path scoped",
                PATH_SCOPED_PORCELAIN_STATUS_ARGS,
                &["--no-optional-locks", "status", "--porcelain", "--"],
            ),
            (
                "porcelain v2",
                PORCELAIN_V2_STATUS_ARGS,
                &["--no-optional-locks", "status", "--porcelain=v2"],
            ),
        ];

        for (name, args, expected) in cases {
            let argv = read_only_status_argv(args);
            assert_eq!(argv, expected, "{name} observation argv drifted");
            assert_read_only_status_shape(&argv);
        }
    }

    #[test]
    fn command_display_matches_the_argv_it_describes() {
        assert_eq!(
            read_only_status_command_display(DIRTY_STATE_STATUS_ARGS),
            "git --no-optional-locks status --porcelain --untracked-files=normal --ignored=no"
        );
        assert_eq!(
            read_only_status_command_display(HUMAN_READABLE_STATUS_ARGS),
            "git --no-optional-locks status"
        );
    }

    #[test]
    fn suppression_is_never_delivered_through_process_environment() {
        // A process-wide GIT_OPTIONAL_LOCKS would also weaken repo-mutating Git
        // commands and every descendant, which this policy exists to avoid.
        assert!(
            std::env::var_os("GIT_OPTIONAL_LOCKS").is_none(),
            "optional-lock suppression must stay child-command-local"
        );
    }
}
