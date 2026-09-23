//! Bound verification evidence: reuse an Apply-time run only when the
//! repository still proves it.
//!
//! Apply frequently runs exactly the repository-local command a proposal
//! declares for Acceptance. Repeating it costs the operator real time, but the
//! cheap ways to avoid the repeat — a log line, a narrative claim, an
//! agent-authored "it passed" field — are all ways to turn *no evidence* into
//! PASS. This module is the opposite trade: a reuse is allowed only when every
//! binding in a runtime-written envelope still matches what Git says right now,
//! and every other outcome selects the ordinary rerun path.
//!
//! Three properties make that trustworthy rather than merely plausible:
//!
//! * **The runtime is the only author.** [`RuntimeVerificationExecutor`] starts
//!   the declared argv itself, waits for it, hashes the artifact it captured,
//!   and writes the envelope. Nothing in the envelope is copied from an agent's
//!   claim, and an envelope that does not carry the runtime's own authority
//!   marking is rejected before any field is compared.
//! * **Validation is an all-fields conjunction.** [`evaluate_reuse`] has no
//!   partial score and no freshness heuristic. Commit, tree, argv, cwd,
//!   automation blob, tool identity, artifact digest, exit code, and clean state
//!   must *all* match; the first mismatch is the rerun reason.
//! * **Nothing lives inside the target.** The envelope and its artifact are
//!   written into a Conflux-owned store below the resolved state root, never
//!   into the target repository or any managed worktree, which is what
//!   constitutional law 4 requires. [`EvidenceStore`] is handed that directory
//!   explicitly; it cannot derive one by joining a workspace with a hidden
//!   sidecar path, so there is no code path left that could pollute a target.
//!
//! **The external record is cache, not authority.** Constitutional law 1 keeps
//! workflow control in the workspace, so a missing, unreadable, malformed,
//! stale, or deliberately deleted record means exactly one thing: run the
//! declared command again. It never implies PASS, never suppresses work, and
//! never selects the next workflow action. That is why cleanliness is now
//! evaluated with *no* Conflux-owned path excluded: the store is outside the
//! tree, so there is nothing left to exclude, and writing evidence can no longer
//! invalidate the evidence it is writing.
//!
//! **What the authority check is and is not.** The envelope is written `0400`
//! inside a `0700` runtime-owned directory, and a file whose mode says otherwise
//! is refused as agent-authored. A process running under the same UID can
//! `chmod` past that, so this is default mutation refusal rather than an
//! integrity guarantee — the same boundary the completion-callback event file
//! draws. What keeps a forged envelope harmless is that forging it buys nothing
//! a rerun would not also have to satisfy: every binding is still compared
//! against live Git state, so the only envelope that survives validation is one
//! describing a command that really did run against exactly this tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::openspec::{
    VerificationCompletionRole, VerificationDeclaration, VerificationExecutionClass,
};

/// Envelope schema.
///
/// Version 2 is where the record left the target worktree: `artifact_path` is
/// store-relative rather than repository-relative, and `clean_before` /
/// `clean_after` mean full target cleanliness with no Conflux path excluded. A
/// version-1 record describes a different world — a repository-local sidecar and
/// an evidence-path-excluded clean state — so it is refused rather than
/// migrated. Refusal costs one rerun; migration would carry a cleanliness claim
/// that was never made.
pub const EVIDENCE_SCHEMA: &str = "conflux-verification-evidence-v2";

/// Marking that says the Conflux runtime executor wrote this record.
pub const EVIDENCE_AUTHORITY: &str = "conflux-runtime-executor";

/// Store-relative directory holding envelopes and their artifacts.
pub const GATES_DIR: &str = "gates";

/// The target-local path earlier releases wrote Acceptance evidence into.
///
/// Retained as a *name to probe for*, never as a path to write, read, parse,
/// migrate, or delete. Its existence in a target is reported so a repository
/// owner can remove it explicitly; nothing in this runtime touches its content.
pub const LEGACY_EVIDENCE_DIR: &str = ".cflx/verification-evidence";

/// Whether a target carries legacy Conflux-owned evidence.
///
/// Existence only. `symlink_metadata` is used rather than `exists` so a dangling
/// symlink at that path is still reported instead of read through, and no entry
/// below it is opened, listed, or parsed. Self-ignored content is detected for
/// the same reason: this is a filesystem probe, not a `git status` question, so
/// an ignore rule cannot hide it.
///
/// Exactly one path is evaluated. Unrelated ignored paths are none of this
/// probe's business, and widening it would reintroduce the Conflux-specific
/// status filtering constitutional law 4 forbids.
pub fn legacy_target_evidence(workspace: &Path) -> Option<PathBuf> {
    let path = workspace.join(LEGACY_EVIDENCE_DIR);
    std::fs::symlink_metadata(&path).ok().map(|_| path)
}

/// Default minimum elapsed duration eligible for reuse.
///
/// Repository-tracked on purpose: a proposal's prose and an agent's claim are
/// both inputs this policy refuses to take. Anything faster than this is cheaper
/// to rerun than to justify, so format/lint/fast-unit commands stay on the
/// existing path.
pub const DEFAULT_MIN_REUSE_SECONDS: i64 = 60;

/// Longest artifact this module will hash and retain, in bytes.
const MAX_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;

/// Repository-tracked reuse policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReusePolicy {
    /// Commands whose measured elapsed duration is below this are rerun.
    pub min_reuse_seconds: i64,
}

impl Default for ReusePolicy {
    fn default() -> Self {
        Self {
            min_reuse_seconds: DEFAULT_MIN_REUSE_SECONDS,
        }
    }
}

/// Identity of the executable that produced (or would produce) a result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolIdentity {
    /// Absolute path of the resolved executable.
    pub path: String,
    /// Full-length content digest of the executable file itself.
    pub executable_digest: String,
    /// Exact version output, when the tool reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// One runtime-supervised verification result, bound to the repository state it
/// was produced from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub schema: String,
    pub authority: String,
    pub verification_id: String,
    /// Full-length Apply commit object ID the run observed.
    pub commit_oid: String,
    /// Full-length tree ID of that commit.
    pub tree_oid: String,
    /// Full-length review base commit the candidate is being judged against, or
    /// empty when no base was resolvable.
    ///
    /// Bound because "this command succeeded" is only reusable evidence for the
    /// review it was produced under. The same argv at the same commit against a
    /// *different* base is a different question, and reusing the older answer
    /// would silently carry a verdict across review ranges. Defaulted so an
    /// envelope written before this binding existed reads as "no base recorded"
    /// and reruns, which is the conservative direction.
    #[serde(default)]
    pub review_base_commit: String,
    /// Exact `base..candidate` range the review covers.
    #[serde(default)]
    pub review_range: String,
    /// Exact argv, as an array. Never a shell string.
    pub argv: Vec<String>,
    /// Working directory, normalized repository-relative (`.` for the root).
    pub cwd: String,
    /// Repository-relative path of the declared automation file.
    pub automation_path: String,
    /// Full-length blob ID of that file at `commit_oid`.
    pub automation_blob_oid: String,
    pub tool: ToolIdentity,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub exit_code: i32,
    /// Store-relative path of the captured output artifact (`gates/<id>.log`).
    ///
    /// Store-relative rather than absolute so the cache survives a state root
    /// that moves — a configured `state_base_dir` change, a different machine —
    /// without every record becoming unreadable, and so nothing in the envelope
    /// names a location inside the target.
    pub artifact_path: String,
    /// Full-length content digest of that artifact.
    pub artifact_digest: String,
    /// Full target cleanliness observed before execution, with no Conflux-owned
    /// path excluded.
    pub clean_before: bool,
    /// Full target cleanliness observed after execution, with no Conflux-owned
    /// path excluded.
    pub clean_after: bool,
}

