//! Shared evidence primitives for managed-worktree `index.lock` contention.
//!
//! Two independent recovery policies exist: the WIP snapshot retry
//! ([`crate::execution::wip_lock_retry`]) and the final Apply commit retry
//! ([`crate::execution::final_commit_lock_retry`]). They classify different
//! commands and prove completion differently, but both must agree on what
//! Git's existing-lock failure actually looks like and on which lock belongs
//! to the managed worktree. That grammar lives here once so the two policies
//! cannot drift apart on the fragile part of the decision.
//!
//! Nothing here decides whether to retry: these are pure observations plus one
//! repository query. The retry semantics stay owned by each policy.

use std::path::{Component, Path, PathBuf};

use crate::vcs::git::commands::run_git;
use crate::vcs::VcsResult;

/// Extract the `index.lock` path from an existing-lock Git failure.
///
/// Git reports `fatal: Unable to create '<path>': File exists.` for this case;
/// any other lock prose (permission denied, configuration failure) and any
/// other lock file is a different failure and must stay terminal.
pub(crate) fn parse_existing_index_lock(stderr: &str) -> Option<PathBuf> {
    for line in stderr.lines() {
        let Some((_, rest)) = line.split_once("Unable to create '") else {
            continue;
        };
        let Some((path, tail)) = rest.split_once('\'') else {
            continue;
        };
        if !tail.trim_start().starts_with(": File exists") {
            continue;
        }
        let path = PathBuf::from(path);
        if path.file_name().is_some_and(|name| name == "index.lock") {
            return Some(path);
        }
    }
    None
}

/// Normalize a path without touching the filesystem, so that classification
/// stays a pure decision.
pub(crate) fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Whether `reported` is one of the managed worktree's own lock paths.
pub(crate) fn is_managed_lock_path(candidates: &[PathBuf], reported: &Path) -> bool {
    let reported = lexically_normalize(reported);
    candidates
        .iter()
        .any(|candidate| lexically_normalize(candidate) == reported)
}

/// Record a git directory and its canonical form as the same lock identity.
fn push_git_dir_variants(git_dirs: &mut Vec<PathBuf>, git_dir: PathBuf) {
    let canonical = std::fs::canonicalize(&git_dir).ok();
    for candidate in [Some(git_dir), canonical].into_iter().flatten() {
        if !git_dirs.contains(&candidate) {
            git_dirs.push(candidate);
        }
    }
}

/// Resolve the `index.lock` paths that identify the managed worktree.
///
/// More than one path can name the same lock because a git directory may be
/// reported through a symlinked prefix (`/var` vs `/private/var` on macOS).
pub(crate) async fn managed_worktree_lock_paths(workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
    let mut git_dirs = Vec::new();
    push_git_dir_variants(
        &mut git_dirs,
        PathBuf::from(run_git(&["rev-parse", "--absolute-git-dir"], workspace_path).await?),
    );

    // A failing command names the git directory the way Git resolved the
    // current directory, which can keep a symlinked prefix that
    // `--absolute-git-dir` already resolved (`/tmp` vs `/private/tmp` on
    // macOS). Rebuilding it from the workspace path covers that form too.
    if let Ok(git_dir) = run_git(&["rev-parse", "--git-dir"], workspace_path).await {
        let git_dir = Path::new(&git_dir);
        let git_dir = if git_dir.is_absolute() {
            git_dir.to_path_buf()
        } else {
            workspace_path.join(git_dir)
        };
        push_git_dir_variants(&mut git_dirs, git_dir);
    }

    Ok(git_dirs
        .into_iter()
        .map(|git_dir| git_dir.join("index.lock"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_stderr(lock_path: &str) -> String {
        format!("fatal: Unable to create '{}': File exists.\n", lock_path)
    }

    #[test]
    fn existing_index_lock_is_parsed_from_git_prose() {
        assert_eq!(
            parse_existing_index_lock(&lock_stderr("/repo/.git/index.lock")),
            Some(PathBuf::from("/repo/.git/index.lock"))
        );
    }

    #[test]
    fn other_lock_prose_and_other_lock_files_are_not_existing_index_locks() {
        assert!(parse_existing_index_lock(
            "fatal: Unable to create '/repo/.git/index.lock': Permission denied\n"
        )
        .is_none());
        assert!(
            parse_existing_index_lock(&lock_stderr("/repo/.git/refs/heads/main.lock")).is_none()
        );
        assert!(parse_existing_index_lock("fatal: not a git repository\n").is_none());
    }

    #[test]
    fn managed_lock_identity_ignores_redundant_path_components() {
        let candidates = vec![PathBuf::from("/repo/.git/worktrees/demo/index.lock")];

        assert!(is_managed_lock_path(
            &candidates,
            Path::new("/repo/.git/worktrees/demo/./index.lock")
        ));
        assert!(!is_managed_lock_path(
            &candidates,
            Path::new("/other/.git/worktrees/demo/index.lock")
        ));
    }
}
