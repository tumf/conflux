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
    MergeCommandResult, PortResult, PushCommandResult, RecoveryCommit, UpstreamGit,
    UpstreamPortError,
};
use super::spine::{CommitTreeEvidence, SpineCommit};

/// Record separator used to frame `git log` output deterministically.
const RECORD_SEP: char = '\u{1e}';
/// Unit separator used between fields of one record.
const UNIT_SEP: char = '\u{1f}';

/// One framed `git log --first-parent` record, before any tree read.
///
/// Both first-parent observations share this parse; only the evidence-bearing
/// one goes on to read each commit's tree.
struct FirstParentRecord {
    sha: String,
    message: String,
    parents: Vec<String>,
}

/// Native Git operations rooted at the cumulative base checkout.
pub struct GitUpstreamOps {
    repo_root: PathBuf,
    /// Non-authoritative record of every Git invocation, for in-crate tests that
    /// assert command shape (for example that recovery discovery issues no
    /// `ls-tree`). It is never read by production code and does not exist
    /// outside `cfg(test)`.
    #[cfg(test)]
    observed_commands: std::sync::Mutex<Vec<Vec<String>>>,
}

impl GitUpstreamOps {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            #[cfg(test)]
            observed_commands: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Every Git invocation made through this instance, in order.
    #[cfg(test)]
    fn observed_commands(&self) -> Vec<Vec<String>> {
        self.observed_commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    async fn run(&self, operation: &str, args: &[&str]) -> PortResult<(bool, String, String)> {
        #[cfg(test)]
        self.observed_commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(args.iter().map(|a| a.to_string()).collect());

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

    /// One bounded `git log --first-parent` invocation, parsed into records.
    ///
    /// Exactly one Git subprocess runs here regardless of how many commits the
    /// walk returns; ordering is normalized to oldest first. No tree is read.
    async fn first_parent_records(
        &self,
        from_exclusive: Option<&str>,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<FirstParentRecord>> {
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
        Ok(parse_first_parent_records(&stdout))
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

/// Parse framed `git log --first-parent` output into oldest-first records.
///
/// `git log` emits newest first; both consumers read oldest first. A record that
/// carries no object name is dropped rather than becoming a commit with an empty
/// SHA, so trailing framing and blank output produce an empty walk instead of
/// phantom commits.
fn parse_first_parent_records(stdout: &str) -> Vec<FirstParentRecord> {
    let mut records = Vec::new();
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

        records.push(FirstParentRecord {
            sha: sha.to_string(),
            message,
            parents,
        });
    }

    records.reverse();
    records
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

    async fn commit_empty(&self, message: &str) -> PortResult<String> {
        self.run_checked(
            "git commit",
            &["commit", "--allow-empty", "--no-verify", "-m", message],
        )
        .await?;
        self.head_sha().await
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

    async fn first_parent_recovery_metadata(
        &self,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<RecoveryCommit>> {
        // Deliberately no `tree_evidence` call: recovery classification never
        // reads archive or active-change evidence, so the whole walk stays at
        // one Git subprocess no matter how many commits it covers.
        Ok(self
            .first_parent_records(None, to, limit)
            .await?
            .into_iter()
            .map(|record| RecoveryCommit {
                sha: record.sha,
                message: record.message,
                parents: record.parents,
            })
            .collect())
    }

    async fn first_parent_commits(
        &self,
        from_exclusive: Option<&str>,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<SpineCommit>> {
        let records = self.first_parent_records(from_exclusive, to, limit).await?;

        let mut commits = Vec::with_capacity(records.len());
        for record in records {
            // Spine classification is evidence-bearing: every commit carries its
            // own tree evidence, and a failed read is never substituted with a
            // default.
            let tree_evidence = self.tree_evidence(&record.sha).await?;
            commits.push(SpineCommit {
                sha: record.sha,
                message: record.message,
                parents: record.parents,
                tree_evidence,
            });
        }
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
    use std::fs;
    use std::process::Command as SyncCommand;
    use tempfile::TempDir;

    #[test]
    fn upstream_integration_strips_dated_archive_prefix() {
        assert_eq!(strip_date_prefix("2026-07-30-my-change"), Some("my-change"));
        assert_eq!(strip_date_prefix("my-change"), None);
        assert_eq!(strip_date_prefix("2026-07-30"), None);
    }

    /// Build one framed `git log` record the way `--format` emits it.
    fn record(sha: &str, parents: &str, message: &str) -> String {
        format!(
            "{}{}{}{}{}{}",
            sha, UNIT_SEP, parents, UNIT_SEP, message, RECORD_SEP
        )
    }

    #[test]
    fn upstream_integration_recovery_parse_reverses_and_keeps_fields() {
        let stdout = format!(
            "{}{}",
            record("bbb", "aaa ccc", "Merge change: x\n\nbody\n"),
            record("aaa", "", "root\n"),
        );
        let parsed = parse_first_parent_records(&stdout);

        // `git log` is newest first; the parse hands back oldest first.
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].sha, "aaa");
        assert!(parsed[0].parents.is_empty());
        assert_eq!(parsed[1].sha, "bbb");
        assert_eq!(
            parsed[1].parents,
            vec!["aaa".to_string(), "ccc".to_string()]
        );
        assert!(parsed[1].message.contains("Merge change: x"));
        assert!(
            parsed[1].message.contains("body"),
            "the raw message body carries the trailers recovery classifies on"
        );
    }

    #[test]
    fn upstream_integration_recovery_parse_drops_malformed_records() {
        // Empty output, framing-only output, and a record with no object name
        // must all yield no commits rather than a phantom empty-SHA commit.
        assert!(parse_first_parent_records("").is_empty());
        assert!(parse_first_parent_records(&format!("{}\n{}", RECORD_SEP, RECORD_SEP)).is_empty());
        assert!(parse_first_parent_records(&record("", "aaa", "orphan\n")).is_empty());

        // A truncated record keeps the fields it does have.
        let truncated = format!("ddd{}", RECORD_SEP);
        let parsed = parse_first_parent_records(&truncated);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sha, "ddd");
        assert!(parsed[0].parents.is_empty());
        assert_eq!(parsed[0].message, "");
    }

    // ── Native fixture coverage ────────────────────────────────────────────
    //
    // These exercise real `git` subprocesses against a throwaway repository, so
    // they are integration-scoped rather than unit-scoped.

    fn git(root: &Path, args: &[&str]) -> String {
        let output = SyncCommand::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "cflx")
            .env("GIT_AUTHOR_EMAIL", "cflx@example.com")
            .env("GIT_COMMITTER_NAME", "cflx")
            .env("GIT_COMMITTER_EMAIL", "cflx@example.com")
            .output()
            .unwrap_or_else(|e| panic!("git {:?}: {}", args, e));
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// `root` -> `Merge change: my-change` (two parents, archive evidence in its
    /// tree) -> `tip`, on the first-parent line.
    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        git(root, &["init", "-q", "-b", "main"]);
        write(root, "README.md", "root\n");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "root"]);

        git(root, &["checkout", "-q", "-b", "work"]);
        write(
            root,
            "openspec/changes/archive/2026-01-01-my-change/proposal.md",
            "archived\n",
        );
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "archive my-change"]);

        git(root, &["checkout", "-q", "main"]);
        git(
            root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "--no-edit",
                "-m",
                "Merge change: my-change\n\nCflx-Trailer: body line\n",
                "work",
            ],
        );