impl VerificationEvidence {
    /// Measured elapsed seconds, or `None` when the record is not ordered.
    pub fn elapsed_seconds(&self) -> Option<i64> {
        let elapsed = self.ended_at.signed_duration_since(self.started_at);
        let seconds = elapsed.num_seconds();
        (seconds >= 0).then_some(seconds)
    }

    /// Machine-readable summary for logs and operator output.
    ///
    /// The artifact digest is included; the artifact *contents* are not, because
    /// this is an identity report rather than a result report.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "verification_id": self.verification_id,
            "commit_oid": self.commit_oid,
            "tree_oid": self.tree_oid,
            "review_base_commit": self.review_base_commit,
            "review_range": self.review_range,
            "argv": self.argv,
            "cwd": self.cwd,
            "automation_path": self.automation_path,
            "automation_blob_oid": self.automation_blob_oid,
            "tool_path": self.tool.path,
            "tool_digest": self.tool.executable_digest,
            "tool_version": self.tool.version,
            "exit_code": self.exit_code,
            "artifact_path": self.artifact_path,
            "artifact_digest": self.artifact_digest,
            "elapsed_seconds": self.elapsed_seconds(),
        })
    }
}

/// Why a stored record could not even be read as a candidate.
///
/// Every variant means rerun. None of them means the implementation failed: a
/// malformed sidecar says nothing about the change under review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceDefect {
    Missing,
    Unreadable(String),
    Malformed(String),
    UnknownSchema(String),
    /// The record does not carry the runtime executor's authority marking, or
    /// its file mode says something other than the runtime wrote it.
    AgentAuthored(String),
}

impl EvidenceDefect {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing => "evidence_missing",
            Self::Unreadable(_) => "evidence_unreadable",
            Self::Malformed(_) => "evidence_malformed",
            Self::UnknownSchema(_) => "evidence_unknown_schema",
            Self::AgentAuthored(_) => "evidence_agent_authored",
        }
    }

    /// Operator-facing detail naming what to look at.
    pub fn detail(&self) -> String {
        match self {
            Self::Missing => "no runtime-authored evidence sidecar exists".to_string(),
            Self::Unreadable(detail) => format!("evidence sidecar could not be read: {detail}"),
            Self::Malformed(detail) => format!("evidence sidecar is malformed: {detail}"),
            Self::UnknownSchema(found) => {
                format!("evidence schema '{found}' is not '{EVIDENCE_SCHEMA}'")
            }
            Self::AgentAuthored(detail) => {
                format!("evidence sidecar is not runtime-authored: {detail}")
            }
        }
    }
}

/// A full-length Git object ID or content digest.
///
/// Short IDs are refused rather than resolved: an abbreviation is ambiguous by
/// construction, and resolving one would let a record bind to a commit it never
/// observed.
fn is_full_length_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Whether a verification ID is usable as an evidence file name.
///
/// Anything that could escape the store's `gates/` directory or collide across
/// changes is refused; the declaration is corrected rather than sanitized here.
/// The same single-component rule the external store applies to every other
/// identity it puts on a path, so a name that is safe in one place is safe in
/// all of them.
pub fn is_storable_verification_id(id: &str) -> bool {
    crate::config::defaults::is_safe_path_component(id)
}

/// Normalize a repository-relative working directory.
///
/// Returns `None` for absolute paths and for anything containing `..`, because
/// a record whose cwd cannot be re-derived from the repository root is not
/// repository-local evidence.
pub fn normalize_relative_cwd(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == "./" {
        return Some(".".to_string());
    }
    if trimmed.starts_with('/') || trimmed.contains('\\') {
        return None;
    }
    let mut segments = Vec::new();
    for segment in trimmed.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return None,
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        Some(".".to_string())
    } else {
        Some(segments.join("/"))
    }
}

/// Parse and structurally validate a stored envelope.
///
/// Fail-closed: every required field must be present, every object ID must be
/// full length, the timestamps must be ordered, the exit status must be success,
/// and the artifact must live inside the store's own `gates/` directory. A
/// record that fails any of these is not a candidate at all.
pub fn parse_evidence(bytes: &[u8]) -> Result<VerificationEvidence, EvidenceDefect> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| EvidenceDefect::Malformed(error.to_string()))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if schema != EVIDENCE_SCHEMA {
        return Err(EvidenceDefect::UnknownSchema(schema));
    }
    let record: VerificationEvidence = serde_json::from_value(value)
        .map_err(|error| EvidenceDefect::Malformed(error.to_string()))?;

    if record.authority != EVIDENCE_AUTHORITY {
        return Err(EvidenceDefect::AgentAuthored(format!(
            "authority '{}' is not '{EVIDENCE_AUTHORITY}'",
            record.authority
        )));
    }
    if !is_storable_verification_id(&record.verification_id) {
        return Err(EvidenceDefect::Malformed(format!(
            "verification id '{}' is not storable",
            record.verification_id
        )));
    }
    for (field, value) in [
        ("commit_oid", &record.commit_oid),
        ("tree_oid", &record.tree_oid),
        ("automation_blob_oid", &record.automation_blob_oid),
        ("artifact_digest", &record.artifact_digest),
        ("tool.executable_digest", &record.tool.executable_digest),
    ] {
        if !is_full_length_object_id(value) {
            return Err(EvidenceDefect::Malformed(format!(
                "{field} '{value}' is not a full-length object id"
            )));
        }
    }
    // An empty review base is a real state — a first commit, a detached
    // candidate — and stays legal. A *present* one must be a full object ID for
    // the same reason every other binding must: an abbreviation is ambiguous,
    // and an ambiguous base cannot prove which review this evidence belongs to.
    if !record.review_base_commit.is_empty()
        && !is_full_length_object_id(&record.review_base_commit)
    {
        return Err(EvidenceDefect::Malformed(format!(
            "review_base_commit '{}' is not a full-length object id",
            record.review_base_commit
        )));
    }
    if record.argv.is_empty() || record.argv.iter().any(|arg| arg.trim().is_empty()) {
        return Err(EvidenceDefect::Malformed(
            "argv must be a non-empty array of non-empty arguments".to_string(),
        ));
    }
    if normalize_relative_cwd(&record.cwd).as_deref() != Some(record.cwd.as_str()) {
        return Err(EvidenceDefect::Malformed(format!(
            "cwd '{}' is not a normalized repository-relative path",
            record.cwd
        )));
    }
    if record.automation_path.trim().is_empty() || record.tool.path.trim().is_empty() {
        return Err(EvidenceDefect::Malformed(
            "automation path and tool path are required".to_string(),
        ));
    }
    if record.artifact_path != EvidenceStore::artifact_relative_path(&record.verification_id) {
        return Err(EvidenceDefect::Malformed(format!(
            "artifact path '{}' is not the store-relative '{}'",
            record.artifact_path,
            EvidenceStore::artifact_relative_path(&record.verification_id)
        )));
    }
    if record.elapsed_seconds().is_none() {
        return Err(EvidenceDefect::Malformed(
            "ended_at precedes started_at".to_string(),
        ));
    }
    if record.exit_code != 0 {
        return Err(EvidenceDefect::Malformed(format!(
            "exit code {} is not success",
            record.exit_code
        )));
    }
    if !record.clean_before || !record.clean_after {
        return Err(EvidenceDefect::Malformed(
            "capture recorded a dirty index or worktree".to_string(),
        ));
    }
    Ok(record)
}

/// Why a declared verification is not a reuse candidate at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IneligibleReason {
    MissingId,
    UnstorableId(String),
    NotRepositoryLocal,
    NotChangeBlocking,
    MissingCommand,
    MissingAutomation,
    ShellSyntax(String),
}

