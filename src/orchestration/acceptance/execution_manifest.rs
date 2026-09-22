//! The runtime-owned Acceptance execution manifest: what Acceptance is allowed
//! to run, what it is reviewing, and whether asking again could possibly answer
//! differently.
//!
//! A proposal's `verifications` block used to reach Acceptance as advice. The
//! reviewer read it, and then decided for itself what "verified" meant — which
//! in practice meant repository-wide suites, base-worktree comparisons, and
//! sibling-change investigation, none of which the change declared and any of
//! which can outlive the absolute runtime limit. A terminated review records no
//! canonical attempt, so the same unbounded exploration was admissible again
//! immediately.
//!
//! This module replaces that advice with a *boundary*. Three things follow from
//! it, and they are deliberately separable:
//!
//! * **The manifest is the blocking command allowlist.** Conflux selects every
//!   eligible `pre-integration` / `repository-local` / `change-blocking`
//!   declaration, normalizes it, and freezes it. A reviewer cannot enlarge the
//!   set, and a command outside it is never awaited, reused, or counted.
//! * **The fingerprint is the identity of the question.** SHA-256 over the
//!   candidate commit/tree, review base/range, canonical change inputs, skill
//!   and executable identity, and every normalized gate entry. Two invocations
//!   with the same fingerprint are the same question; re-asking it after the
//!   deadline already answered "no" is refused rather than redispatched.
//! * **Nothing lives outside the worktree.** The manifest is a repository-local
//!   sidecar inside the existing Git-excluded evidence directory, so deleting
//!   `~/.local/state/cflx/**` cannot change a retry decision — constitutional
//!   law 1 — and a restart re-derives the same fingerprint from the same files.
//!
//! What the fingerprint deliberately excludes is as load-bearing as what it
//! includes: timestamps, terminal output, gate durations, and the terminal
//! record itself are all absent, because a fingerprint that moved on its own
//! would turn "nothing changed" into an infinite retry loop.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::verification_evidence::{
    eligible_request, is_storable_verification_id, EvidenceStore, ToolIdentity,
    VerificationRequest, EVIDENCE_DIR,
};
use crate::openspec::VerificationDeclaration;

/// The only phase whose declarations may block a change before integration.
pub const PRE_INTEGRATION_PHASE: &str = "pre-integration";

/// Manifest schema. Any other value is a hard refusal, never a migration.
pub const MANIFEST_SCHEMA: &str = "conflux-acceptance-execution-manifest-v1";

/// Seconds reserved at the end of the effective Acceptance deadline for
/// termination, reap, persistence, and state projection.
///
/// The configured minimum effective deadline is 300s, so at least 270s always
/// remain for gates plus review.
pub const FINALIZATION_RESERVE_SECS: u64 = 30;

/// Stable operator-facing token for a refused unchanged retry.
pub const UNCHANGED_ACCEPTANCE_INPUT: &str = "unchanged_acceptance_input";

/// The fingerprint components whose repository-visible change restores retry
/// eligibility. Reported verbatim so a refusal names what to do about it.
pub const FINGERPRINT_COMPONENTS: &[&str] = &[
    "candidate_commit",
    "candidate_tree",
    "review_base",
    "review_range",
    "change_inputs",
    "skill_identity",
    "executable_identity",
    "declaration",
    "automation_blob",
    "command_identity",
    "tool_identity",
    "artifact_digest",
];

/// Repository-relative manifest path for one change.
pub fn manifest_relative_path(change_id: &str) -> String {
    format!("{EVIDENCE_DIR}/acceptance-manifest-{change_id}.json")
}

/// Whether a change ID is usable as a manifest file name.
///
/// Same rule the evidence sidecar applies to a verification ID: anything that
/// could escape the evidence directory or collide across changes is refused
/// rather than sanitized, because a sanitized name binds a manifest to a change
/// it does not describe.
pub fn is_storable_change_id(change_id: &str) -> bool {
    is_storable_verification_id(change_id)
}

// ============================================================================
// Terminal classification
// ============================================================================

/// Why a bounded Acceptance invocation ended without a verdict.
///
/// Every variant is a non-resumable execution hold with `status: stalled` and
/// blocker kind `none`. None of them is the generic runtime-limit `error`: the
/// outer limit remains a defect-containment fallback that ordinary bounded
/// Acceptance no longer reaches. And none of them is a verdict about the change:
/// a review that ran out of time says nothing about whether the implementation
/// is correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceHoldCategory {
    /// Declared gates could not finish inside the shared budget.
    DeclaredGateDeadlineExhausted,
    /// Semantic review had no complete verdict when finalization began.
    ReviewDeadlineExhausted,
    /// The reviewer was terminated but its process group was not proven gone.
    ReviewCleanupUnproven,
    /// A stored manifest could not be parsed or validated.
    ManifestMalformed,
    /// The runtime itself could not execute or observe what it needed to.
    RuntimeDefect,
}

