//! Startup preflight for local orchestration-owning entrypoints.
//!
//! Bare `cflx`, `cflx tui`, and `cflx run` become the repository's local
//! orchestration owner. Ownership is scoped to the canonical Git *common*
//! directory, which every linked worktree of one repository shares — so without
//! this preflight an owner started inside a linked worktree would adopt that
//! worktree as its base workspace and cut managed worktrees from it. Nested
//! orchestration is what produces a managed worktree that disappears while its
//! shared Git registration survives, and then a repeated
//! `Failed to verify base branch: No such file or directory`.
//!
//! The boundary is simpler than repairing that state: a local orchestration
//! owner starts only from the repository's main worktree. This module is the
//! single classifier that enforces it, and every owner entrypoint runs it
//! *before* repository-lock acquisition, logging initialization, listener
//! binding, lifecycle adapters, AI subprocesses, hooks, and any workspace
//! mutation — so a refused invocation leaves nothing behind.
//!
//! Non-owner commands never reach it. `cflx client`, OpenSpec inspection and
//! validation, logs, and completion keep resolving linked worktrees exactly as
//! before.
//!
//! Nothing here is a workflow control input: it neither reads nor writes
//! durable state, and it never prunes, repairs, or redirects a worktree.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cli;
use crate::repo_lock::{self, GitDirectories, InvocationKind, LockDecision};

/// Whether a workspace may own local orchestration for its repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceEligibility {
    /// The repository's main worktree.
    MainWorktree,
    /// A registered linked worktree, which may not own local orchestration.
    LinkedWorktree,
}

/// Classify resolved Git identities without touching the filesystem.
///
/// A linked worktree's Git directory is `<common>/worktrees/<name>` and its
/// `commondir` file points back at the shared directory, so the two identities
/// differ. Every other working tree resolves both to the same path and stays
/// eligible — including a repository whose `.git` is a pointer file to a
/// separate Git directory, which is why a pointer file alone never decides this.
pub fn classify_git_directories(dirs: &GitDirectories) -> WorkspaceEligibility {
    if dirs.git_dir == dirs.common_dir {
        WorkspaceEligibility::MainWorktree
    } else {
        WorkspaceEligibility::LinkedWorktree
    }
}