impl IneligibleReason {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingId => "declaration_missing_id",
            Self::UnstorableId(_) => "declaration_unstorable_id",
            Self::NotRepositoryLocal => "declaration_not_repository_local",
            Self::NotChangeBlocking => "declaration_not_change_blocking",
            Self::MissingCommand => "declaration_missing_command",
            Self::MissingAutomation => "declaration_missing_automation",
            Self::ShellSyntax(_) => "declaration_shell_syntax",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::MissingId => "verification declaration has no id".to_string(),
            Self::UnstorableId(id) => format!("verification id '{id}' is not storable"),
            Self::NotRepositoryLocal => {
                "only execution_class: repository-local evidence is reusable".to_string()
            }
            Self::NotChangeBlocking => {
                "only completion_role: change-blocking evidence is reusable".to_string()
            }
            Self::MissingCommand => "declaration has no evidence/rerun command".to_string(),
            Self::MissingAutomation => "declaration has no automation path".to_string(),
            Self::ShellSyntax(command) => {
                format!("command '{command}' is not a plain argv the runtime can supervise")
            }
        }
    }
}

/// A declaration the runtime is willing to supervise and later reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub verification_id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub automation_path: String,
}

/// Shell metacharacters that mean the declared command is not a plain argv.
///
/// The runtime executor starts a process directly, so a pipeline, a redirection,
/// or a conditional is not something it can supervise — and pretending otherwise
/// would bind evidence to a command that never ran that way.
const SHELL_METACHARACTERS: [char; 9] = ['|', '&', ';', '<', '>', '(', ')', '`', '$'];

/// Derive a supervisable request from a proposal declaration.
pub fn eligible_request(
    declaration: &VerificationDeclaration,
) -> Result<VerificationRequest, IneligibleReason> {
    let id = declaration
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or(IneligibleReason::MissingId)?;
    if !is_storable_verification_id(id) {
        return Err(IneligibleReason::UnstorableId(id.to_string()));
    }
    let execution_class = declaration
        .execution_class
        .as_deref()
        .and_then(VerificationExecutionClass::parse);
    if !execution_class.is_some_and(VerificationExecutionClass::is_repository_local) {
        return Err(IneligibleReason::NotRepositoryLocal);
    }
    let completion_role = declaration
        .completion_role
        .as_deref()
        .and_then(VerificationCompletionRole::parse);
    if completion_role != Some(VerificationCompletionRole::ChangeBlocking) {
        return Err(IneligibleReason::NotChangeBlocking);
    }
    let command = declaration
        .rerun
        .as_deref()
        .or(declaration.evidence.as_deref())
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or(IneligibleReason::MissingCommand)?;
    if command.contains(SHELL_METACHARACTERS) {
        return Err(IneligibleReason::ShellSyntax(command.to_string()));
    }
    let argv = shlex::split(command)
        .filter(|argv| !argv.is_empty() && argv.iter().all(|arg| !arg.trim().is_empty()))
        .ok_or_else(|| IneligibleReason::ShellSyntax(command.to_string()))?;
    let automation_path = declaration
        .automation
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or(IneligibleReason::MissingAutomation)?;
    if normalize_relative_cwd(automation_path).is_none() {
        return Err(IneligibleReason::MissingAutomation);
    }
    Ok(VerificationRequest {
        verification_id: id.to_string(),
        argv,
        cwd: ".".to_string(),
        automation_path: automation_path.to_string(),
    })
}

/// Repository state the validator compares an envelope against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentBindings {
    pub commit_oid: String,
    pub tree_oid: String,
    /// The review this comparison is being made for, when the caller binds one.
    ///
    /// `None` is not "no base": it is *this caller asks no review question*, and
    /// the check is skipped rather than answered against a value nobody
    /// supplied. `Some` with empty fields is the opposite — a review with no
    /// resolvable base — and an envelope recorded under a real base mismatches
    /// it, which is exactly the rerun that case deserves.
    pub review: Option<ReviewBinding>,
    pub automation_blob_oid: String,
    pub tool: ToolIdentity,
    /// Full target cleanliness, with no Conflux-owned path excluded.
    pub clean: bool,
    /// Digest of the artifact as it exists now, or `None` when it is absent or
    /// unreadable.
    pub artifact_digest: Option<String>,
}

/// Why Acceptance must run the declared command itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerunReason {
    Ineligible(IneligibleReason),
    Defect(EvidenceDefect),
    /// A binding named by `field` differs between the record and current state.
    Mismatch {
        field: &'static str,
        recorded: String,
        current: String,
    },
    /// The target carries staged, unstaged, or untracked changes.
    DirtyWorktree(Vec<String>),
    /// Current repository state could not be observed at all.
    Unobservable(String),
    /// The measured run was cheaper than the reuse threshold.
    BelowDurationThreshold {
        elapsed: i64,
        threshold: i64,
    },
}

impl RerunReason {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Ineligible(reason) => reason.code(),
            Self::Defect(defect) => defect.code(),
            Self::Mismatch { .. } => "binding_mismatch",
            Self::DirtyWorktree(_) => "worktree_dirty",
            Self::Unobservable(_) => "state_unobservable",
            Self::BelowDurationThreshold { .. } => "below_reuse_duration_threshold",
        }
    }

    /// Operator-facing detail naming exactly what to look at.
    pub fn detail(&self) -> String {
        match self {
            Self::Ineligible(reason) => reason.detail(),
            Self::Defect(defect) => defect.detail(),
            Self::Mismatch {
                field,
                recorded,
                current,
            } => format!("{field} changed: evidence '{recorded}', current '{current}'"),
            Self::DirtyWorktree(entries) => format!(
                "worktree differs from the bound commit: {}",
                entries.join(", ")
            ),
            Self::Unobservable(detail) => {
                format!("current repository state could not be proven: {detail}")
            }
            Self::BelowDurationThreshold { elapsed, threshold } => format!(
                "measured {elapsed}s is below the {threshold}s reuse threshold; cheap commands rerun"
            ),
        }
    }
}

/// What Acceptance should do with one declared verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReuseDecision {
    /// Every binding matched; the recorded success stands without rerunning.
    Reuse {
        verification_id: String,
        evidence: Box<VerificationEvidence>,
    },
    /// Run the declared command. Never a verdict about the change.
    Rerun {
        verification_id: String,
        reason: RerunReason,
    },
}

impl ReuseDecision {
    pub fn verification_id(&self) -> &str {
        match self {
            Self::Reuse {
                verification_id, ..
            }
            | Self::Rerun {
                verification_id, ..
            } => verification_id,
        }
    }

    pub fn is_reuse(&self) -> bool {
        matches!(self, Self::Reuse { .. })
    }

    /// `reused` or `rerun`, the two words operator output uses.
    pub fn outcome(&self) -> &'static str {
        if self.is_reuse() {
            "reused"
        } else {
            "rerun"
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Reuse {
                verification_id,
                evidence,
            } => serde_json::json!({
                "verification_id": verification_id,
                "outcome": "reused",
                "evidence": evidence.to_json(),
            }),
            Self::Rerun {
                verification_id,
                reason,
            } => serde_json::json!({
                "verification_id": verification_id,
                "outcome": "rerun",
                "reason": reason.code(),
                "detail": reason.detail(),
            }),
        }
    }

    /// One operator-facing line.
    pub fn summary(&self) -> String {
        match self {
            Self::Reuse {
                verification_id,
                evidence,
            } => format!(
                "{verification_id}: reused (commit {}, {}s, artifact {})",
                &evidence.commit_oid[..12.min(evidence.commit_oid.len())],
                evidence.elapsed_seconds().unwrap_or_default(),
                evidence.artifact_path
            ),
            Self::Rerun {
                verification_id,
                reason,
            } => format!(
                "{verification_id}: rerun ({}) {}",
                reason.code(),
                reason.detail()
            ),
        }
    }
}