impl AcceptanceHoldCategory {
    /// Stable machine-readable category string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredGateDeadlineExhausted => "acceptance_declared_gate_deadline_exhausted",
            Self::ReviewDeadlineExhausted => "acceptance_review_deadline_exhausted",
            Self::ReviewCleanupUnproven => "acceptance_review_cleanup_unproven",
            Self::ManifestMalformed => "acceptance_manifest_malformed",
            Self::RuntimeDefect => "acceptance_runtime_defect",
        }
    }

    /// Parse a stored category string.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "acceptance_declared_gate_deadline_exhausted" => {
                Some(Self::DeclaredGateDeadlineExhausted)
            }
            "acceptance_review_deadline_exhausted" => Some(Self::ReviewDeadlineExhausted),
            "acceptance_review_cleanup_unproven" => Some(Self::ReviewCleanupUnproven),
            "acceptance_manifest_malformed" => Some(Self::ManifestMalformed),
            "acceptance_runtime_defect" => Some(Self::RuntimeDefect),
            _ => None,
        }
    }

    /// Operator-facing explanation of what stopped the invocation.
    pub fn detail(self) -> &'static str {
        match self {
            Self::DeclaredGateDeadlineExhausted => {
                "declared change-blocking verification did not finish inside the shared \
                 Acceptance budget"
            }
            Self::ReviewDeadlineExhausted => {
                "semantic review produced no canonical verdict before the reserved finalization \
                 window began"
            }
            Self::ReviewCleanupUnproven => {
                "the reviewer was terminated but its owned process group was not proven quiescent"
            }
            Self::ManifestMalformed => {
                "the stored Acceptance execution manifest could not be parsed or validated"
            }
            Self::RuntimeDefect => {
                "the Conflux runtime could not execute or observe the declared Acceptance boundary"
            }
        }
    }
}

/// A non-resumable Acceptance execution hold, with the evidence that produced it.
///
/// `cleanup_confirmed` travels with the hold on purpose: "we stopped it" and
/// "it is actually gone" are separate facts, and an unproven process group is
/// reported rather than silently acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceExecutionHold {
    pub category: AcceptanceHoldCategory,
    /// The shared effective deadline, in seconds, this invocation was bounded by.
    pub budget_secs: u64,
    /// Whether the owned process group was proven quiescent after termination.
    pub cleanup_confirmed: bool,
    /// One-line process-group cleanup diagnostics.
    pub cleanup_diagnostics: String,
    /// Input fingerprint of the manifest this invocation ran under.
    pub fingerprint: String,
    /// Concrete evidence lines: gate outcomes, observed failures.
    pub evidence: Vec<String>,
}

impl AcceptanceExecutionHold {
    /// Another automatic attempt at the same unchanged input is never admissible.
    pub fn permits_retry(&self) -> bool {
        false
    }

    /// Actionable operator-facing summary used as the blocker's error text.
    pub fn summary(&self, change_id: &str) -> String {
        format!(
            "Acceptance for '{}' settled as a non-resumable execution hold ({}): {}. \
             Input fingerprint {}. This is not a PASS, not a FAIL, and not an external block. \
             A retry with unchanged repository input is refused as `{}`; change one of {} to \
             permit a new bounded attempt.",
            change_id,
            self.category.as_str(),
            self.category.detail(),
            &self.fingerprint[..16.min(self.fingerprint.len())],
            UNCHANGED_ACCEPTANCE_INPUT,
            FINGERPRINT_COMPONENTS.join(", "),
        )
    }

    /// Project the hold as the shared stalled-blocker payload.
    ///
    /// `unblock_condition` is deliberately `None` and `prerequisite_owner`
    /// deliberately absent: this is an execution stop, not a wait on a named
    /// external prerequisite, and supplying either would push the reducer's
    /// classifier onto the external `blocked` path it must not take.
    pub fn to_stalled_blocker(&self, change_id: &str) -> crate::events::StalledBlocker {
        let mut evidence = vec![format!("input_fingerprint={}", self.fingerprint)];
        evidence.extend(self.evidence.iter().cloned());
        evidence.push(format!(
            "process_group_cleanup={}",
            if self.cleanup_confirmed {
                "confirmed".to_string()
            } else {
                format!("unproven: {}", self.cleanup_diagnostics)
            }
        ));
        crate::events::StalledBlocker {
            category: self.category.as_str().to_string(),
            phase: "acceptance".to_string(),
            gate: "acceptance_execution_boundary".to_string(),
            error_summary: self.summary(change_id),
            evidence,
            unblock_condition: None,
            prerequisite_owner: None,
            next_action: format!(
                "Change one of {} in the repository, then retry explicitly. An unchanged retry is \
                 refused as `{}`.",
                FINGERPRINT_COMPONENTS.join(", "),
                UNCHANGED_ACCEPTANCE_INPUT
            ),
            resumable: false,
            worktree_preserved: true,
        }
    }
}

