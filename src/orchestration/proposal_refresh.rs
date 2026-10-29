//! Candidate proposal refresh: carry one corrected `proposal.md` into an
//! already-applied candidate worktree as one hook-retaining commit.
//!
//! This module owns the whole Git side of the shared `refresh_proposal` operator
//! command — the pure eligibility table, the read-only preview, and the apply
//! steps — so the operator service, the `/api/v2` preview resource, and the TUI
//! all read one implementation rather than three.
//!
//! The command is deliberately narrow:
//!
//! - the only content read from the source commit is the blob at
//!   `openspec/changes/<change_id>/proposal.md`, resolved from an *explicit*
//!   `source_ref` — there is no default branch;
//! - the only write is that one file, staged by literal pathspec and committed
//!   with repository hooks enabled on top of the candidate's current `HEAD`;
//! - nothing is ever reset, amended, cleaned, or retried. A failure after the
//!   write retains whatever state Git actually holds, and the report's `effects`
//!   are re-read from Git rather than assumed from the step that was attempted.
//!
//! Stale Acceptance evidence needs no mechanism here: the manifest's
//! `change_input_digest` reads the candidate `proposal.md`, and evidence records
//! bind the candidate commit and tree, so the refresh commit invalidates them by
//! content identity.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::orchestration::operator_command::{is_active_status, is_final_status};
use crate::orchestration::state::BlockerKind;

/// Deadline for one read-only Git invocation.
///
/// Generous on purpose: it is a hang safeguard, not a performance assertion. The
/// commit step itself runs repository hooks and is never bounded here, exactly
/// like the final Apply commit.
const GIT_READ_DEADLINE: Duration = Duration::from_secs(120);

/// Bytes of hook output retained in a settled result.
pub const HOOK_OUTPUT_TAIL_BYTES: usize = 4096;

/// Bytes of unified patch a preview carries before it is truncated.
pub const PREVIEW_PATCH_LIMIT_BYTES: usize = 64 * 1024;

/// The one repository-relative path this command reads and writes.
pub fn proposal_path(change_id: &str) -> String {
    format!("openspec/changes/{change_id}/proposal.md")
}

// ============================================================================
// Reasons
// ============================================================================

/// Every stable reason a refresh can be refused for.
///
/// One vocabulary for both halves of the decision: the lifecycle reasons are
/// what `actions.refresh_proposal` publishes and what admission refuses with
/// before any Git read, and the command-time reasons are what the transaction
/// refuses with once it has looked at the repository. [`Self::is_lifecycle`] is
/// the split a client maps onto `target_ineligible` versus `refresh_refused`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefreshReason {
    // ── Lifecycle ─────────────────────────────────────────────────────────
    /// The owner does not track a change with this ID.
    UnknownTarget,
    /// The change has no registered managed worktree to refresh.
    NoCandidateWorktree,
    /// Archived, merged, pushed, or rejected.
    TerminalTarget,
    /// The change is executing or owns a live managed process.
    TargetActive,
    /// The change is admitted (`queued`, or `blocked` on a dependency).
    TargetAdmitted,
    /// The change is parked on a base-lane merge wait.
    MergeWait,
    /// The change is waiting for manual resolution.
    ResolveWait,
    /// The change is waiting on a rejection decision.
    RejectWait,

    // ── Command time ──────────────────────────────────────────────────────
    /// `source_ref` does not resolve to a commit in the project repository.
    SourceRefUnresolvable,
    /// The source commit has no `proposal.md` for this change.
    SourceProposalMissing,
    /// The candidate `HEAD` has no `proposal.md` for this change.
    CandidateProposalMissing,
    /// A tree entry or on-disk path component is a symlink or not a regular file.
    UnsafeProposalPath,
    /// `expected_source` or `expected_head` no longer matches.
    RefreshExpectationMismatch,
    /// The candidate has staged, unstaged, or untracked entries.
    CandidateDirty,
    /// A merge, cherry-pick, revert, rebase, or bisect is in progress.
    CandidateOperationInProgress,
    /// A repository Git read failed, so nothing could be decided safely.
    GitReadFailed,
    /// The source bytes could not be written to the candidate path.
    WriteFailed,
    /// A commit hook exited non-zero.
    HookRejected,
    /// Staging or committing failed for a reason other than a hook.
    CommitFailed,
    /// The commit is not exactly one commit on `expected_head` changing only the path.
    RefreshCommitUnexpectedTree,
    /// The commit landed but the worktree is not clean afterwards.
    HookLeftWorkspaceDirty,
}

impl RefreshReason {
    /// Lifecycle reasons, in documentation order.
    pub const LIFECYCLE: [RefreshReason; 8] = [
        Self::UnknownTarget,
        Self::NoCandidateWorktree,
        Self::TerminalTarget,
        Self::TargetActive,
        Self::TargetAdmitted,
        Self::MergeWait,
        Self::ResolveWait,
        Self::RejectWait,
    ];