/// Per-verification reuse report for one change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerificationReusePlan {
    pub decisions: Vec<ReuseDecision>,
}

impl VerificationReusePlan {
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
    }

    pub fn reused_ids(&self) -> Vec<&str> {
        self.decisions
            .iter()
            .filter(|decision| decision.is_reuse())
            .map(ReuseDecision::verification_id)
            .collect()
    }

    pub fn rerun_ids(&self) -> Vec<&str> {
        self.decisions
            .iter()
            .filter(|decision| !decision.is_reuse())
            .map(ReuseDecision::verification_id)
            .collect()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "verifications": self
                .decisions
                .iter()
                .map(ReuseDecision::to_json)
                .collect::<Vec<_>>(),
            "reused": self.reused_ids(),
            "rerun": self.rerun_ids(),
        })
    }
}

/// Compare one parsed record against current repository state.
///
/// All-fields conjunction, first mismatch wins. The order is deliberate: the
/// cheapest and most explanatory bindings are checked first so the reported
/// reason names the thing an operator would have changed.
pub fn evaluate_reuse(
    request: &VerificationRequest,
    record: &VerificationEvidence,
    current: &CurrentBindings,
    policy: ReusePolicy,
) -> ReuseDecision {
    let id = request.verification_id.clone();
    let rerun = |reason: RerunReason| ReuseDecision::Rerun {
        verification_id: id.clone(),
        reason,
    };

    if record.verification_id != request.verification_id {
        return rerun(RerunReason::Mismatch {
            field: "verification_id",
            recorded: record.verification_id.clone(),
            current: request.verification_id.clone(),
        });
    }
    if record.argv != request.argv {
        return rerun(RerunReason::Mismatch {
            field: "argv",
            recorded: record.argv.join(" "),
            current: request.argv.join(" "),
        });
    }
    if record.cwd != request.cwd {
        return rerun(RerunReason::Mismatch {
            field: "cwd",
            recorded: record.cwd.clone(),
            current: request.cwd.clone(),
        });
    }
    if record.automation_path != request.automation_path {
        return rerun(RerunReason::Mismatch {
            field: "automation_path",
            recorded: record.automation_path.clone(),
            current: request.automation_path.clone(),
        });
    }
    if record.commit_oid != current.commit_oid {
        return rerun(RerunReason::Mismatch {
            field: "commit_oid",
            recorded: record.commit_oid.clone(),
            current: current.commit_oid.clone(),
        });
    }
    if record.tree_oid != current.tree_oid {
        return rerun(RerunReason::Mismatch {
            field: "tree_oid",
            recorded: record.tree_oid.clone(),
            current: current.tree_oid.clone(),
        });
    }
    // Checked right after the candidate, and before the cheaper bindings, because
    // this is the mismatch an operator is most likely to have *caused*: rebasing
    // or repointing the base branch changes what the review covers without
    // touching the candidate at all, and evidence produced against the old range
    // is not evidence about the new one.
    if let Some(review) = current.review.as_ref() {
        if record.review_base_commit != review.base_commit {
            return rerun(RerunReason::Mismatch {
                field: "review_base_commit",
                recorded: record.review_base_commit.clone(),
                current: review.base_commit.clone(),
            });
        }
        if record.review_range != review.range {
            return rerun(RerunReason::Mismatch {
                field: "review_range",
                recorded: record.review_range.clone(),
                current: review.range.clone(),
            });
        }
    }
    if record.automation_blob_oid != current.automation_blob_oid {
        return rerun(RerunReason::Mismatch {
            field: "automation_blob_oid",
            recorded: record.automation_blob_oid.clone(),
            current: current.automation_blob_oid.clone(),
        });
    }
    if record.tool.path != current.tool.path {
        return rerun(RerunReason::Mismatch {
            field: "tool_path",
            recorded: record.tool.path.clone(),
            current: current.tool.path.clone(),
        });
    }
    if record.tool.executable_digest != current.tool.executable_digest {
        return rerun(RerunReason::Mismatch {
            field: "tool_digest",
            recorded: record.tool.executable_digest.clone(),
            current: current.tool.executable_digest.clone(),
        });
    }
    if record.tool.version != current.tool.version {
        return rerun(RerunReason::Mismatch {
            field: "tool_version",
            recorded: record.tool.version.clone().unwrap_or_default(),
            current: current.tool.version.clone().unwrap_or_default(),
        });
    }
    match current.artifact_digest.as_deref() {
        None => {
            return rerun(RerunReason::Defect(EvidenceDefect::Unreadable(format!(
                "artifact '{}' is absent or unreadable",
                record.artifact_path
            ))))
        }
        Some(digest) if digest != record.artifact_digest => {
            return rerun(RerunReason::Mismatch {
                field: "artifact_digest",
                recorded: record.artifact_digest.clone(),
                current: digest.to_string(),
            })
        }
        Some(_) => {}
    }
    if !current.clean {
        return rerun(RerunReason::DirtyWorktree(Vec::new()));
    }
    let elapsed = record.elapsed_seconds().unwrap_or_default();
    if elapsed < policy.min_reuse_seconds {
        return rerun(RerunReason::BelowDurationThreshold {
            elapsed,
            threshold: policy.min_reuse_seconds,
        });
    }
    ReuseDecision::Reuse {
        verification_id: id,
        evidence: Box::new(record.clone()),
    }
}

/// Every porcelain v1 entry, with no Conflux-owned path excluded.
///
/// Constitutional law 4 is explicit that a cleanliness exception is not a way to
/// make a Conflux-owned write permissible, so there is no exception here: a
/// staged, unstaged, or untracked entry is dirty whatever its path. Nothing this
/// runtime writes lands in a target any more, so the exclusion the predecessor
/// needed — writing the sidecar must not invalidate the sidecar — has nothing
/// left to protect.
///
/// Ignored paths are outside this function's remit entirely: porcelain status is
/// asked the ordinary tracked/untracked question, and the one legacy path that
/// still matters is probed separately by [`legacy_target_evidence`].
pub fn dirty_entries(porcelain_status: &str) -> Vec<String> {
    porcelain_status
        .lines()
        .filter(|line| line.len() >= 4)
        .map(|line| line.trim().to_string())
        .collect()
}

/// Repository observations the executor and validator both need.
///
/// Injected so tests can drive every branch with in-memory doubles; production
/// binds the Git-backed implementation.
#[async_trait]
pub trait RepositoryFacts: Send + Sync {
    /// Full-length object ID of `HEAD`.
    async fn head_commit(&self, workspace: &Path) -> Result<String, String>;
    /// Full-length commit object ID a revision name resolves to.
    ///
    /// Defaulted to a refusal rather than to `HEAD`: a fact source that cannot
    /// resolve a base must leave the review range empty, and silently answering
    /// with the candidate would bind evidence to a comparison nobody made.
    async fn resolve_revision(&self, workspace: &Path, revision: &str) -> Result<String, String> {
        let _ = (workspace, revision);
        Err(format!("revision '{revision}' cannot be resolved here"))
    }
    /// Full-length tree ID of `HEAD`.
    async fn head_tree(&self, workspace: &Path) -> Result<String, String>;
    /// Full-length blob ID of a tracked path at `HEAD`.
    async fn tracked_blob_oid(&self, workspace: &Path, path: &str) -> Result<String, String>;
    /// Full-length content digest of an arbitrary file.
    async fn hash_file(&self, workspace: &Path, path: &Path) -> Result<String, String>;
    /// Raw porcelain v1 status, untracked files included.
    async fn porcelain_status(&self, workspace: &Path) -> Result<String, String>;
    /// Resolve `program` to an executable identity.
    async fn resolve_tool(&self, workspace: &Path, program: &str) -> Result<ToolIdentity, String>;
}