// ============================================================================
// Deadline partition
// ============================================================================

/// How the shared Acceptance budget is split between work and finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlinePartition {
    /// The effective absolute deadline, in seconds. `0` means disabled.
    pub absolute_secs: u64,
    /// Seconds available to declared gates plus semantic review.
    pub work_secs: u64,
    /// Seconds reserved for termination, reap, persistence, and projection.
    pub reserve_secs: u64,
}

impl DeadlinePartition {
    /// Whether an absolute bound exists at all.
    pub fn is_bounded(&self) -> bool {
        self.absolute_secs > 0
    }

    /// Work budget as a duration, or `None` when the limit is disabled.
    pub fn work_duration(&self) -> Option<std::time::Duration> {
        self.is_bounded()
            .then(|| std::time::Duration::from_secs(self.work_secs))
    }
}

/// Split the effective absolute Acceptance deadline into work and finalization.
///
/// A disabled limit (`0`) stays disabled: reserving a finalization window out of
/// an unbounded budget would invent a deadline the operator switched off. A
/// positive limit always keeps at least one second of work, because a partition
/// that leaves zero would turn every invocation into an immediate hold.
pub fn partition_deadline(absolute_secs: u64) -> DeadlinePartition {
    if absolute_secs == 0 {
        return DeadlinePartition {
            absolute_secs: 0,
            work_secs: 0,
            reserve_secs: 0,
        };
    }
    let reserve = FINALIZATION_RESERVE_SECS.min(absolute_secs.saturating_sub(1));
    DeadlinePartition {
        absolute_secs,
        work_secs: absolute_secs - reserve,
        reserve_secs: reserve,
    }
}

// ============================================================================
// Manifest
// ============================================================================

/// One normalized declared gate, frozen into the manifest.
///
/// Every field here is fingerprint-bearing: changing the argv, the automation
/// blob, the resolved tool, or the bound artifact is a repository-visible change
/// that makes the Acceptance question genuinely different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestGate {
    pub verification_id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub automation_path: String,
    /// Full-length blob ID of the automation file at the candidate commit, or
    /// empty when it could not be observed (which forces execution).
    #[serde(default)]
    pub automation_blob_oid: String,
    #[serde(default)]
    pub tool: ToolIdentity,
    /// Repository-relative artifact path the runtime binds results to.
    pub artifact_path: String,
    /// Digest of the currently stored artifact, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
}

impl ManifestGate {
    /// Canonical fingerprint projection: identity only, never a result.
    fn fingerprint_value(&self) -> serde_json::Value {
        serde_json::json!({
            "verification_id": self.verification_id,
            "argv": self.argv,
            "cwd": self.cwd,
            "automation_path": self.automation_path,
            "automation_blob_oid": self.automation_blob_oid,
            "tool_path": self.tool.path,
            "tool_digest": self.tool.executable_digest,
            "tool_version": self.tool.version,
            "artifact_path": self.artifact_path,
            "artifact_digest": self.artifact_digest,
        })
    }

    /// The supervisable request this gate represents.
    pub fn request(&self) -> VerificationRequest {
        VerificationRequest {
            verification_id: self.verification_id.clone(),
            argv: self.argv.clone(),
            cwd: self.cwd.clone(),
            automation_path: self.automation_path.clone(),
        }
    }
}

/// Identity of the embedded Acceptance skill the reviewer will load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillIdentity {
    /// Selected skill name, for example `cflx-accept`.
    pub name: String,
    /// Digest of the embedded skill text.
    pub digest: String,
}