    /// Command-time reasons, in documentation order.
    pub const COMMAND_TIME: [RefreshReason; 13] = [
        Self::SourceRefUnresolvable,
        Self::SourceProposalMissing,
        Self::CandidateProposalMissing,
        Self::UnsafeProposalPath,
        Self::RefreshExpectationMismatch,
        Self::CandidateDirty,
        Self::CandidateOperationInProgress,
        Self::GitReadFailed,
        Self::WriteFailed,
        Self::HookRejected,
        Self::CommitFailed,
        Self::RefreshCommitUnexpectedTree,
        Self::HookLeftWorkspaceDirty,
    ];

    /// Stable machine-readable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTarget => "unknown_target",
            Self::NoCandidateWorktree => "no_candidate_worktree",
            Self::TerminalTarget => "terminal_target",
            Self::TargetActive => "target_active",
            Self::TargetAdmitted => "target_admitted",
            Self::MergeWait => "merge_wait",
            Self::ResolveWait => "resolve_wait",
            Self::RejectWait => "reject_wait",
            Self::SourceRefUnresolvable => "source_ref_unresolvable",
            Self::SourceProposalMissing => "source_proposal_missing",
            Self::CandidateProposalMissing => "candidate_proposal_missing",
            Self::UnsafeProposalPath => "unsafe_proposal_path",
            Self::RefreshExpectationMismatch => "refresh_expectation_mismatch",
            Self::CandidateDirty => "candidate_dirty",
            Self::CandidateOperationInProgress => "candidate_operation_in_progress",
            Self::GitReadFailed => "git_read_failed",
            Self::WriteFailed => "write_failed",
            Self::HookRejected => "hook_rejected",
            Self::CommitFailed => "commit_failed",
            Self::RefreshCommitUnexpectedTree => "refresh_commit_unexpected_tree",
            Self::HookLeftWorkspaceDirty => "hook_left_workspace_dirty",
        }
    }

    /// Parse a stable token.
    #[allow(dead_code)] // Read by the vocabulary contract tests.
    pub fn parse(token: &str) -> Option<Self> {
        Self::LIFECYCLE
            .iter()
            .chain(Self::COMMAND_TIME.iter())
            .copied()
            .find(|reason| reason.as_str() == token)
    }

    /// True for a reason decided from owner facts alone, before any Git read.
    #[allow(dead_code)] // Read by the vocabulary contract tests.
    pub fn is_lifecycle(self) -> bool {
        Self::LIFECYCLE.contains(&self)
    }

    /// Short operator-facing explanation.
    pub fn describe(self) -> &'static str {
        match self {
            Self::UnknownTarget => "not tracked by this owner",
            Self::NoCandidateWorktree => "has no managed candidate worktree",
            Self::TerminalTarget => "is archived, merged, pushed, or rejected",
            Self::TargetActive => "is executing or owns a live managed process",
            Self::TargetAdmitted => "is admitted for execution",
            Self::MergeWait => "is waiting on a base merge",
            Self::ResolveWait => "is waiting for manual resolution",
            Self::RejectWait => "is waiting on a rejection decision",
            Self::SourceRefUnresolvable => "the source ref does not resolve to a commit",
            Self::SourceProposalMissing => "the source commit has no proposal.md for this change",
            Self::CandidateProposalMissing => {
                "the candidate HEAD has no proposal.md for this change"
            }
            Self::UnsafeProposalPath => "the proposal path is not a regular non-symlink file",
            Self::RefreshExpectationMismatch => "the source commit or candidate HEAD moved",
            Self::CandidateDirty => "the candidate worktree is not clean",
            Self::CandidateOperationInProgress => "a Git operation is in progress in the candidate",
            Self::GitReadFailed => "a repository read failed",
            Self::WriteFailed => "the proposal file could not be written",
            Self::HookRejected => "a commit hook rejected the commit",
            Self::CommitFailed => "Git failed to stage or commit",
            Self::RefreshCommitUnexpectedTree => "the commit changed more than the proposal path",
            Self::HookLeftWorkspaceDirty => "the worktree is dirty after the commit",
        }
    }
}

// ============================================================================
// Eligibility
// ============================================================================

/// What a refresh request may do to one change, decided from owner facts alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshAdmission {
    /// The target may be refreshed; command-time preconditions still apply.
    Allowed,
    /// Refused with a stable lifecycle reason, before any Git read.
    Refused(RefreshReason),
}

impl RefreshAdmission {
    /// True when the target is eligible.
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// The refusal reason, if refused.
    pub fn exclusion(self) -> Option<RefreshReason> {
        match self {
            Self::Allowed => None,
            Self::Refused(reason) => Some(reason),
        }
    }
}