/// One directly supervised process run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisedOutcome {
    pub exit_code: i32,
    /// Merged stdout and stderr, already bounded by the supervisor.
    pub output: Vec<u8>,
}

/// Seconds a supervised gate is given to exit cooperatively after its own
/// deadline expired, before the owned process group is force-killed.
///
/// Deliberately short: the deadline that just expired was the whole budget, and
/// what remains is the finalization reserve termination, reap, persistence, and
/// projection all have to fit inside.
const GATE_TERMINATION_GRACE_MS: u64 = 200;

/// Total budget for *proving* a terminated gate's process group is quiescent.
const GATE_CLEANUP_TIMEOUT_MS: u64 = 5_000;

/// What one supervised run produced, including the supervisor's own bound.
///
/// The second variant is why this type exists at all. A bound enforced by
/// cancelling the caller's future cannot terminate anything: dropping a future
/// that is awaiting a child leaves that child running in the worktree, and the
/// hold written afterwards would claim a cleanup nobody performed. Enforcing the
/// bound *inside* the supervisor is what makes "the gate is gone" a fact the
/// outcome can carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisedRun {
    /// The command ran to completion inside its budget.
    Completed(SupervisedOutcome),
    /// The budget expired. The child and its owned process group were
    /// terminated; the report is the proof — or the explicit failure to prove —
    /// that nothing survived.
    DeadlineExhausted(Box<crate::process_manager::ProcessGroupCleanupReport>),
}

/// The runtime's own process supervision.
#[async_trait]
pub trait CommandSupervisor: Send + Sync {
    async fn run(&self, argv: &[String], cwd: &Path) -> Result<SupervisedOutcome, String>;

    /// Run under a finite budget the supervisor itself enforces.
    ///
    /// The default is for supervisors that own no OS process — in-memory
    /// doubles, and anything whose `run` starts nothing. For those, cancelling
    /// the future really is the whole termination, so the cleanup evidence is
    /// `not_applicable` rather than a claim about a process that never existed.
    /// A supervisor that *does* spawn must override this: see
    /// [`DirectCommandSupervisor`].
    async fn run_bounded(
        &self,
        argv: &[String],
        cwd: &Path,
        budget: Option<Duration>,
    ) -> Result<SupervisedRun, String> {
        let Some(budget) = budget else {
            return self.run(argv, cwd).await.map(SupervisedRun::Completed);
        };
        match tokio::time::timeout(budget, self.run(argv, cwd)).await {
            Ok(result) => result.map(SupervisedRun::Completed),
            Err(_) => Ok(SupervisedRun::DeadlineExhausted(Box::new(
                crate::process_manager::ProcessGroupCleanupReport::not_applicable(
                    "this supervisor owns no OS process, so cancelling its run is the whole \
                     termination",
                ),
            ))),
        }
    }
}

/// Wall-clock source, injected so capture timestamps are deterministic in tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Production clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Why a supervised execution produced no reusable envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureRefusal {
    pub verification_id: String,
    pub code: &'static str,
    pub detail: String,
}

impl CaptureRefusal {
    fn new(verification_id: &str, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            verification_id: verification_id.to_string(),
            code,
            detail: detail.into(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "verification_id": self.verification_id,
            "outcome": "not_captured",
            "reason": self.code,
            "detail": self.detail,
        })
    }
}

/// What one supervised execution produced.
///
/// Two artifact paths travel together on purpose. The envelope's
/// `artifact_path` is store-relative because the *record* has to survive a
/// state root that moves; a report is read once, by an operator standing
/// somewhere the store root is not, so it also carries `artifact_location` —
/// the absolute path of the same file. Reporting only the store-relative form
/// names a location that resolves to nothing from the caller's directory, which
/// is the one thing a capture report exists to avoid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The command succeeded and every binding held; the envelope is on disk.
    Captured {
        evidence: Box<VerificationEvidence>,
        /// Absolute path of the captured artifact inside the external store.
        artifact_location: PathBuf,
    },
    /// The command ran and failed. No envelope: a failure is not reusable, and
    /// it is also not this module's verdict to report.
    CommandFailed {
        verification_id: String,
        exit_code: i32,
        /// Store-relative artifact path, the same spelling the envelope uses.
        artifact_path: String,
        /// Absolute path of that artifact, so the failing output is openable.
        artifact_location: PathBuf,
    },
    /// Nothing was captured, and the reason is not a command failure.
    Refused(CaptureRefusal),
    /// The supervisor's own budget expired. The command was terminated and its
    /// owned process group swept; the report says whether that is *proven*.
    ///
    /// Distinct from [`Self::Refused`] because it is neither a verdict nor an
    /// environment defect: the command was perfectly runnable, and what ended it
    /// was the shared Acceptance budget.
    DeadlineExhausted {
        verification_id: String,
        cleanup: Box<crate::process_manager::ProcessGroupCleanupReport>,
    },
}

impl CaptureOutcome {
    /// Absolute path of the artifact this outcome produced, when it produced
    /// one.
    ///
    /// A refusal and an exhausted deadline have no artifact to name: the first
    /// never wrote one, and the second reports a sweep rather than output.
    pub fn artifact_location(&self) -> Option<&Path> {
        match self {
            Self::Captured {
                artifact_location, ..
            }
            | Self::CommandFailed {
                artifact_location, ..
            } => Some(artifact_location.as_path()),
            Self::Refused(_) | Self::DeadlineExhausted { .. } => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Captured {
                evidence,
                artifact_location,
            } => serde_json::json!({
                "verification_id": evidence.verification_id,
                "outcome": "captured",
                "evidence": evidence.to_json(),
                "artifact_location": artifact_location.display().to_string(),
            }),
            Self::CommandFailed {
                verification_id,
                exit_code,
                artifact_path,
                artifact_location,
            } => serde_json::json!({
                "verification_id": verification_id,
                "outcome": "command_failed",
                "exit_code": exit_code,
                "artifact_path": artifact_path,
                "artifact_location": artifact_location.display().to_string(),
            }),
            Self::Refused(refusal) => refusal.to_json(),
            Self::DeadlineExhausted {
                verification_id,
                cleanup,
            } => serde_json::json!({
                "verification_id": verification_id,
                "outcome": "deadline_exhausted",
                "cleanup_confirmed": cleanup.is_confirmed(),
                "cleanup_diagnostics": cleanup.diagnostics(),
            }),
        }
    }
}

/// Conflux-owned external evidence storage for one change.
///
/// The root is handed in rather than derived: this type has no way to build a
/// path from a workspace, which is what makes "Acceptance never writes into a
/// target" a property of the type rather than a rule callers have to remember.
/// The caller resolves the directory through
/// [`crate::config::defaults::acceptance_store_path`], which refuses any root
/// inside the target repository or a managed worktree before this store exists
/// at all.
///
/// The directory is runtime-owned: `0700`, holding files written `0400`. Reads
/// apply the same ownership test they would apply to any untrusted input,
/// because any process under this UID could have written the bytes.
#[derive(Debug, Clone)]
pub struct EvidenceStore {
    root: PathBuf,
}

impl EvidenceStore {
    /// Bind the store to one already-resolved external directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The external store root this instance owns.
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Absolute path of the gate directory holding envelopes and artifacts.
    pub fn directory(&self) -> PathBuf {
        self.root.join(GATES_DIR)
    }

    /// Store-relative envelope path for one verification.
    pub fn envelope_relative_path(verification_id: &str) -> String {
        format!("{GATES_DIR}/{verification_id}.json")
    }

    /// Store-relative artifact path for one verification.
    pub fn artifact_relative_path(verification_id: &str) -> String {
        format!("{GATES_DIR}/{verification_id}.log")
    }