/// What the runtime did with one declared gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GateOutcome {
    /// The command succeeded now, under runtime supervision.
    Executed {
        verification_id: String,
        artifact_path: String,
        elapsed_seconds: i64,
    },
    /// Exact bound evidence was still valid, so nothing ran.
    Reused {
        verification_id: String,
        artifact_path: String,
        elapsed_seconds: i64,
    },
    /// The command ran and failed. A blocking failure, and a `fail` verdict.
    Failed {
        verification_id: String,
        exit_code: i32,
        artifact_path: String,
    },
    /// The runtime could not run or observe the gate at all.
    Refused {
        verification_id: String,
        code: String,
        detail: String,
    },
    /// The shared budget ran out before this gate could finish.
    DeadlineExhausted { verification_id: String },
}

impl GateOutcome {
    pub fn verification_id(&self) -> &str {
        match self {
            Self::Executed {
                verification_id, ..
            }
            | Self::Reused {
                verification_id, ..
            }
            | Self::Failed {
                verification_id, ..
            }
            | Self::Refused {
                verification_id, ..
            }
            | Self::DeadlineExhausted { verification_id } => verification_id,
        }
    }

    /// Whether this gate proves its declared verification currently succeeds.
    pub fn is_successful(&self) -> bool {
        matches!(self, Self::Executed { .. } | Self::Reused { .. })
    }

    /// Whether this gate is a blocking failure of the change under review.
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Stable one-word status for prompt and operator output.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Executed { .. } => "executed",
            Self::Reused { .. } => "reused",
            Self::Failed { .. } => "failed",
            Self::Refused { .. } => "refused",
            Self::DeadlineExhausted { .. } => "deadline_exhausted",
        }
    }

    /// One operator-facing line.
    pub fn summary(&self) -> String {
        match self {
            Self::Executed {
                verification_id,
                artifact_path,
                elapsed_seconds,
            } => format!(
                "{verification_id}: executed and passed ({elapsed_seconds}s, artifact {artifact_path})"
            ),
            Self::Reused {
                verification_id,
                artifact_path,
                elapsed_seconds,
            } => format!(
                "{verification_id}: reused bound evidence ({elapsed_seconds}s, artifact {artifact_path})"
            ),
            Self::Failed {
                verification_id,
                exit_code,
                artifact_path,
            } => format!(
                "{verification_id}: FAILED with exit code {exit_code} (artifact {artifact_path})"
            ),
            Self::Refused {
                verification_id,
                code,
                detail,
            } => format!("{verification_id}: not executed ({code}) {detail}"),
            Self::DeadlineExhausted { verification_id } => {
                format!("{verification_id}: did not finish inside the shared Acceptance budget")
            }
        }
    }
}

/// Typed gate results for one Acceptance invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateResults {
    pub outcomes: Vec<GateOutcome>,
}

impl GateResults {
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// The first blocking failure, when one exists.
    pub fn first_failure(&self) -> Option<&GateOutcome> {
        self.outcomes.iter().find(|outcome| outcome.is_failure())
    }

    /// The first gate the shared budget cut short, when one exists.
    pub fn first_deadline_exhausted(&self) -> Option<&GateOutcome> {
        self.outcomes
            .iter()
            .find(|outcome| matches!(outcome, GateOutcome::DeadlineExhausted { .. }))
    }

    /// The first gate the runtime could not run at all, when one exists.
    pub fn first_refusal(&self) -> Option<&GateOutcome> {
        self.outcomes
            .iter()
            .find(|outcome| matches!(outcome, GateOutcome::Refused { .. }))
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "gates": self.outcomes,
            "all_successful": self.outcomes.iter().all(GateOutcome::is_successful),
        })
    }

    pub fn summary_lines(&self) -> Vec<String> {
        self.outcomes.iter().map(GateOutcome::summary).collect()
    }
}

/// The terminal record one invocation leaves behind.
///
/// Deliberately outside the fingerprint: recording that the deadline expired
/// must not itself make the next question different, or the refusal this whole
/// module exists for would never fire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestTerminal {
    /// The hold category, when the invocation settled as a non-resumable hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_category: Option<AcceptanceHoldCategory>,
    /// The fingerprint that was current when the terminal state was recorded.
    pub fingerprint: String,
    /// Whether the owned process group was proven quiescent.
    pub cleanup_confirmed: bool,
    /// One-line cleanup diagnostics.
    #[serde(default)]
    pub cleanup_diagnostics: String,
    pub recorded_at: DateTime<Utc>,
}