/// The whole refresh eligibility rule as a pure function of owner facts.
///
/// Read by the API's published `actions.refresh_proposal`, the shared
/// transaction's admission, the TUI hint, and every test, so none of them can
/// re-derive a different table. Independent of process mode: the command
/// touches no scheduler state.
pub fn classify_refresh_proposal(
    display_status: &str,
    blocker_kind: BlockerKind,
    tracked: bool,
    has_worktree: bool,
    owns_managed_process: bool,
) -> RefreshAdmission {
    use RefreshAdmission::{Allowed, Refused};

    if !tracked {
        return Refused(RefreshReason::UnknownTarget);
    }
    if is_final_status(display_status) {
        return Refused(RefreshReason::TerminalTarget);
    }
    if is_active_status(display_status) || owns_managed_process {
        return Refused(RefreshReason::TargetActive);
    }
    let eligible_status = match display_status {
        "merge wait" => return Refused(RefreshReason::MergeWait),
        "resolve pending" => return Refused(RefreshReason::ResolveWait),
        "reject pending" => return Refused(RefreshReason::RejectWait),
        "queued" => return Refused(RefreshReason::TargetAdmitted),
        // A dependency wait — or a hold with no structured blocker — is work the
        // owner clears by itself, so it is still admitted.
        "blocked" => blocker_kind == BlockerKind::External,
        "error" | "stopped" | "stalled" | "not queued" => true,
        // Unknown vocabulary is refused conservatively rather than written to.
        _ => false,
    };
    if !eligible_status {
        return Refused(RefreshReason::TargetAdmitted);
    }
    if !has_worktree {
        return Refused(RefreshReason::NoCandidateWorktree);
    }
    Allowed
}

// ============================================================================
// Reports
// ============================================================================

/// Successful settlement of an apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// One scoped commit landed and verified.
    Refreshed,
    /// The blobs were identical; nothing was written.
    Unchanged,
}

impl RefreshOutcome {
    /// Stable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Refreshed => "refreshed",
            Self::Unchanged => "unchanged",
        }
    }
}

/// What the command actually did to the candidate, read back from Git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshEffects {
    /// The command wrote its target file.
    pub file_written: bool,
    /// The command's staging step completed (not whether changes remain staged).
    pub staged: bool,
    /// `HEAD` was observed to have moved off `expected_head`.
    pub committed: bool,
    /// The candidate worktree was observed clean when the command finished.
    pub clean_after: bool,
}

/// The settled result of one apply, successful or refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshReport {
    /// Target change.
    pub change_id: String,
    /// `Some` on success.
    pub outcome: Option<RefreshOutcome>,
    /// `Some` on refusal.
    pub refusal: Option<RefreshReason>,
    /// The ref the caller named.
    pub source_ref: String,
    /// Resolved source commit, when it resolved.
    pub source_commit: Option<String>,
    /// Source blob identity, when read.
    pub source_blob: Option<String>,
    /// Candidate `HEAD` before the command, when read.
    pub candidate_head_before: Option<String>,
    /// Candidate `HEAD` after the command, when read.
    pub candidate_head_after: Option<String>,
    /// Candidate blob identity before the command, when read.
    pub candidate_blob_before: Option<String>,
    /// The repository-relative path.
    pub path: String,
    /// Observed effects.
    pub effects: RefreshEffects,
    /// Bounded tail of hook / commit output, when a commit ran.
    pub hook_output_tail: Option<String>,
    /// Bounded diagnostic detail (dirty entries, current values, Git stderr).
    pub detail: Option<String>,
}

impl RefreshReport {
    fn new(change_id: &str, source_ref: &str) -> Self {
        Self {
            change_id: change_id.to_string(),
            outcome: None,
            refusal: None,
            source_ref: source_ref.to_string(),
            source_commit: None,
            source_blob: None,
            candidate_head_before: None,
            candidate_head_after: None,
            candidate_blob_before: None,
            path: proposal_path(change_id),
            effects: RefreshEffects::default(),
            hook_output_tail: None,
            detail: None,
        }
    }