    /// Absolute path of one verification's captured artifact.
    pub fn artifact_path(&self, verification_id: &str) -> PathBuf {
        self.root
            .join(Self::artifact_relative_path(verification_id))
    }

    /// Create the runtime-owned external directory.
    ///
    /// No ignore file is written. There is nothing to hide from Git out here,
    /// and constitutional law 4 is explicit that hiding a Conflux-owned artifact
    /// was never what made one permissible.
    pub fn ensure_directory(&self) -> std::io::Result<()> {
        let directory = self.directory();
        std::fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700));
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    /// Read and structurally validate one envelope.
    ///
    /// The file mode is part of the authority test on Unix: an envelope the
    /// runtime wrote is `0400`, so anything writable is reported as
    /// agent-authored rather than parsed into a candidate.
    pub fn load(&self, verification_id: &str) -> Result<VerificationEvidence, EvidenceDefect> {
        if !is_storable_verification_id(verification_id) {
            return Err(EvidenceDefect::Malformed(format!(
                "verification id '{verification_id}' is not storable"
            )));
        }
        let path = self
            .root
            .join(Self::envelope_relative_path(verification_id));
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(EvidenceDefect::Missing)
            }
            Err(error) => return Err(EvidenceDefect::Unreadable(error.to_string())),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o777;
            if mode != 0o400 {
                return Err(EvidenceDefect::AgentAuthored(format!(
                    "envelope mode {mode:04o} is not the runtime-written 0400"
                )));
            }
        }
        #[cfg(not(unix))]
        let _ = metadata;
        let bytes =
            std::fs::read(&path).map_err(|error| EvidenceDefect::Unreadable(error.to_string()))?;
        parse_evidence(&bytes)
    }

    /// Atomically replace the envelope with a read-only runtime-written file.
    ///
    /// Atomic replacement is what keeps a partially written record from ever
    /// becoming a candidate: a reader sees the previous complete file or the new
    /// complete file, never a prefix of either.
    pub fn store(&self, evidence: &VerificationEvidence) -> std::io::Result<()> {
        self.ensure_directory()?;
        let directory = self.directory();
        let final_path = self
            .root
            .join(Self::envelope_relative_path(&evidence.verification_id));
        let temporary = directory.join(format!(
            ".{}.tmp-{}",
            evidence.verification_id,
            std::process::id()
        ));
        let serialized = serde_json::to_vec_pretty(evidence)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(&temporary, &serialized)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o400))?;
        }
        // A previous 0400 envelope is not writable, so replacement removes it
        // rather than truncating it.
        let _ = std::fs::remove_file(&final_path);
        match std::fs::rename(&temporary, &final_path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    /// Write the captured output artifact next to its envelope.
    fn store_artifact(&self, verification_id: &str, output: &[u8]) -> std::io::Result<PathBuf> {
        self.ensure_directory()?;
        let path = self.artifact_path(verification_id);
        let bounded = if output.len() > MAX_ARTIFACT_BYTES {
            &output[output.len() - MAX_ARTIFACT_BYTES..]
        } else {
            output
        };
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, bounded)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))?;
        }
        Ok(path)
    }
}

/// The only writer of reusable envelopes.
///
/// It starts the declared argv itself and derives every recorded field from what
/// it observed: the exit status from the child it waited on, the artifact digest
/// from the bytes it wrote, the identities from Git. No field is ever taken from
/// an agent's report, which is what makes the envelope evidence rather than a
/// restatement of a claim.
pub struct RuntimeVerificationExecutor<F, S, C> {
    workspace: PathBuf,
    /// Where the envelope and artifact are written. Always outside `workspace`.
    store: EvidenceStore,
    facts: F,
    supervisor: S,
    clock: C,
    review: Option<ReviewBinding>,
}

/// The review this executor's captures are evidence *for*.
///
/// Carried rather than recomputed: the manifest already resolved the base once
/// and froze it, and re-resolving here could bind a capture to a base branch
/// that moved between the two reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewBinding {
    /// Full-length review base commit, or empty when none resolved.
    pub base_commit: String,
    /// Exact `base..candidate` range.
    pub range: String,
}