        write(root, "README.md", "tip\n");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "tip"]);

        dir
    }

    fn issued(ops: &GitUpstreamOps, subcommand: &str) -> usize {
        ops.observed_commands()
            .iter()
            .filter(|args| args.first().map(String::as_str) == Some(subcommand))
            .count()
    }

    #[tokio::test]
    async fn upstream_integration_recovery_metadata_reads_no_commit_trees() {
        let dir = fixture();
        let ops = GitUpstreamOps::new(dir.path());
        let head = ops.head_sha().await.unwrap();
        let before = ops.observed_commands().len();

        let commits = ops
            .first_parent_recovery_metadata(&head, None)
            .await
            .unwrap();

        assert_eq!(commits.len(), 3, "{:?}", commits);
        assert_eq!(commits[0].message.trim(), "root");
        assert!(commits[1].message.contains("Merge change: my-change"));
        assert!(
            commits[1].message.contains("Cflx-Trailer: body line"),
            "the raw body must survive: recovery identity lives in trailers"
        );
        assert_eq!(
            commits[1].parents.len(),
            2,
            "merge-parent binding needs every parent in Git order"
        );
        assert_eq!(commits[1].parents[0], commits[0].sha);
        assert_eq!(commits[2].message.trim(), "tip");
        assert_eq!(commits[2].sha, head);

        // The whole bounded walk is one Git subprocess and reads no tree.
        let walk = &ops.observed_commands()[before..];
        assert_eq!(
            walk.len(),
            1,
            "recovery discovery must issue exactly one Git command: {:?}",
            walk
        );
        assert_eq!(walk[0][0], "log");
        assert!(walk[0].iter().any(|arg| arg == "--first-parent"));
        assert_eq!(
            issued(&ops, "ls-tree"),
            0,
            "recovery discovery must not read commit trees: {:?}",
            ops.observed_commands()
        );
    }

    #[tokio::test]
    async fn upstream_integration_recovery_metadata_honors_the_bound() {
        let dir = fixture();
        let ops = GitUpstreamOps::new(dir.path());
        let head = ops.head_sha().await.unwrap();

        let all = ops
            .first_parent_recovery_metadata(&head, None)
            .await
            .unwrap();
        let bounded = ops
            .first_parent_recovery_metadata(&head, Some(2))
            .await
            .unwrap();

        // The bound keeps the newest commits and still reports them oldest first,
        // so the walk drops history beyond the limit rather than truncating the
        // recent end an operator is recovering.
        assert_eq!(bounded.len(), 2);
        assert_eq!(bounded[0].sha, all[1].sha);
        assert_eq!(bounded[1].sha, all[2].sha);
        assert!(
            ops.observed_commands()
                .iter()
                .any(|args| args.iter().any(|arg| arg == "-n2")),
            "the limit must be pushed down to `git log`: {:?}",
            ops.observed_commands()
        );
        assert_eq!(issued(&ops, "ls-tree"), 0);
    }

    #[tokio::test]
    async fn upstream_integration_spine_walk_still_reads_commit_tree_evidence() {
        let dir = fixture();
        let ops = GitUpstreamOps::new(dir.path());
        let head = ops.head_sha().await.unwrap();

        let commits = ops.first_parent_commits(None, &head, None).await.unwrap();

        assert_eq!(commits.len(), 3);
        let merge = &commits[1];
        assert!(merge.message.contains("Merge change: my-change"));
        assert!(
            merge
                .tree_evidence
                .archived_change_ids
                .contains("my-change"),
            "spine validation still needs archive evidence from the commit tree: {:?}",
            merge.tree_evidence
        );
        assert!(merge
            .tree_evidence
            .archived_change_ids
            .contains("2026-01-01-my-change"));
        assert!(
            !merge.tree_evidence.active_change_ids.contains("my-change"),
            "the archived change is no longer an active directory"
        );
        assert_eq!(
            issued(&ops, "ls-tree"),
            commits.len() * 2,
            "the evidence-bearing walk reads archive and active trees per commit"
        );
    }
}
