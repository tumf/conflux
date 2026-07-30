//! Real Git implementation of [`UpstreamGit`].
//!
//! Every operation here is a native Conflux-owned Git invocation. None of them
//! runs through `AgentRunner`, `AiCommandRunner`, or an AI command template.
//! Behavior is covered by the heavy E2E suite against real repositories,
//! worktrees, and local bare remotes.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use super::classify::MergeRepositoryState;
use super::ports::{
    MergeCommandResult, PortResult, PushCommandResult, UpstreamGit, UpstreamPortError,
};
use super::spine::{CommitTreeEvidence, SpineCommit};

/// Record separator used to frame `git log` output deterministically.
const RECORD_SEP: char = '\u{1e}';
/// Unit separator used between fields of one record.
const UNIT_SEP: char = '\u{1f}';

/// Native Git operations rooted at the cumulative base checkout.
pub struct GitUpstreamOps {
    repo_root: PathBuf,
}

impl GitUpstreamOps {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    async fn run(&self, operation: &str, args: &[&str]) -> PortResult<(bool, String, String)> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| UpstreamPortError::new(operation, e.to_string()))?;

        Ok((
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }

    async fn run_checked(&self, operation: &str, args: &[&str]) -> PortResult<String> {
        let (success, stdout, stderr) = self.run(operation, args).await?;
        if !success {
            return Err(UpstreamPortError::new(operation, stderr.trim().to_string()));
        }
        Ok(stdout)
    }

    /// Resolve a revision, returning `None` when it does not exist.
    async fn rev_parse_optional(&self, reference: &str) -> PortResult<Option<String>> {
        let (success, stdout, _) = self
            .run("git rev-parse", &["rev-parse", "--verify", "-q", reference])
            .await?;
        let value = stdout.trim().to_string();
        Ok((success && !value.is_empty()).then_some(value))
    }

    /// Archive/active change evidence read from one commit's own tree.
    async fn tree_evidence(&self, commit: &str) -> PortResult<CommitTreeEvidence> {
        let mut archived = Vec::new();
        let mut active = Vec::new();

        let archive_spec = format!("{}:openspec/changes/archive", commit);
        let (ok, stdout, _) = self
            .run("git ls-tree", &["ls-tree", "--name-only", &archive_spec])
            .await?;
        if ok {
            for name in stdout.lines().map(str::trim).filter(|s| !s.is_empty()) {
                let name = name.trim_end_matches('/');
                archived.push(name.to_string());
                // Dated archive entries are `YYYY-MM-DD-<change-id>`.
                if let Some(rest) = strip_date_prefix(name) {
                    archived.push(rest.to_string());
                }
            }
        }

        let changes_spec = format!("{}:openspec/changes", commit);
        let (ok, stdout, _) = self
            .run(
                "git ls-tree",
                &["ls-tree", "--name-only", "-d", &changes_spec],
            )
            .await?;
        if ok {
            for name in stdout.lines().map(str::trim).filter(|s| !s.is_empty()) {
                let name = name.trim_end_matches('/');
                if name != "archive" {
                    active.push(name.to_string());
                }
            }
        }

        Ok(CommitTreeEvidence::new(archived, active))
    }
}

/// Strip a leading `YYYY-MM-DD-` prefix from an archive directory name.
fn strip_date_prefix(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    if bytes.len() < 11 {
        return None;
    }
    let dated = bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'-';
    dated.then(|| &name[11..])
}

#[async_trait]
impl UpstreamGit for GitUpstreamOps {
    async fn remote_configured(&self, remote: &str) -> PortResult<bool> {
        let stdout = self.run_checked("git remote", &["remote"]).await?;
        Ok(stdout.lines().any(|line| line.trim() == remote))
    }

    async fn current_branch(&self) -> PortResult<Option<String>> {
        let (success, stdout, _) = self
            .run(
                "git symbolic-ref",
                &["symbolic-ref", "--short", "-q", "HEAD"],
            )
            .await?;
        let value = stdout.trim().to_string();
        Ok((success && !value.is_empty()).then_some(value))
    }

    async fn fetch(&self, remote: &str, _branch: &str) -> PortResult<()> {
        // Fetch the whole remote so remote-tracking refs are refreshed even when
        // the same-name branch does not exist; a missing branch is then a
        // resolution result rather than a command failure.
        self.run_checked("git fetch", &["fetch", "--prune", remote])
            .await?;
        Ok(())
    }

    async fn fetched_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>> {
        self.rev_parse_optional(&format!("refs/remotes/{}/{}", remote, branch))
            .await
    }