/// The frozen Acceptance execution manifest for one invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceExecutionManifest {
    pub schema: String,
    pub change_id: String,
    /// Full-length candidate commit object ID under review.
    pub candidate_commit_oid: String,
    /// Full-length tree ID of that commit.
    pub candidate_tree_oid: String,
    /// Full-length review base commit, or empty when no base is resolvable.
    #[serde(default)]
    pub review_base_commit: String,
    /// Exact `base..candidate` range string.
    #[serde(default)]
    pub review_range: String,
    /// The revision name the review base was resolved from.
    ///
    /// Deliberately outside the fingerprint: what identifies the review is the
    /// commit the base *resolved to*, not the name it was reached by. It is
    /// stored so a later recomputation can re-resolve the same ref — and so a
    /// base branch that has since moved produces a genuinely different
    /// fingerprint instead of an accidentally matching one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_base_ref: Option<String>,
    /// Digest over canonical proposal, task artifact, spec deltas, and constitution.
    pub change_input_digest: String,
    /// Selected embedded Acceptance skill identity.
    #[serde(default)]
    pub skill: SkillIdentity,
    /// Identity of the Conflux executable that owns this boundary.
    #[serde(default)]
    pub executable_identity: String,
    /// Every eligible declared change-blocking gate, ordered by verification ID.
    pub gates: Vec<ManifestGate>,
    pub created_at: DateTime<Utc>,
    /// Shared effective absolute deadline, in seconds, for gates plus review.
    #[serde(default)]
    pub absolute_deadline_secs: u64,
    /// Work budget after the finalization reserve is taken out.
    #[serde(default)]
    pub work_budget_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<ManifestTerminal>,
}