/// The main worktree path carried by `git worktree list --porcelain` output.
///
/// Git lists the main worktree first, so the first `worktree <path>` record is
/// the answer. Output with no usable record yields `None` rather than a guess.
pub fn parse_main_worktree(porcelain: &str) -> Option<PathBuf> {
    porcelain
        .lines()
        .find_map(|line| line.strip_prefix("worktree "))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Render the operator-facing linked-worktree refusal.
///
/// It names what was rejected, where to start instead, and states plainly that
/// nothing was touched — the refusal is a hard stop, never a repair.
pub fn linked_worktree_message(worktree: &Path, main_worktree: Option<&Path>) -> String {
    let mut out = String::new();
    out.push_str("conflux cannot start local orchestration from a linked git worktree\n");
    out.push_str(&format!("  linked worktree: {}\n", worktree.display()));
    match main_worktree {
        Some(main) => out.push_str(&format!("  main worktree:   {}\n", main.display())),
        None => out.push_str(
            "  main worktree:   unavailable (git worktree list --porcelain could not be read)\n",
        ),
    }
    out.push_str("Start Conflux from the main worktree instead. Nothing was changed: no worktree was created, redirected, pruned, or removed.");
    out
}

/// Read the repository's main worktree path from Git.
///
/// Read-only: `git worktree list` reports the registry, it never repairs or
/// prunes it. A failure yields `None` so the caller can fall back rather than
/// turn a diagnostic into a second error.
fn read_main_worktree(workspace: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(workspace)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let parsed = parse_main_worktree(&String::from_utf8_lossy(&output.stdout))?;
    Some(std::fs::canonicalize(&parsed).unwrap_or(parsed))
}

/// Best-effort main worktree path for the diagnostic.
///
/// `git worktree list --porcelain` is the authority. When it cannot be read the
/// common directory's parent is the next best repository evidence, and when even
/// that is unavailable the message says so rather than naming a guess.
fn main_worktree_for_diagnostic(workspace: &Path, common_dir: &Path) -> Option<PathBuf> {
    read_main_worktree(workspace).or_else(|| common_dir.parent().map(Path::to_path_buf))
}

/// Refuse `workspace` when repository evidence proves it is a linked worktree.
///
/// `None` means eligible: the main worktree, a separate-Git-directory working
/// tree, or a path with no repository identity at all — the last of which the
/// Git preflight has already refused for owner entrypoints.
pub fn linked_worktree_error(workspace: &Path) -> Option<String> {
    let dirs = repo_lock::discover_git_directories(workspace)?;
    if classify_git_directories(&dirs) == WorkspaceEligibility::MainWorktree {
        return None;
    }
    let main = main_worktree_for_diagnostic(workspace, &dirs.common_dir);
    Some(linked_worktree_message(
        &dirs.worktree_root,
        main.as_deref(),
    ))
}

/// Reject an executable orchestration entrypoint that has no usable Git
/// repository.
///
/// Cumulative Git-worktree orchestration is the only execution model: there is
/// no serial fallback to degrade to, so this is a hard startup requirement.
fn git_preflight_error() -> Option<String> {
    if !cli::check_git_directory() {
        return Some(
            "conflux requires a git repository (.git directory not found): worktree \
             orchestration is the only execution model, so run cflx from inside a git \
             repository"
                .to_string(),
        );
    }
    if !cli::check_git_available() {
        return Some(
            "conflux requires the git command: install git, or make it available on PATH"
                .to_string(),
        );
    }
    None
}

/// The whole startup preflight for one parsed invocation.
///
/// Returns the operator-facing refusal, or `None` when startup may proceed.
/// Repository and Git-command availability are checked first, then main-worktree
/// eligibility, because "there is no repository" and "this repository's worktree
/// cannot own orchestration" are different answers and only the second one has a
/// main worktree to name.
///
/// Invocation kinds that do not own local orchestration always proceed: the
/// classification is shared with the repository lock precisely so that one list
/// of owner entrypoints exists rather than two that can drift.
pub fn owner_startup_error(kind: InvocationKind) -> Option<String> {
    if matches!(repo_lock::classify_invocation(kind), LockDecision::Bypass) {
        return None;
    }
    if let Some(message) = git_preflight_error() {
        return Some(message);
    }
    // An unreadable current directory has no workspace identity to classify.
    // The Git checks above already passed, so nothing is silently admitted here
    // that the repository preflight would have refused.
    let workspace = std::env::current_dir().ok()?;
    linked_worktree_error(&workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(worktree_root: &str, git_dir: &str, common_dir: &str) -> GitDirectories {
        GitDirectories {
            worktree_root: PathBuf::from(worktree_root),
            git_dir: PathBuf::from(git_dir),
            common_dir: PathBuf::from(common_dir),
        }
    }

    #[test]
    fn a_plain_main_worktree_is_eligible() {
        assert_eq!(
            classify_git_directories(&dirs("/repo", "/repo/.git", "/repo/.git")),
            WorkspaceEligibility::MainWorktree
        );
    }

    /// A `.git` pointer file to a separate Git directory is not a linked
    /// worktree: both identities still resolve to the same directory.
    #[test]
    fn a_separate_git_directory_is_eligible() {
        assert_eq!(
            classify_git_directories(&dirs("/work", "/elsewhere/repo.git", "/elsewhere/repo.git")),
            WorkspaceEligibility::MainWorktree
        );
    }

    #[test]
    fn a_linked_worktree_is_refused() {
        assert_eq!(
            classify_git_directories(&dirs("/repo/wt", "/repo/.git/worktrees/wt", "/repo/.git")),
            WorkspaceEligibility::LinkedWorktree
        );
    }

    #[test]
    fn the_first_porcelain_record_names_the_main_worktree() {
        let porcelain = "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\n\
                         worktree /repo/wt\nHEAD def\nbranch refs/heads/linked\n";
        assert_eq!(parse_main_worktree(porcelain), Some(PathBuf::from("/repo")));
    }

    #[test]
    fn porcelain_output_without_a_usable_record_names_nothing() {
        assert_eq!(parse_main_worktree(""), None);
        assert_eq!(parse_main_worktree("worktree   \n"), None);
        assert_eq!(
            parse_main_worktree("HEAD abc\nbranch refs/heads/main\n"),
            None
        );
    }

    #[test]
    fn the_refusal_names_both_worktrees_and_where_to_start() {
        let message = linked_worktree_message(Path::new("/repo/wt"), Some(Path::new("/repo")));
        assert!(message.contains("/repo/wt"), "message={message}");
        assert!(message.contains("/repo\n"), "message={message}");
        assert!(message.contains("main worktree"), "message={message}");
        assert!(
            message.contains("Start Conflux from the main worktree"),
            "message={message}"
        );
    }

    /// An unreadable registry must not be reported as a path: a guessed main
    /// worktree is worse than an admitted gap.
    #[test]
    fn the_refusal_admits_an_unknown_main_worktree() {
        let message = linked_worktree_message(Path::new("/repo/wt"), None);
        assert!(message.contains("unavailable"), "message={message}");
        assert!(
            message.contains("Start Conflux from the main worktree"),
            "message={message}"
        );
    }

    /// The owner list is the repository lock's own list, so the two boundaries
    /// cannot drift apart.
    #[test]
    fn only_owner_entrypoints_are_preflighted() {
        for kind in [
            InvocationKind::DefaultTui,
            InvocationKind::Tui,
            InvocationKind::Run,
        ] {
            assert!(matches!(
                repo_lock::classify_invocation(kind),
                LockDecision::Acquire(_)
            ));
        }
        // A non-owner invocation is admitted without any repository read at all.
        assert_eq!(owner_startup_error(InvocationKind::Other), None);
    }
}