    async fn head_sha(&self) -> PortResult<String> {
        Ok(self
            .run_checked("git rev-parse", &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string())
    }

    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> PortResult<bool> {
        let (success, _, _) = self
            .run(
                "git merge-base",
                &["merge-base", "--is-ancestor", ancestor, descendant],
            )
            .await?;
        Ok(success)
    }

    async fn merge_base(&self, a: &str, b: &str) -> PortResult<String> {
        Ok(self
            .run_checked("git merge-base", &["merge-base", a, b])
            .await?
            .trim()
            .to_string())
    }

    async fn merge_no_ff(&self, sha: &str, message: &str) -> PortResult<MergeCommandResult> {
        let (exit_success, _, _) = self
            .run(
                "git merge",
                &["merge", "--no-ff", "--no-edit", "-m", message, sha],
            )
            .await?;
        // Repository state, not the command's prose, classifies the outcome.
        let state = self.merge_repository_state().await?;
        Ok(MergeCommandResult {
            exit_success,
            state,
        })
    }

    async fn merge_repository_state(&self) -> PortResult<MergeRepositoryState> {
        let (merge_head_present, _, _) = self
            .run(
                "git rev-parse",
                &["rev-parse", "-q", "--verify", "MERGE_HEAD"],
            )
            .await?;
        let unmerged = self
            .run_checked(
                "git diff",
                &["diff", "--name-only", "--diff-filter=U", "--no-color"],
            )
            .await
            .unwrap_or_default();
        Ok(MergeRepositoryState {
            merge_head_present,
            has_unmerged_entries: unmerged.lines().any(|line| !line.trim().is_empty()),
        })
    }

    async fn is_working_tree_clean(&self) -> PortResult<bool> {
        let stdout = self
            .run_checked("git status", &["status", "--porcelain"])
            .await?;
        Ok(stdout.trim().is_empty())
    }

    async fn status_porcelain_v2(&self) -> PortResult<String> {
        self.run_checked("git status", &["status", "--porcelain=v2"])
            .await
    }

    async fn commit_message(&self, sha: &str) -> PortResult<String> {
        self.run_checked("git log", &["log", "-1", "--format=%B", sha])
            .await
    }

    async fn commit_parents(&self, sha: &str) -> PortResult<Vec<String>> {
        let stdout = self
            .run_checked("git log", &["log", "-1", "--format=%P", sha])
            .await?;
        Ok(stdout
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>())
    }

    async fn first_parent_commits(
        &self,
        from_exclusive: Option<&str>,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<SpineCommit>> {
        let range = match from_exclusive {
            Some(from) => format!("{}..{}", from, to),
            None => to.to_string(),
        };
        let format = format!("--format=%H{}%P{}%B{}", UNIT_SEP, UNIT_SEP, RECORD_SEP);
        let limit_arg = limit.map(|n| format!("-n{}", n));

        let mut args: Vec<&str> = vec!["log", "--first-parent", &format];
        if let Some(limit_arg) = limit_arg.as_deref() {
            args.push(limit_arg);
        }
        args.push(&range);

        let stdout = self.run_checked("git log", &args).await?;

        let mut commits = Vec::new();
        for record in stdout.split(RECORD_SEP) {
            let record = record.trim_start_matches('\n');
            if record.trim().is_empty() {
                continue;
            }
            let mut fields = record.split(UNIT_SEP);
            let Some(sha) = fields.next().map(str::trim) else {
                continue;
            };
            if sha.is_empty() {
                continue;
            }
            let parents: Vec<String> = fields
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_string)
                .collect();
            let message = fields.next().unwrap_or_default().to_string();

            commits.push(SpineCommit {
                sha: sha.to_string(),
                message,
                parents,
                tree_evidence: self.tree_evidence(sha).await?,
            });
        }

        // `git log` emits newest first; the spine is validated oldest first.
        commits.reverse();
        Ok(commits)
    }

    async fn local_ref_sha(&self, reference: &str) -> PortResult<Option<String>> {
        self.rev_parse_optional(reference).await
    }

    async fn push_porcelain(&self, remote: &str, branch: &str) -> PortResult<PushCommandResult> {
        let refspec = format!("HEAD:refs/heads/{}", branch);
        let (exit_success, stdout, _) = self
            .run("git push", &["push", "--porcelain", remote, &refspec])
            .await?;
        Ok(PushCommandResult {
            exit_success,
            porcelain_stdout: stdout,
        })
    }

    async fn ls_remote_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>> {
        let refname = format!("refs/heads/{}", branch);
        let stdout = self
            .run_checked("git ls-remote", &["ls-remote", remote, &refname])
            .await?;
        Ok(stdout
            .lines()
            .find_map(|line| line.split_whitespace().next().map(str::to_string)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_integration_strips_dated_archive_prefix() {
        assert_eq!(strip_date_prefix("2026-07-30-my-change"), Some("my-change"));
        assert_eq!(strip_date_prefix("my-change"), None);
        assert_eq!(strip_date_prefix("2026-07-30"), None);
    }
}