impl<F, S, C> RuntimeVerificationExecutor<F, S, C>
where
    F: RepositoryFacts,
    S: CommandSupervisor,
    C: Clock,
{
    /// Bind an executor to the target it runs in and the external store it
    /// writes to.
    ///
    /// The two are separate parameters because they are separate trees, and
    /// keeping them separate is the point: the command's working directory is
    /// the target, and every byte this executor produces lands in `store`.
    pub fn new(
        workspace: impl Into<PathBuf>,
        store: EvidenceStore,
        facts: F,
        supervisor: S,
        clock: C,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            store,
            facts,
            supervisor,
            clock,
            review: None,
        }
    }

    /// The external store this executor writes to.
    pub fn store(&self) -> &EvidenceStore {
        &self.store
    }

    /// Bind every capture this executor writes to one review base and range.
    ///
    /// Unbound by default so a caller with no review context — `openspec
    /// verify`, a unit test — records no base rather than a fabricated one. An
    /// envelope with no base is still valid evidence; it is simply not reusable
    /// where a real review base *is* bound.
    pub fn with_review_binding(mut self, review: ReviewBinding) -> Self {
        self.review = Some(review);
        self
    }

    /// Observe every binding the envelope will carry.
    async fn snapshot(&self, request: &VerificationRequest) -> Result<CurrentBindings, String> {
        let workspace = self.workspace.as_path();
        let commit_oid = self.facts.head_commit(workspace).await?;
        let tree_oid = self.facts.head_tree(workspace).await?;
        let automation_blob_oid = self
            .facts
            .tracked_blob_oid(workspace, &request.automation_path)
            .await?;
        let tool = self
            .facts
            .resolve_tool(workspace, request.argv.first().map_or("", String::as_str))
            .await?;
        let status = self.facts.porcelain_status(workspace).await?;
        Ok(CurrentBindings {
            commit_oid,
            tree_oid,
            review: self.review.clone(),
            automation_blob_oid,
            tool,
            clean: dirty_entries(&status).is_empty(),
            artifact_digest: None,
        })
    }

    /// Run one declared verification under direct supervision and, on success,
    /// write the bound envelope.
    ///
    /// The post-execution snapshot is not a formality: a command that rewrites
    /// tracked files, or a commit that lands while it runs, invalidates the
    /// binding the record would otherwise claim. Capture is refused in that case
    /// rather than recorded with the pre-execution values.
    pub async fn capture(&self, request: &VerificationRequest) -> CaptureOutcome {
        self.capture_bounded(request, None).await
    }

    /// [`Self::capture`] under a finite budget the *supervisor* enforces.
    ///
    /// The budget is pushed all the way down rather than wrapped around this
    /// call on purpose. A `timeout` around the capture future would drop it
    /// mid-execution, leaving the declared command running in the worktree with
    /// nobody left to reap it — and the hold written afterwards would claim a
    /// cleanup that never happened. Handing the bound to the supervisor is what
    /// lets the expiry carry real termination evidence.
    pub async fn capture_bounded(
        &self,
        request: &VerificationRequest,
        budget: Option<Duration>,
    ) -> CaptureOutcome {
        let id = request.verification_id.as_str();
        // The external store is proven usable *before* the command starts. A
        // gate that runs and then cannot record anything has spent the shared
        // budget for nothing, and — since there is no target-local fallback —
        // failing here is the whole refusal rather than the first half of one.
        let store = &self.store;
        if let Err(error) = store.ensure_directory() {
            return CaptureOutcome::Refused(CaptureRefusal::new(
                id,
                "evidence_store_unavailable",
                error.to_string(),
            ));
        }
        let before = match self.snapshot(request).await {
            Ok(before) => before,
            Err(error) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "state_unobservable",
                    error,
                ))
            }
        };
        if !before.clean {
            return CaptureOutcome::Refused(CaptureRefusal::new(
                id,
                "worktree_dirty",
                "the target worktree is dirty before execution".to_string(),
            ));
        }

        let cwd = if request.cwd == "." {
            self.workspace.clone()
        } else {
            self.workspace.join(&request.cwd)
        };
        let started_at = self.clock.now();
        let outcome = match self
            .supervisor
            .run_bounded(&request.argv, &cwd, budget)
            .await
        {
            Ok(SupervisedRun::Completed(outcome)) => outcome,
            // The command was terminated and its group swept before this
            // returned, so the caller can report the deadline *and* whether the
            // worktree is provably free of it.
            Ok(SupervisedRun::DeadlineExhausted(cleanup)) => {
                return CaptureOutcome::DeadlineExhausted {
                    verification_id: id.to_string(),
                    cleanup,
                }
            }
            Err(error) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "supervision_failed",
                    error,
                ))
            }
        };
        let ended_at = self.clock.now();

        let artifact_path = match store.store_artifact(id, &outcome.output) {
            Ok(path) => path,
            Err(error) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "artifact_unwritable",
                    error.to_string(),
                ))
            }
        };
        if outcome.exit_code != 0 {
            return CaptureOutcome::CommandFailed {
                verification_id: id.to_string(),
                exit_code: outcome.exit_code,
                artifact_path: EvidenceStore::artifact_relative_path(id),
                // The path `store_artifact` just wrote, not a path rebuilt from
                // a root the reporter would have to guess at.
                artifact_location: artifact_path,
            };
        }

        let artifact_digest = match self
            .facts
            .hash_file(self.workspace.as_path(), artifact_path.as_path())
            .await
        {
            Ok(digest) if is_full_length_object_id(&digest) => digest,
            Ok(digest) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "artifact_digest_invalid",
                    format!("'{digest}' is not a full-length object id"),
                ))
            }
            Err(error) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "artifact_digest_unavailable",
                    error,
                ))
            }
        };

        let after = match self.snapshot(request).await {
            Ok(after) => after,
            Err(error) => {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "state_unobservable",
                    error,
                ))
            }
        };
        if !after.clean {
            return CaptureOutcome::Refused(CaptureRefusal::new(
                id,
                "worktree_dirty",
                "the target worktree became dirty while the command ran".to_string(),
            ));
        }
        for (field, recorded, current) in [
            ("commit_oid", &before.commit_oid, &after.commit_oid),
            ("tree_oid", &before.tree_oid, &after.tree_oid),
            (
                "automation_blob_oid",
                &before.automation_blob_oid,
                &after.automation_blob_oid,
            ),
            (
                "tool_digest",
                &before.tool.executable_digest,
                &after.tool.executable_digest,
            ),
        ] {
            if recorded != current {
                return CaptureOutcome::Refused(CaptureRefusal::new(
                    id,
                    "binding_moved_during_execution",
                    format!(
                        "{field} changed from '{recorded}' to '{current}' while the command ran"
                    ),
                ));
            }
        }

        let evidence = VerificationEvidence {
            schema: EVIDENCE_SCHEMA.to_string(),
            authority: EVIDENCE_AUTHORITY.to_string(),
            verification_id: id.to_string(),
            commit_oid: before.commit_oid.clone(),
            tree_oid: before.tree_oid.clone(),
            review_base_commit: before
                .review
                .as_ref()
                .map(|review| review.base_commit.clone())
                .unwrap_or_default(),
            review_range: before
                .review
                .as_ref()
                .map(|review| review.range.clone())
                .unwrap_or_default(),
            argv: request.argv.clone(),
            cwd: request.cwd.clone(),
            automation_path: request.automation_path.clone(),
            automation_blob_oid: before.automation_blob_oid.clone(),
            tool: before.tool.clone(),
            started_at,
            ended_at,
            exit_code: outcome.exit_code,
            artifact_path: EvidenceStore::artifact_relative_path(id),
            artifact_digest,
            clean_before: true,
            clean_after: true,
        };
        if let Err(error) = self.store.store(&evidence) {
            return CaptureOutcome::Refused(CaptureRefusal::new(
                id,
                "evidence_unwritable",
                error.to_string(),
            ));
        }
        CaptureOutcome::Captured {
            evidence: Box::new(evidence),
            artifact_location: artifact_path,
        }
    }
}

/// Decide reuse for one declared verification.
///
/// Repository state is the only authority: `store` supplies a *candidate*
/// record, and every binding on it is then compared against what Git says right
/// now. A store that holds nothing, or holds something that no longer matches,
/// decides `Rerun` — never PASS and never "skip this gate".
pub async fn decide_reuse<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    store: &EvidenceStore,
    declaration: &VerificationDeclaration,
    policy: ReusePolicy,
) -> Option<ReuseDecision> {
    let declared_id = declaration
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    // Only repository-local declarations are candidates at all; a deployed or
    // credentialed observation is not something Acceptance runs locally, so it
    // has no reuse decision to report.
    if !declaration
        .execution_class
        .as_deref()
        .and_then(VerificationExecutionClass::parse)
        .is_some_and(VerificationExecutionClass::is_repository_local)
    {
        return None;
    }
    let request = match eligible_request(declaration) {
        Ok(request) => request,
        Err(reason) => {
            return Some(ReuseDecision::Rerun {
                verification_id: declared_id.to_string(),
                reason: RerunReason::Ineligible(reason),
            })
        }
    };
    let record = match store.load(&request.verification_id) {
        Ok(record) => record,
        Err(defect) => {
            return Some(ReuseDecision::Rerun {
                verification_id: request.verification_id,
                reason: RerunReason::Defect(defect),
            })
        }
    };

    let unobservable = |error: String| {
        Some(ReuseDecision::Rerun {
            verification_id: request.verification_id.clone(),
            reason: RerunReason::Unobservable(error),
        })
    };
    let commit_oid = match facts.head_commit(workspace).await {
        Ok(value) => value,
        Err(error) => return unobservable(error),
    };
    let tree_oid = match facts.head_tree(workspace).await {
        Ok(value) => value,
        Err(error) => return unobservable(error),
    };
    let automation_blob_oid = match facts
        .tracked_blob_oid(workspace, &request.automation_path)
        .await
    {
        Ok(value) => value,
        Err(error) => return unobservable(error),
    };
    let tool = match facts
        .resolve_tool(workspace, request.argv.first().map_or("", String::as_str))
        .await
    {
        Ok(value) => value,
        Err(error) => return unobservable(error),
    };
    let status = match facts.porcelain_status(workspace).await {
        Ok(value) => value,
        Err(error) => return unobservable(error),
    };
    let dirty = dirty_entries(&status);
    if !dirty.is_empty() {
        return Some(ReuseDecision::Rerun {
            verification_id: request.verification_id,
            reason: RerunReason::DirtyWorktree(dirty),
        });
    }
    // Hashed from the external store, so deleting the cache makes the artifact
    // unreadable and forces the rerun — the one behaviour cache loss is allowed
    // to cause.
    let artifact_digest = facts
        .hash_file(workspace, &store.artifact_path(&request.verification_id))
        .await
        .ok()
        .filter(|digest| is_full_length_object_id(digest));

    let current = CurrentBindings {
        commit_oid,
        tree_oid,
        // This path has no manifest and therefore no review to bind. It asks
        // "does this evidence still describe the current worktree", not "does it
        // describe the current review", so it does not compare a review binding
        // it never resolved.
        review: None,
        automation_blob_oid,
        tool,
        clean: true,
        artifact_digest,
    };
    Some(evaluate_reuse(&request, &record, &current, policy))
}