    /// One operator-facing sentence.
    pub fn describe(&self) -> String {
        let head = |value: &Option<String>| {
            value
                .as_deref()
                .map(|oid| oid[..12.min(oid.len())].to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        match (self.outcome, self.refusal) {
            (Some(RefreshOutcome::Refreshed), _) => format!(
                "refreshed {} in '{}' from {} ({}): HEAD {} -> {}",
                self.path,
                self.change_id,
                self.source_ref,
                head(&self.source_commit),
                head(&self.candidate_head_before),
                head(&self.candidate_head_after),
            ),
            (Some(RefreshOutcome::Unchanged), _) => format!(
                "{} in '{}' already matches {} ({}); nothing was written",
                self.path,
                self.change_id,
                self.source_ref,
                head(&self.source_commit),
            ),
            (None, Some(reason)) => {
                let mut text = format!(
                    "refresh of '{}' refused as `{}`: {}; effects: file_written={}, staged={}, \
                     committed={}, clean_after={}",
                    self.change_id,
                    reason.as_str(),
                    reason.describe(),
                    self.effects.file_written,
                    self.effects.staged,
                    self.effects.committed,
                    self.effects.clean_after,
                );
                if let Some(detail) = &self.detail {
                    text.push_str(&format!(" ({detail})"));
                }
                text
            }
            (None, None) => format!("refresh of '{}' settled without a result", self.change_id),
        }
    }

    fn refuse(mut self, reason: RefreshReason, detail: Option<String>) -> Self {
        self.refusal = Some(reason);
        if detail.is_some() {
            self.detail = detail;
        }
        self
    }
}

/// A read-only preview of what an apply would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshPreview {
    /// Target change.
    pub change_id: String,
    /// True when no lifecycle or command-time precondition refuses.
    pub eligible: bool,
    /// The first refusal found, when not eligible.
    pub refusal: Option<RefreshReason>,
    /// The ref the caller named.
    pub source_ref: String,
    /// Resolved source commit.
    pub source_commit: Option<String>,
    /// Current candidate `HEAD`.
    pub candidate_head: Option<String>,
    /// The repository-relative path.
    pub path: String,
    /// Source blob identity.
    pub source_blob: Option<String>,
    /// Candidate blob identity.
    pub candidate_blob: Option<String>,
    /// True when the blobs are identical.
    pub no_op: bool,
    /// Unified patch from candidate to source, when both blobs were read.
    pub patch: Option<String>,
    /// Lines the patch adds.
    pub added_lines: usize,
    /// Lines the patch removes.
    pub removed_lines: usize,
    /// Bounded diagnostic detail for a refusal.
    pub detail: Option<String>,
}

impl RefreshPreview {
    /// A preview refused before any Git read.
    pub fn refused(change_id: &str, source_ref: &str, reason: RefreshReason) -> Self {
        Self {
            change_id: change_id.to_string(),
            eligible: false,
            refusal: Some(reason),
            source_ref: source_ref.to_string(),
            source_commit: None,
            candidate_head: None,
            path: proposal_path(change_id),
            source_blob: None,
            candidate_blob: None,
            no_op: false,
            patch: None,
            added_lines: 0,
            removed_lines: 0,
            detail: None,
        }
    }
}

// ============================================================================
// Git access
// ============================================================================

/// Captured output of one Git invocation.
struct GitOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

impl GitOutput {
    fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }
}

/// Run one read-only Git invocation with a finite deadline.
///
/// Inherited repository-selection variables are removed so the invocation always
/// addresses the directory it was given, never one a parent process selected.
async fn git<I, S>(cwd: &Path, args: I) -> Result<GitOutput, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = tokio::process::Command::new("git");
    command
        .args(args)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(GIT_READ_DEADLINE, command.output())
        .await
        .map_err(|_| format!("git did not finish within {GIT_READ_DEADLINE:?}"))?
        .map_err(|error| format!("git could not be started: {error}"))?;
    Ok(GitOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: bounded_tail(&String::from_utf8_lossy(&output.stderr), 1024),
    })
}