impl AcceptanceExecutionManifest {
    /// SHA-256 over canonical JSON of every field that can change review
    /// authority or gate validity.
    ///
    /// Timestamps, budgets, and the terminal record are excluded: a fingerprint
    /// that moved on its own would let an unchanged retry look changed. Gate
    /// ordering is deterministic by verification ID, which is enforced at
    /// construction and re-checked by [`parse_manifest`].
    pub fn fingerprint(&self) -> String {
        let canonical = serde_json::json!({
            "schema": self.schema,
            "change_id": self.change_id,
            "candidate_commit_oid": self.candidate_commit_oid,
            "candidate_tree_oid": self.candidate_tree_oid,
            "review_base_commit": self.review_base_commit,
            "review_range": self.review_range,
            "change_input_digest": self.change_input_digest,
            "skill_name": self.skill.name,
            "skill_digest": self.skill.digest,
            "executable_identity": self.executable_identity,
            "gates": self
                .gates
                .iter()
                .map(ManifestGate::fingerprint_value)
                .collect::<Vec<_>>(),
        });
        // `serde_json::Value` serializes maps in the order this literal declares,
        // and every nested value is built the same way, so the byte sequence is
        // canonical without a separate canonicalizer.
        let mut hasher = Sha256::new();
        hasher.update(canonical.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// The gates as supervisable requests, in manifest order.
    pub fn requests(&self) -> Vec<VerificationRequest> {
        self.gates.iter().map(ManifestGate::request).collect()
    }

    /// Whether this manifest declares no blocking gate at all.
    ///
    /// An empty allowlist is a legitimate boundary, not a defect: a change that
    /// declares no eligible repository-local change-blocking verification is
    /// reviewed semantically and nothing else. What it must never become is
    /// permission for the reviewer to pick its own gates.
    pub fn has_gates(&self) -> bool {
        !self.gates.is_empty()
    }

    /// Machine-readable projection handed to the semantic reviewer.
    ///
    /// Carries the frozen candidate, the exact review range, the fingerprint,
    /// and the declared allowlist — never a command the reviewer is expected to
    /// run, because the runtime already ran it.
    pub fn to_review_json(&self) -> serde_json::Value {
        serde_json::json!({
            "change_id": self.change_id,
            "candidate_commit": self.candidate_commit_oid,
            "candidate_tree": self.candidate_tree_oid,
            "review_base": self.review_base_commit,
            "review_range": self.review_range,
            "input_fingerprint": self.fingerprint(),
            "declared_blocking_verifications": self
                .gates
                .iter()
                .map(|gate| serde_json::json!({
                    "verification_id": gate.verification_id,
                    "command": gate.argv.join(" "),
                    "automation": gate.automation_path,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

/// Why a stored manifest could not be read as authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestDefect {
    Missing,
    Unreadable(String),
    Malformed(String),
    UnknownSchema(String),
}

impl ManifestDefect {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing => "manifest_missing",
            Self::Unreadable(_) => "manifest_unreadable",
            Self::Malformed(_) => "manifest_malformed",
            Self::UnknownSchema(_) => "manifest_unknown_schema",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Missing => "no runtime-authored Acceptance manifest exists".to_string(),
            Self::Unreadable(detail) => format!("manifest could not be read: {detail}"),
            Self::Malformed(detail) => format!("manifest is malformed: {detail}"),
            Self::UnknownSchema(found) => {
                format!("manifest schema '{found}' is not '{MANIFEST_SCHEMA}'")
            }
        }
    }
}

/// A full-length Git object ID or content digest.
fn is_full_length_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse and structurally validate a stored manifest.
///
/// Fail-closed in the same shape the evidence sidecar uses: a manifest that is
/// missing a binding, carries a short object ID, or lists its gates out of
/// canonical order is not a weaker manifest, it is not a manifest. The ordering
/// check matters because the fingerprint is order-sensitive: accepting a
/// reordered gate list would let the same declarations hash two different ways.
pub fn parse_manifest(bytes: &[u8]) -> Result<AcceptanceExecutionManifest, ManifestDefect> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| ManifestDefect::Malformed(error.to_string()))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if schema != MANIFEST_SCHEMA {
        return Err(ManifestDefect::UnknownSchema(schema));
    }
    let manifest: AcceptanceExecutionManifest = serde_json::from_value(value)
        .map_err(|error| ManifestDefect::Malformed(error.to_string()))?;

    if !is_storable_change_id(&manifest.change_id) {
        return Err(ManifestDefect::Malformed(format!(
            "change id '{}' is not storable",
            manifest.change_id
        )));
    }
    for (field, value) in [
        ("candidate_commit_oid", &manifest.candidate_commit_oid),
        ("candidate_tree_oid", &manifest.candidate_tree_oid),
        ("change_input_digest", &manifest.change_input_digest),
    ] {
        if !is_full_length_object_id(value) {
            return Err(ManifestDefect::Malformed(format!(
                "{field} '{value}' is not a full-length object id"
            )));
        }
    }
    if !manifest.review_base_commit.is_empty()
        && !is_full_length_object_id(&manifest.review_base_commit)
    {
        return Err(ManifestDefect::Malformed(format!(
            "review_base_commit '{}' is not a full-length object id",
            manifest.review_base_commit
        )));
    }
    let mut previous: Option<&str> = None;
    for gate in &manifest.gates {
        if !is_storable_verification_id(&gate.verification_id) {
            return Err(ManifestDefect::Malformed(format!(
                "verification id '{}' is not storable",
                gate.verification_id
            )));
        }
        if gate.argv.is_empty() || gate.argv.iter().any(|arg| arg.trim().is_empty()) {
            return Err(ManifestDefect::Malformed(format!(
                "gate '{}' has no supervisable argv",
                gate.verification_id
            )));
        }
        if let Some(previous) = previous {
            if previous >= gate.verification_id.as_str() {
                return Err(ManifestDefect::Malformed(format!(
                    "gates are not in canonical verification-id order at '{}'",
                    gate.verification_id
                )));
            }
        }
        previous = Some(gate.verification_id.as_str());
    }
    if let Some(terminal) = &manifest.terminal {
        if !is_full_length_object_id(&terminal.fingerprint) {
            return Err(ManifestDefect::Malformed(format!(
                "terminal fingerprint '{}' is not a full-length digest",
                terminal.fingerprint
            )));
        }
    }
    Ok(manifest)
}

// ============================================================================
// Storage
// ============================================================================

/// Repository-local manifest storage inside the existing evidence directory.
#[derive(Debug, Clone)]
pub struct ManifestStore {
    workspace: PathBuf,
}

impl ManifestStore {
    pub fn new(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
        }
    }

    /// Absolute manifest path for one change.
    pub fn path(&self, change_id: &str) -> PathBuf {
        self.workspace.join(manifest_relative_path(change_id))
    }

    /// Read and structurally validate the stored manifest.
    ///
    /// Unlike the verification sidecar this does not test the file mode: the
    /// manifest is runtime-derived rather than runtime-*witnessed*, so a forged
    /// one buys nothing. Every field it could lie about — commit, tree, blob,
    /// tool, artifact digest — is recomputed from live Git state before it is
    /// compared, and a forged terminal record can only ever *refuse* a retry the
    /// operator can restore by touching the repository.
    pub fn load(&self, change_id: &str) -> Result<AcceptanceExecutionManifest, ManifestDefect> {
        if !is_storable_change_id(change_id) {
            return Err(ManifestDefect::Malformed(format!(
                "change id '{change_id}' is not storable"
            )));
        }
        let path = self.path(change_id);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ManifestDefect::Missing)
            }
            Err(error) => return Err(ManifestDefect::Unreadable(error.to_string())),
        };
        parse_manifest(&bytes)
    }