/// Per-verification reuse plan for one change's declarations.
pub async fn plan_reuse<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    store: &EvidenceStore,
    declarations: &[VerificationDeclaration],
    policy: ReusePolicy,
) -> VerificationReusePlan {
    let mut decisions = Vec::new();
    let mut seen = BTreeMap::new();
    for declaration in declarations {
        if let Some(decision) = decide_reuse(facts, workspace, store, declaration, policy).await {
            // A duplicate declared ID is a proposal defect the validator already
            // reports; here the first decision stands so the plan never lists one
            // ID twice with two outcomes.
            if seen
                .insert(decision.verification_id().to_string(), ())
                .is_none()
            {
                decisions.push(decision);
            }
        }
    }
    VerificationReusePlan { decisions }
}

/// Git-backed repository facts.
pub struct GitRepositoryFacts;

#[async_trait]
impl RepositoryFacts for GitRepositoryFacts {
    async fn head_commit(&self, workspace: &Path) -> Result<String, String> {
        run_git(workspace, &["rev-parse", "HEAD"]).await
    }

    async fn head_tree(&self, workspace: &Path) -> Result<String, String> {
        run_git(workspace, &["rev-parse", "HEAD^{tree}"]).await
    }

    async fn resolve_revision(&self, workspace: &Path, revision: &str) -> Result<String, String> {
        // `^{commit}` is what refuses a tag or tree that would otherwise resolve
        // to something a `base..candidate` range cannot mean.
        run_git(workspace, &["rev-parse", &format!("{revision}^{{commit}}")]).await
    }

    async fn tracked_blob_oid(&self, workspace: &Path, path: &str) -> Result<String, String> {
        run_git(workspace, &["rev-parse", &format!("HEAD:{path}")]).await
    }

    async fn hash_file(&self, workspace: &Path, path: &Path) -> Result<String, String> {
        let path = path.to_string_lossy().to_string();
        run_git(workspace, &["hash-object", "-t", "blob", "--", &path]).await
    }

    async fn porcelain_status(&self, workspace: &Path) -> Result<String, String> {
        let argv = crate::vcs::git::commands::status_policy::read_only_status_argv(
            crate::vcs::git::commands::status_policy::PORCELAIN_UNTRACKED_STATUS_ARGS,
        );
        run_git_raw(workspace, &argv).await
    }

    async fn resolve_tool(&self, workspace: &Path, program: &str) -> Result<ToolIdentity, String> {
        if program.trim().is_empty() {
            return Err("verification argv has no program".to_string());
        }
        let resolved = resolve_executable(program)
            .ok_or_else(|| format!("executable '{program}' is not on PATH"))?;
        let executable_digest = self.hash_file(workspace, resolved.as_path()).await?;
        // `--version` is best-effort: a tool that does not report one is bound by
        // its file digest alone, which is the stronger identity anyway.
        let version = tokio::process::Command::new(&resolved)
            .arg(VERSION_FLAG)
            .current_dir(workspace)
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| usable_tool_version(&String::from_utf8_lossy(&output.stdout)));
        Ok(ToolIdentity {
            path: resolved.to_string_lossy().to_string(),
            executable_digest,
            version,
        })
    }
}

/// The flag asked of every resolved executable.
const VERSION_FLAG: &str = "--version";

/// Keep a version string only when the tool actually reported one.
///
/// `/bin/echo --version` prints `--version`, and a tool that ignores the flag
/// entirely prints nothing. Recording either as a "version" would put a value in
/// the envelope that reads like an identity and is not one. The executable
/// digest is already the strong binding, so the honest answer here is `None`.
fn usable_tool_version(raw: &str) -> Option<String> {
    let version = raw.trim();
    (!version.is_empty() && version != VERSION_FLAG).then(|| version.to_string())
}

/// Locate `program` the way process spawning will.
fn resolve_executable(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 {
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(candidate)
        };
        return absolute.is_file().then_some(absolute);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

async fn run_git(workspace: &Path, args: &[&str]) -> Result<String, String> {
    run_git_raw(workspace, args).await.map(|out| {
        out.lines()
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    })
}

async fn run_git_raw(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(workspace)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|error| format!("git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Directly supervised child process with merged, bounded output.
///
/// The gate is started in its own session, so the declared command and every
/// descendant it spawns belong to one owned process group. That is what makes
/// termination provable: killing a shell says nothing about the `sleep` it
/// `exec`ed or forked, and a gate whose deadline expires must leave nothing
/// running in the worktree the next phase is about to read.
pub struct DirectCommandSupervisor;

#[async_trait]
impl CommandSupervisor for DirectCommandSupervisor {
    async fn run(&self, argv: &[String], cwd: &Path) -> Result<SupervisedOutcome, String> {
        match self.run_bounded(argv, cwd, None).await? {
            SupervisedRun::Completed(outcome) => Ok(outcome),
            // Unreachable: an unbounded run has no deadline to exhaust. Reported
            // as a supervision failure rather than panicked on, because a
            // refusal is recoverable and a panic in a gate is not.
            SupervisedRun::DeadlineExhausted(report) => Err(format!(
                "unbounded supervision reported a deadline: {}",
                report.diagnostics()
            )),
        }
    }

    async fn run_bounded(
        &self,
        argv: &[String],
        cwd: &Path,
        budget: Option<Duration>,
    ) -> Result<SupervisedRun, String> {
        use tokio::io::AsyncReadExt;

        let (program, arguments) = argv
            .split_first()
            .ok_or_else(|| "verification argv is empty".to_string())?;
        let mut command = tokio::process::Command::new(program);
        command
            .args(arguments)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            // Belt and braces for every path that is *not* the deadline below:
            // a panic, an early return, or a caller that drops this future must
            // not leave the gate running either.
            .kill_on_drop(true);
        crate::process_manager::configure_process_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| format!("{program}: {error}"))?;
        // `setsid` makes the child its own session and group leader, so its PID
        // is the PGID every descendant inherits.
        let pgid = child.id().unwrap_or(0);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        // Drained on their own tasks so a chatty gate can never fill a pipe and
        // wedge the wait this deadline is supposed to bound.
        let stdout_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            if let Some(mut stream) = stdout {
                let _ = stream.read_to_end(&mut buffer).await;
            }
            buffer
        });
        let stderr_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            if let Some(mut stream) = stderr {
                let _ = stream.read_to_end(&mut buffer).await;
            }
            buffer
        });

        let status = match budget {
            None => child.wait().await,
            Some(budget) => match tokio::time::timeout(budget, child.wait()).await {
                Ok(status) => status,
                Err(_) => {
                    // Order matters. The leader is killed and *reaped* first: an
                    // unreaped zombie is still a process-group member, so
                    // probing before reaping would report "members remain" for a
                    // group whose only occupant is already dead. Only then is
                    // the rest of the group swept and proven.
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    stdout_task.abort();
                    stderr_task.abort();
                    let report = crate::process_manager::cleanup_process_group_verified(
                        pgid,
                        GATE_TERMINATION_GRACE_MS,
                        GATE_CLEANUP_TIMEOUT_MS,
                        Some("acceptance-gate"),
                        None,
                    )
                    .await;
                    return Ok(SupervisedRun::DeadlineExhausted(Box::new(report)));
                }
            },
        };
        let status = status.map_err(|error| format!("{program}: {error}"))?;
        let mut merged = stdout_task.await.unwrap_or_default();
        merged.extend_from_slice(&stderr_task.await.unwrap_or_default());
        Ok(SupervisedRun::Completed(SupervisedOutcome {
            // A signal-terminated child reports no code; treating that as a
            // distinct non-zero value keeps it out of the success path.
            exit_code: status.code().unwrap_or(-1),
            output: merged,
        }))
    }
}

#[cfg(test)]
#[path = "verification_evidence/tests.rs"]
mod tests;