/// Keep the last `limit` bytes of `text`, on a character boundary.
fn bounded_tail(text: &str, limit: usize) -> String {
    let text = text.trim_end();
    if text.len() <= limit {
        return text.to_string();
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

/// Literal pathspec, so no character of a change ID is ever a glob.
fn literal(path: &str) -> String {
    format!(":(literal){path}")
}

/// Resolve `source_ref` to a full commit ID in the project repository.
async fn resolve_source(project_root: &Path, source_ref: &str) -> Result<String, RefreshReason> {
    // An option-looking ref is refused outright; `--end-of-options` already keeps
    // it from being parsed as one, and this keeps the refusal typed.
    if source_ref.trim().is_empty() || source_ref.starts_with('-') {
        return Err(RefreshReason::SourceRefUnresolvable);
    }
    let spec = format!("{source_ref}^{{commit}}");
    let output = git(
        project_root,
        [
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .await
    .map_err(|_| RefreshReason::SourceRefUnresolvable)?;
    let oid = output.stdout_text();
    if output.success && is_full_oid(&oid) {
        Ok(oid)
    } else {
        Err(RefreshReason::SourceRefUnresolvable)
    }
}

/// True for a full-length lowercase hexadecimal commit ID (SHA-1 or SHA-256).
pub fn is_full_oid(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Read the candidate's current `HEAD`.
async fn candidate_head(worktree: &Path) -> Result<String, String> {
    let output = git(worktree, ["rev-parse", "--verify", "HEAD^{commit}"]).await?;
    let oid = output.stdout_text();
    if output.success && is_full_oid(&oid) {
        Ok(oid)
    } else {
        Err(format!("HEAD could not be read: {}", output.stderr))
    }
}

/// One tree entry for the proposal path at a commit.
enum TreeEntry {
    /// A regular file blob.
    Regular { blob: String },
    /// Present, but a symlink, gitlink, or tree.
    Unsafe,
    /// Absent.
    Missing,
}

/// Read the proposal path's tree entry at `commit`.
async fn tree_entry(cwd: &Path, commit: &str, path: &str) -> Result<TreeEntry, String> {
    let output = git(
        cwd,
        ["ls-tree", "-z", "--full-tree", commit, "--", &literal(path)],
    )
    .await?;
    if !output.success {
        return Err(format!("ls-tree failed: {}", output.stderr));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(record) = text.split('\0').find(|record| !record.is_empty()) else {
        return Ok(TreeEntry::Missing);
    };
    let (meta, entry_path) = record.split_once('\t').unwrap_or((record, ""));
    if entry_path != path {
        return Ok(TreeEntry::Missing);
    }
    let mut fields = meta.split_whitespace();
    let mode = fields.next().unwrap_or_default();
    let kind = fields.next().unwrap_or_default();
    let oid = fields.next().unwrap_or_default();
    Ok(match (mode, kind) {
        ("100644" | "100755", "blob") => TreeEntry::Regular {
            blob: oid.to_string(),
        },
        _ => TreeEntry::Unsafe,
    })
}

/// Read a blob's exact bytes.
async fn blob_bytes(cwd: &Path, blob: &str) -> Result<Vec<u8>, String> {
    let output = git(cwd, ["cat-file", "blob", blob]).await?;
    if output.success {
        Ok(output.stdout)
    } else {
        Err(format!("cat-file failed: {}", output.stderr))
    }
}

/// Verify every on-disk component of `path` stays inside `worktree` without a
/// symlink, and that the leaf is a regular file.
fn on_disk_path_is_safe(worktree: &Path, path: &str) -> Result<PathBuf, RefreshReason> {
    let mut current = worktree.to_path_buf();
    let components: Vec<&str> = path.split('/').collect();
    for (index, component) in components.iter().enumerate() {
        if component.is_empty() || *component == "." || *component == ".." {
            return Err(RefreshReason::UnsafeProposalPath);
        }
        current.push(component);
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(RefreshReason::CandidateProposalMissing)
            }
            Err(_) => return Err(RefreshReason::UnsafeProposalPath),
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(RefreshReason::UnsafeProposalPath);
        }
        let is_leaf = index + 1 == components.len();
        if (is_leaf && !file_type.is_file()) || (!is_leaf && !file_type.is_dir()) {
            return Err(RefreshReason::UnsafeProposalPath);
        }
    }
    Ok(current)
}

/// Full porcelain status, untracked files included, nothing excluded.
async fn porcelain_entries(worktree: &Path) -> Result<Vec<String>, String> {
    let output = git(
        worktree,
        crate::vcs::git::commands::status_policy::read_only_status_argv(&[
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ]),
    )
    .await?;
    if !output.success {
        return Err(format!("status failed: {}", output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// Markers of an in-progress Git operation inside the candidate's Git directory.
const IN_PROGRESS_MARKERS: [&str; 7] = [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "REBASE_HEAD",
    "rebase-merge",
    "rebase-apply",
    "BISECT_LOG",
];

/// The in-progress markers present in the candidate's Git directory.
async fn in_progress_operations(worktree: &Path) -> Result<Vec<&'static str>, String> {
    let output = git(worktree, ["rev-parse", "--absolute-git-dir"]).await?;
    if !output.success {
        return Err(format!("git dir could not be read: {}", output.stderr));
    }
    let git_dir = PathBuf::from(output.stdout_text());
    Ok(IN_PROGRESS_MARKERS
        .iter()
        .copied()
        .filter(|marker| std::fs::symlink_metadata(git_dir.join(marker)).is_ok())
        .collect())
}

/// Join a list of entries into one bounded diagnostic.
fn bounded_list(entries: &[String]) -> String {
    const SHOWN: usize = 20;
    let mut text = entries
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if entries.len() > SHOWN {
        text.push_str(&format!("; … {} more", entries.len() - SHOWN));
    }
    text
}

/// Facts both the preview and the apply read before deciding anything.
struct Observed {
    source_commit: String,
    candidate_head: String,
    source_blob: String,
    candidate_blob: String,
    leaf: PathBuf,
}

/// Resolve and read everything up to (not including) cleanliness checks.
///
/// Fills `report`-shaped fields as it goes through `record`, so a refusal still
/// names every identity it managed to read.
async fn observe(
    project_root: &Path,
    worktree: &Path,
    change_id: &str,
    source_ref: &str,
    mut record: impl FnMut(ObservedField, String),
) -> Result<Observed, (RefreshReason, Option<String>)> {
    let path = proposal_path(change_id);

    let source_commit = resolve_source(project_root, source_ref)
        .await
        .map_err(|reason| (reason, None))?;
    record(ObservedField::SourceCommit, source_commit.clone());

    let head = candidate_head(worktree)
        .await
        .map_err(|detail| (RefreshReason::GitReadFailed, Some(detail)))?;
    record(ObservedField::CandidateHead, head.clone());

    let source_blob = match tree_entry(project_root, &source_commit, &path).await {
        Ok(TreeEntry::Regular { blob }) => blob,
        Ok(TreeEntry::Unsafe) => return Err((RefreshReason::UnsafeProposalPath, None)),
        Ok(TreeEntry::Missing) => return Err((RefreshReason::SourceProposalMissing, None)),
        Err(detail) => return Err((RefreshReason::GitReadFailed, Some(detail))),
    };
    record(ObservedField::SourceBlob, source_blob.clone());

    let candidate_blob = match tree_entry(worktree, &head, &path).await {
        Ok(TreeEntry::Regular { blob }) => blob,
        Ok(TreeEntry::Unsafe) => return Err((RefreshReason::UnsafeProposalPath, None)),
        Ok(TreeEntry::Missing) => return Err((RefreshReason::CandidateProposalMissing, None)),
        Err(detail) => return Err((RefreshReason::GitReadFailed, Some(detail))),
    };
    record(ObservedField::CandidateBlob, candidate_blob.clone());

    let leaf = on_disk_path_is_safe(worktree, &path).map_err(|reason| (reason, None))?;

    Ok(Observed {
        source_commit,
        candidate_head: head,
        source_blob,
        candidate_blob,
        leaf,
    })
}

/// Which identity [`observe`] just read.
#[derive(Clone, Copy)]
enum ObservedField {
    SourceCommit,
    CandidateHead,
    SourceBlob,
    CandidateBlob,
}

/// Cleanliness and in-progress preconditions, in the frozen order.
async fn preconditions(worktree: &Path) -> Result<(), (RefreshReason, Option<String>)> {
    let entries = porcelain_entries(worktree)
        .await
        .map_err(|detail| (RefreshReason::GitReadFailed, Some(detail)))?;
    if !entries.is_empty() {
        return Err((RefreshReason::CandidateDirty, Some(bounded_list(&entries))));
    }
    let markers = in_progress_operations(worktree)
        .await
        .map_err(|detail| (RefreshReason::GitReadFailed, Some(detail)))?;
    if !markers.is_empty() {
        return Err((
            RefreshReason::CandidateOperationInProgress,
            Some(markers.join(", ")),
        ));
    }
    Ok(())
}

/// Unified patch from the candidate's proposal to the source's, with line counts.
async fn patch_between(
    project_root: &Path,
    candidate_head: &str,
    source_commit: &str,
    path: &str,
) -> Result<(String, usize, usize), String> {
    let output = git(
        project_root,
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            candidate_head,
            source_commit,
            "--",
            &literal(path),
        ],
    )
    .await?;
    if !output.success {
        return Err(format!("diff failed: {}", output.stderr));
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let added = text
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    let removed = text
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count();
    let patch = if text.len() > PREVIEW_PATCH_LIMIT_BYTES {
        let mut end = PREVIEW_PATCH_LIMIT_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n… patch truncated\n", &text[..end])
    } else {
        text
    };
    Ok((patch, added, removed))
}

// ============================================================================
// Preview
// ============================================================================

/// Compute a read-only preview for an already-admitted target.
///
/// Runs the same command-time preconditions an apply runs, in the same order,
/// and writes nothing: no file, no index entry, no ref, no command record.
pub async fn preview(
    project_root: &Path,
    worktree: &Path,
    change_id: &str,
    source_ref: &str,
) -> RefreshPreview {
    let mut preview = RefreshPreview::refused(change_id, source_ref, RefreshReason::GitReadFailed);
    preview.refusal = None;

    let observed = {
        let preview = &mut preview;
        observe(
            project_root,
            worktree,
            change_id,
            source_ref,
            |field, value| match field {
                ObservedField::SourceCommit => preview.source_commit = Some(value),
                ObservedField::CandidateHead => preview.candidate_head = Some(value),
                ObservedField::SourceBlob => preview.source_blob = Some(value),
                ObservedField::CandidateBlob => preview.candidate_blob = Some(value),
            },
        )
        .await
    };
    let observed = match observed {
        Ok(observed) => observed,
        Err((reason, detail)) => {
            preview.refusal = Some(reason);
            preview.detail = detail;
            return preview;
        }
    };

    preview.no_op = observed.source_blob == observed.candidate_blob;
    if !preview.no_op {
        match patch_between(
            project_root,
            &observed.candidate_head,
            &observed.source_commit,
            &preview.path,
        )
        .await
        {
            Ok((patch, added, removed)) => {
                preview.patch = Some(patch);
                preview.added_lines = added;
                preview.removed_lines = removed;
            }
            Err(detail) => {
                preview.refusal = Some(RefreshReason::GitReadFailed);
                preview.detail = Some(detail);
                return preview;
            }
        }
    }

    if let Err((reason, detail)) = preconditions(worktree).await {
        preview.refusal = Some(reason);
        preview.detail = detail;
        return preview;
    }
    preview.eligible = true;
    preview
}

// ============================================================================
// Apply
// ============================================================================

/// The caller's expectations for one apply, taken from a preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshRequest {
    /// Target change.
    pub change_id: String,
    /// Explicit source ref.
    pub source_ref: String,
    /// Full commit ID the source ref must still resolve to.
    pub expected_source: String,
    /// Full commit ID the candidate `HEAD` must still be at.
    pub expected_head: String,
}

/// Apply one refresh to an already-admitted target.
///
/// Fails closed at the first violated precondition with nothing written. After
/// the write, nothing is ever reset, amended, or cleaned: the report's effects
/// are re-read from Git whatever happened.
pub async fn apply(
    project_root: &Path,
    worktree: &Path,
    request: &RefreshRequest,
) -> RefreshReport {
    let change_id = request.change_id.as_str();
    let mut report = RefreshReport::new(change_id, &request.source_ref);

    let observed = {
        let report = &mut report;
        observe(
            project_root,
            worktree,
            change_id,
            &request.source_ref,
            |field, value| match field {
                ObservedField::SourceCommit => report.source_commit = Some(value),
                ObservedField::CandidateHead => {
                    report.candidate_head_before = Some(value.clone());
                    report.candidate_head_after = Some(value);
                }
                ObservedField::SourceBlob => report.source_blob = Some(value),
                ObservedField::CandidateBlob => report.candidate_blob_before = Some(value),
            },
        )
        .await
    };

    // Expectations are checked as soon as both identities are known, so a moved
    // source or head is reported as a mismatch rather than as whatever a later
    // read happens to trip over.
    let mismatch = |report: &RefreshReport| -> Option<String> {
        let source_moved = report
            .source_commit
            .as_deref()
            .is_some_and(|current| current != request.expected_source);
        let head_moved = report
            .candidate_head_before
            .as_deref()
            .is_some_and(|current| current != request.expected_head);
        (source_moved || head_moved).then(|| {
            format!(
                "expected_source {} / current {}; expected_head {} / current {}",
                request.expected_source,
                report.source_commit.as_deref().unwrap_or("unknown"),
                request.expected_head,
                report.candidate_head_before.as_deref().unwrap_or("unknown"),
            )
        })
    };

    let observed = match observed {
        Ok(observed) => observed,
        Err((reason, detail)) => {
            let report = match mismatch(&report) {
                Some(detail) => {
                    report.refuse(RefreshReason::RefreshExpectationMismatch, Some(detail))
                }
                None => report.refuse(reason, detail),
            };
            return finish_without_write(report, worktree).await;
        }
    };
    if let Some(detail) = mismatch(&report) {
        let report = report.refuse(RefreshReason::RefreshExpectationMismatch, Some(detail));
        return finish_without_write(report, worktree).await;
    }

    if let Err((reason, detail)) = preconditions(worktree).await {
        let report = report.refuse(reason, detail);
        return finish_without_write(report, worktree).await;
    }

    if observed.source_blob == observed.candidate_blob {
        report.outcome = Some(RefreshOutcome::Unchanged);
        return finish_without_write(report, worktree).await;
    }

    // 1. Write the source bytes over the existing regular file. Opening it for
    //    truncation keeps its mode, and `O_NOFOLLOW` refuses a leaf that turned
    //    into a symlink after the check above.
    let bytes = match blob_bytes(project_root, &observed.source_blob).await {
        Ok(bytes) => bytes,
        Err(detail) => {
            let report = report.refuse(RefreshReason::GitReadFailed, Some(detail));
            return finish_without_write(report, worktree).await;
        }
    };
    if let Err(error) = write_no_follow(&observed.leaf, &bytes) {
        // The open may have truncated before the write failed, so whether the
        // file changed is read back rather than assumed.
        report.effects.file_written = std::fs::read(&observed.leaf)
            .map(|current| current == bytes)
            .unwrap_or(false);
        let report = report.refuse(RefreshReason::WriteFailed, Some(error.to_string()));
        return finish(report, worktree, &request.expected_head).await;
    }
    report.effects.file_written = true;

    // 2. Stage exactly the path.
    let path = proposal_path(change_id);
    match git(worktree, ["add", "--", &literal(&path)]).await {
        Ok(output) if output.success => report.effects.staged = true,
        Ok(output) => {
            let report = report.refuse(
                RefreshReason::CommitFailed,
                Some(format!("staging failed: {}", output.stderr)),
            );
            return finish(report, worktree, &request.expected_head).await;
        }
        Err(detail) => {
            let report = report.refuse(
                RefreshReason::CommitFailed,
                Some(format!("staging failed: {detail}")),
            );
            return finish(report, worktree, &request.expected_head).await;
        }
    }

    // 3. Commit with hooks enabled, streaming output to the owner log.
    let message = format!(
        "Refresh proposal: {change_id} from {}",
        &observed.source_commit[..12]
    );
    let mut tail = String::new();
    let commit = crate::vcs::commands::run_vcs_command_streamed(
        "git",
        &["commit", "-m", &message],
        worktree,
        crate::vcs::VcsBackend::Git,
        |stream, line| {
            tracing::info!(
                change_id = %change_id,
                operation = "refresh_proposal",
                stream = stream.as_str(),
                "{line}"
            );
            tail.push_str(line);
            tail.push('\n');
            if tail.len() > HOOK_OUTPUT_TAIL_BYTES * 2 {
                tail = bounded_tail(&tail, HOOK_OUTPUT_TAIL_BYTES);
                tail.push('\n');
            }
        },
    )
    .await;
    report.hook_output_tail = Some(bounded_tail(&tail, HOOK_OUTPUT_TAIL_BYTES));

    match commit {
        Err(error) => {
            let report = report.refuse(RefreshReason::CommitFailed, Some(error.to_string()));
            finish(report, worktree, &request.expected_head).await
        }
        Ok(output) if !output.success => {
            let reason = if output.exit_code
                == Some(crate::vcs::git::commands::commit::GIT_REPOSITORY_REJECTION_EXIT_CODE)
            {
                RefreshReason::HookRejected
            } else {
                RefreshReason::CommitFailed
            };
            let report = report.refuse(reason, Some(bounded_tail(&output.stderr, 1024)));
            finish(report, worktree, &request.expected_head).await
        }
        Ok(_) => verify_commit(report, worktree, &request.expected_head, &path).await,
    }
}

/// Write `bytes` over an existing regular file without following a symlink.
fn write_no_follow(leaf: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(leaf)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Post-commit verification: exactly one commit on `expected_head`, changing
/// exactly the path, with a clean worktree afterwards.
async fn verify_commit(
    report: RefreshReport,
    worktree: &Path,
    expected_head: &str,
    path: &str,
) -> RefreshReport {
    let mut report = finish(report, worktree, expected_head).await;
    if !report.effects.committed {
        report.refusal = Some(RefreshReason::CommitFailed);
        report.detail = Some("git commit succeeded but HEAD did not move".to_string());
        return report;
    }

    let parents = git(worktree, ["rev-list", "--parents", "-n", "1", "HEAD"])
        .await
        .ok()
        .filter(|output| output.success)
        .map(|output| {
            output
                .stdout_text()
                .split_whitespace()
                .skip(1)
                .map(str::to_string)
                .collect::<Vec<_>>()
        });
    let changed = git(
        worktree,
        ["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    )
    .await
    .ok()
    .filter(|output| output.success)
    .map(|output| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    });

    let sole_parent = parents.as_deref() == Some(&[expected_head.to_string()][..]);
    let exact_tree = changed.as_deref() == Some(&[path.to_string()][..]);
    if !sole_parent || !exact_tree {
        report.refusal = Some(RefreshReason::RefreshCommitUnexpectedTree);
        report.detail = Some(format!(
            "parents {:?}; changed paths {:?}",
            parents.unwrap_or_default(),
            changed
                .map(|paths| bounded_list(&paths))
                .unwrap_or_default()
        ));
        return report;
    }
    if !report.effects.clean_after {
        report.refusal = Some(RefreshReason::HookLeftWorkspaceDirty);
        report.detail = porcelain_entries(worktree)
            .await
            .ok()
            .map(|entries| bounded_list(&entries));
        return report;
    }
    report.outcome = Some(RefreshOutcome::Refreshed);
    report
}

/// Re-read `HEAD` and cleanliness after a write was attempted.
async fn finish(mut report: RefreshReport, worktree: &Path, expected_head: &str) -> RefreshReport {
    let head_after = candidate_head(worktree).await.ok();
    report.effects.committed = head_after
        .as_deref()
        .is_some_and(|head| head != expected_head);
    report.candidate_head_after = head_after;
    report.effects.clean_after = observe_clean(worktree).await;
    report
}

/// Re-read cleanliness for a result that wrote nothing.
async fn finish_without_write(mut report: RefreshReport, worktree: &Path) -> RefreshReport {
    report.effects.clean_after = observe_clean(worktree).await;
    report
}

/// Observed cleanliness; an unreadable status is never reported as clean.
async fn observe_clean(worktree: &Path) -> bool {
    porcelain_entries(worktree)
        .await
        .is_ok_and(|entries| entries.is_empty())
}

/// Real-Git fixtures are shared with the operator-service and manifest tests.
#[cfg(test)]
pub(crate) mod tests;