    /// Atomically replace the stored manifest.
    ///
    /// Atomic replacement is what keeps a partially written manifest from ever
    /// becoming authority: a reader sees the previous complete file or the new
    /// complete file, never a prefix of either.
    pub fn store(&self, manifest: &AcceptanceExecutionManifest) -> std::io::Result<()> {
        if !is_storable_change_id(&manifest.change_id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("change id '{}' is not storable", manifest.change_id),
            ));
        }
        EvidenceStore::new(self.workspace.clone()).ensure_directory()?;
        let directory = self.workspace.join(EVIDENCE_DIR);
        let final_path = self.path(&manifest.change_id);
        let temporary = directory.join(format!(
            ".acceptance-manifest-{}.tmp-{}",
            manifest.change_id,
            std::process::id()
        ));
        let serialized = serde_json::to_vec_pretty(manifest)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(&temporary, &serialized)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        match std::fs::rename(&temporary, &final_path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    /// Record the terminal outcome of one invocation against the stored manifest.
    pub fn record_terminal(
        &self,
        manifest: &AcceptanceExecutionManifest,
        terminal: ManifestTerminal,
    ) -> std::io::Result<()> {
        let mut stored = manifest.clone();
        stored.terminal = Some(terminal);
        self.store(&stored)
    }

    /// Drop a stored manifest. Used when a change settles normally, so a stale
    /// hold cannot refuse a later legitimate attempt.
    pub fn clear(&self, change_id: &str) {
        if is_storable_change_id(change_id) {
            let _ = std::fs::remove_file(self.path(change_id));
        }
    }
}

// ============================================================================
// Admission
// ============================================================================

/// Whether a new bounded Acceptance attempt may be dispatched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceAdmission {
    /// No terminal hold applies to the current input; dispatch normally.
    Admit,
    /// The current fingerprint matches a recorded non-resumable deadline hold.
    Refuse {
        /// Always [`UNCHANGED_ACCEPTANCE_INPUT`].
        outcome: &'static str,
        /// The hold category that produced the refusal.
        category: AcceptanceHoldCategory,
        /// The unchanged fingerprint.
        fingerprint: String,
        /// Components whose repository-visible change restores eligibility.
        components: Vec<&'static str>,
    },
}

impl AcceptanceAdmission {
    pub fn is_refused(&self) -> bool {
        matches!(self, Self::Refuse { .. })
    }

    /// Operator-facing detail naming exactly what to change.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Admit => None,
            Self::Refuse {
                category,
                fingerprint,
                components,
                ..
            } => Some(format!(
                "Acceptance input is unchanged since the {} hold (fingerprint {}). \
                 A repository-visible change to one of {} restores retry eligibility.",
                category.as_str(),
                &fingerprint[..16.min(fingerprint.len())],
                components.join(", ")
            )),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Admit => serde_json::json!({ "admitted": true }),
            Self::Refuse {
                outcome,
                category,
                fingerprint,
                components,
            } => serde_json::json!({
                "admitted": false,
                "outcome": outcome,
                "category": category.as_str(),
                "fingerprint": fingerprint,
                "restore_eligibility_by_changing": components,
            }),
        }
    }
}

/// Decide whether the current input may be dispatched, given what a prior
/// invocation recorded.
///
/// Only a *non-resumable hold* whose recorded fingerprint equals the current one
/// refuses. Every other shape admits: no stored manifest, a stored manifest with
/// no terminal record, a terminal record from a different fingerprint, or a
/// terminal record with no hold category (an ordinary settled verdict). That
/// asymmetry is the point — the guard exists to stop one specific loop, not to
/// become a general gate on Acceptance.
pub fn classify_admission(
    current_fingerprint: &str,
    stored: Option<&AcceptanceExecutionManifest>,
) -> AcceptanceAdmission {
    let Some(terminal) = stored.and_then(|manifest| manifest.terminal.as_ref()) else {
        return AcceptanceAdmission::Admit;
    };
    let Some(category) = terminal.hold_category else {
        return AcceptanceAdmission::Admit;
    };
    if terminal.fingerprint != current_fingerprint {
        return AcceptanceAdmission::Admit;
    }
    AcceptanceAdmission::Refuse {
        outcome: UNCHANGED_ACCEPTANCE_INPUT,
        category,
        fingerprint: current_fingerprint.to_string(),
        components: FINGERPRINT_COMPONENTS.to_vec(),
    }
}

/// Read the stored manifest and classify admission for one change.
///
/// A malformed stored manifest **admits**: refusing on a file nobody can read
/// would strand a change on a defect it did not cause, and the worst case of
/// admitting is one more bounded attempt that rewrites the manifest correctly.
pub fn classify_admission_from_store(
    workspace: &Path,
    change_id: &str,
    current_fingerprint: &str,
) -> AcceptanceAdmission {
    match ManifestStore::new(workspace.to_path_buf()).load(change_id) {
        Ok(stored) => classify_admission(current_fingerprint, Some(&stored)),
        Err(_) => AcceptanceAdmission::Admit,
    }
}

// ============================================================================
// Construction
// ============================================================================

/// Workspace-derived admission for the shared operator command service.
///
/// Holds a resolver rather than a path because a change's managed worktree is
/// created and destroyed by the parallel runtime: resolving per call means a
/// change whose worktree has gone is admitted (there is no evidence to refuse
/// on) instead of being refused against a path that no longer exists.
pub struct WorkspaceAcceptanceAdmission<R> {
    resolve_workspace: R,
    skill_name: String,
}

impl<R> WorkspaceAcceptanceAdmission<R>
where
    R: Fn(&str) -> Option<PathBuf> + Send + Sync,
{
    pub fn new(resolve_workspace: R, skill_name: impl Into<String>) -> Self {
        Self {
            resolve_workspace,
            skill_name: skill_name.into(),
        }
    }
}

#[async_trait::async_trait]
impl<R> crate::orchestration::operator_command::AcceptanceAdmissionPort
    for WorkspaceAcceptanceAdmission<R>
where
    R: Fn(&str) -> Option<PathBuf> + Send + Sync,
{
    async fn classify(&self, change_id: &str) -> AcceptanceAdmission {
        let Some(workspace) = (self.resolve_workspace)(change_id) else {
            return AcceptanceAdmission::Admit;
        };
        // Nothing recorded means nothing to refuse, and reading the stored
        // manifest first is also how the base *ref* is recovered: recomputing
        // against a different base would produce a mismatching fingerprint for
        // the wrong reason and silently admit every unchanged retry.
        let store = ManifestStore::new(workspace.clone());
        let Ok(stored) = store.load(change_id) else {
            return AcceptanceAdmission::Admit;
        };
        if stored.terminal.is_none() {
            return AcceptanceAdmission::Admit;
        }
        // The deadline the manifest was built under does not participate in the
        // fingerprint, so passing the stored value back in is not required: any
        // value reproduces the same digest. Zero is used to say "no bound is
        // being asserted here", because this path starts nothing.
        let manifest = match super::manifest_builder::build_manifest_for_workspace(
            &super::verification_evidence::GitRepositoryFacts,
            &workspace,
            change_id,
            &self.skill_name,
            stored.review_base_ref.as_deref(),
            0,
        )
        .await
        {
            Ok(manifest) => manifest,
            // An underivable manifest admits, for the same reason a malformed
            // stored one does: a change must not be stranded on a defect it did
            // not cause.
            Err(_) => return AcceptanceAdmission::Admit,
        };
        classify_admission(&manifest.fingerprint(), Some(&stored))
    }
}

/// Which declarations the manifest is allowed to contain.
///
/// All four conditions, conjunctively. A declaration that fails any of them is
/// not a weaker gate — it is simply not part of the blocking boundary, and its
/// absence can never keep Acceptance open.
pub fn is_eligible_declaration(declaration: &VerificationDeclaration) -> bool {
    let phase_ok = declaration
        .phase
        .as_deref()
        .map(str::trim)
        .is_some_and(|phase| phase == PRE_INTEGRATION_PHASE);
    phase_ok && eligible_request(declaration).is_ok()
}

/// Select and normalize every eligible declared gate, in canonical order.
pub fn eligible_gates(declarations: &[VerificationDeclaration]) -> Vec<VerificationRequest> {
    let mut requests: Vec<VerificationRequest> = declarations
        .iter()
        .filter(|declaration| is_eligible_declaration(declaration))
        .filter_map(|declaration| eligible_request(declaration).ok())
        .collect();
    requests.sort_by(|left, right| left.verification_id.cmp(&right.verification_id));
    requests.dedup_by(|left, right| left.verification_id == right.verification_id);
    requests
}

/// Digest a set of canonical change inputs into one full-length value.
///
/// Path-tagged so moving content between files changes the digest, and sorted so
/// filesystem iteration order cannot.
pub fn digest_change_inputs(inputs: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<&(String, Vec<u8>)> = inputs.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (path, bytes) in sorted {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update(bytes.len().to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

/// Digest arbitrary text into a full-length value (skill and executable identity).
pub fn digest_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "execution_manifest/tests.rs"]
mod tests;
