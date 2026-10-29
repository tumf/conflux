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
//!   deadline already answered "no" is refused rather than redispatched —
//!   *within the owner that recorded the answer*.
//! * **Nothing lives inside the target, and nothing outside it is authority.**
//!   The manifest is persisted into the Conflux-owned external store, never into
//!   the target repository or a managed worktree (constitutional law 4), and it
//!   is cache and diagnostics only. Deleting it may cost a rerun; it may not
//!   change admission, retry, archive, merge, or next-action selection
//!   (constitutional law 1).
//!
//! **Retry refusal is process-local.** The unchanged-input guard lives in
//! [`LiveAcceptanceHolds`], a typed in-memory registry owned by the current
//! process. Ephemeral single-process state is explicitly permitted; a durable
//! record that could refuse work after a restart is not, because then deleting
//! out-of-worktree state would change the next action chosen for the same
//! workspace contents. So a restarted owner reconstructs no hold at all: the
//! first admission in the new lifetime — explicit retry or automatic mark
//! settlement — gets one fresh bounded attempt, while still-valid cached gate
//! results may be reused to make it cheap.
//!
//! What the fingerprint deliberately excludes is as load-bearing as what it
//! includes: timestamps, terminal output, and gate durations are all absent,
//! because a fingerprint that moved on its own would turn "nothing changed" into
//! an infinite retry loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::verification_evidence::{
    eligible_request, empty_env_binding_digest, is_storable_verification_id, render_command,
    DeclarationDefect, EnvBindings, EvidenceStore, IneligibleReason, ToolIdentity,
    VerificationRequest,
};
use crate::openspec::{
    VerificationCompletionRole, VerificationDeclaration, VerificationExecutionClass,
};

/// The only phase whose declarations may block a change before integration.
pub const PRE_INTEGRATION_PHASE: &str = "pre-integration";

/// Manifest schema.
///
/// Version 2 is where the manifest stopped being workflow authority: it lives in
/// the external store, its gate artifact paths are store-relative, and it no
/// longer carries a terminal record that could refuse a retry. A version-1
/// manifest describes the opposite arrangement, so it is refused rather than
/// migrated — reading one would mean resurrecting the durable admission this
/// change removed.
pub const MANIFEST_SCHEMA: &str = "conflux-acceptance-execution-manifest-v2";

/// File name of the manifest inside one change's external store.
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

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

/// Whether a change ID is usable as a single external-store path component.
///
/// Same rule the evidence envelope applies to a verification ID: anything that
/// could escape the store directory or collide across changes is refused rather
/// than sanitized, because a sanitized name binds a manifest to a change it does
/// not describe.
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
    /// The external Acceptance state path is unavailable, unwritable, or
    /// resolves inside the target repository or a managed worktree.
    ///
    /// Distinct from [`Self::RuntimeDefect`] because the operator action is
    /// different and knowable: point `state_base_dir` somewhere outside the
    /// project, or make the resolved directory writable. Reported *before* any
    /// declared gate starts, and never satisfied by writing into the target.
    StatePathUnavailable,
    /// The target still carries legacy `.cflx/verification-evidence` content.
    ///
    /// Its own category because it is neither a defect nor a verdict: the
    /// repository contains Conflux-owned files an earlier release wrote, and
    /// removing them is an explicit repository-owner action. The runtime refuses
    /// to proceed rather than opening, parsing, migrating, hiding, or deleting
    /// content it no longer owns.
    LegacyTargetEvidence,
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
            Self::StatePathUnavailable => "acceptance_state_path_unavailable",
            Self::LegacyTargetEvidence => "acceptance_legacy_target_evidence",
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
            "acceptance_state_path_unavailable" => Some(Self::StatePathUnavailable),
            "acceptance_legacy_target_evidence" => Some(Self::LegacyTargetEvidence),
            _ => None,
        }
    }

    /// Whether a recorded hold of this category refuses an *unchanged* retry.
    ///
    /// Only the three deadline categories do. They are the ones a fingerprint
    /// actually decides: the invocation was bounded, the bound expired, and
    /// asking the identical question again can only expire again.
    ///
    /// Every other category deliberately does not. None of them is a statement
    /// about the input at all — a dirty worktree, an unwritable state root, a
    /// transient Git failure, an unparsable cache record, or legacy evidence a
    /// repository owner has since deleted can all recover without any
    /// fingerprint component moving, and none of those facts is a fingerprint
    /// component. Refusing on them would turn a recoverable environment fact
    /// into a permanent `unchanged_acceptance_input` refusal nobody can clear by
    /// fixing the environment.
    pub fn refuses_unchanged_retry(self) -> bool {
        match self {
            Self::DeclaredGateDeadlineExhausted
            | Self::ReviewDeadlineExhausted
            | Self::ReviewCleanupUnproven => true,
            Self::ManifestMalformed
            | Self::RuntimeDefect
            | Self::StatePathUnavailable
            | Self::LegacyTargetEvidence => false,
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
            Self::StatePathUnavailable => {
                "the external Conflux-owned Acceptance state path could not be resolved, created, \
                 or written, and Acceptance may not fall back to the target worktree"
            }
            Self::LegacyTargetEvidence => {
                "the target still carries legacy Conflux-owned '.cflx/verification-evidence' \
                 content, which this runtime neither reads nor removes"
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
    ///
    /// The refusal clause is stated only for the categories that actually refuse
    /// — and only for the owner that recorded them. Claiming a general refusal
    /// on a state-path or legacy-evidence hold would tell an operator to edit
    /// the repository when what they need to do is fix the environment.
    pub fn summary(&self, change_id: &str) -> String {
        let head = format!(
            "Acceptance for '{}' settled as a non-resumable execution hold ({}): {}. \
             Input fingerprint {}. This is not a PASS, not a FAIL, and not an external block.",
            change_id,
            self.category.as_str(),
            self.category.detail(),
            &self.fingerprint[..16.min(self.fingerprint.len())],
        );
        if self.category.refuses_unchanged_retry() {
            format!(
                "{head} While this owner process is alive, a retry with unchanged repository \
                 input is refused as `{}`; change one of {} to permit a new bounded attempt. A \
                 restarted owner admits one fresh attempt without a repository change.",
                UNCHANGED_ACCEPTANCE_INPUT,
                FINGERPRINT_COMPONENTS.join(", "),
            )
        } else {
            format!("{head} {}", self.next_action())
        }
    }

    /// What an operator does about this hold.
    pub fn next_action(&self) -> String {
        match self.category {
            AcceptanceHoldCategory::StatePathUnavailable => {
                "Make the Conflux-owned Acceptance state root resolvable and writable outside the \
                 target repository — set `state_base_dir` to an absolute path outside the project, \
                 or grant write permission on the resolved directory — then retry acceptance."
                    .to_string()
            }
            AcceptanceHoldCategory::LegacyTargetEvidence => {
                "Remove the legacy '.cflx/verification-evidence' directory from the target \
                 yourself; Conflux will not read, migrate, or delete it. Then retry acceptance."
                    .to_string()
            }
            _ if self.category.refuses_unchanged_retry() => format!(
                "Change one of {} in the repository, then retry explicitly. While this owner \
                 process is alive an unchanged retry is refused as `{}`.",
                FINGERPRINT_COMPONENTS.join(", "),
                UNCHANGED_ACCEPTANCE_INPUT
            ),
            _ => "Resolve the reported runtime condition, then retry acceptance.".to_string(),
        }
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
            next_action: self.next_action(),
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
    /// Executable plus arguments. Never contains a leading assignment.
    pub argv: Vec<String>,
    /// Sorted, distinct names of the effective environment bindings.
    #[serde(default)]
    pub env_names: Vec<String>,
    /// Full-length digest of the effective bindings — identity by digest,
    /// never by value.
    #[serde(default = "empty_env_binding_digest")]
    pub env_binding_digest: String,
    /// The binding values themselves, in memory only.
    ///
    /// Never serialized: a manifest written to the store carries names and the
    /// digest, never a value. The manifest is frozen and executed by the same
    /// owner, so the runtime that starts the child still holds the values.
    #[serde(skip)]
    pub env: EnvBindings,
    pub cwd: String,
    pub automation_path: String,
    /// Full-length blob ID of the automation file at the candidate commit, or
    /// empty when it could not be observed (which forces execution).
    #[serde(default)]
    pub automation_blob_oid: String,
    #[serde(default)]
    pub tool: ToolIdentity,
    /// Store-relative artifact path the runtime binds results to.
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
            "env_names": self.env_names,
            "env_binding_digest": self.env_binding_digest,
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
            env: self.env.clone(),
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
    ///
    /// The runtime terminated the command and swept its owned process group
    /// before producing this outcome. `cleanup_confirmed` says whether that
    /// sweep *proved* the group empty: "we stopped it" and "it is gone" are
    /// separate facts, and the hold this becomes must not claim the second from
    /// the first.
    DeadlineExhausted {
        verification_id: String,
        #[serde(default)]
        cleanup_confirmed: bool,
        #[serde(default)]
        cleanup_diagnostics: String,
    },
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
            | Self::DeadlineExhausted {
                verification_id, ..
            } => verification_id,
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
            Self::DeadlineExhausted {
                verification_id,
                cleanup_confirmed,
                cleanup_diagnostics,
            } => format!(
                "{verification_id}: did not finish inside the shared Acceptance budget; \
                 terminated, process-group cleanup {}",
                if *cleanup_confirmed {
                    "confirmed".to_string()
                } else {
                    format!("unproven: {cleanup_diagnostics}")
                }
            ),
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

    /// Process-group cleanup evidence for the gate phase.
    ///
    /// A gate that hit the shared deadline was terminated, and its own sweep is
    /// the only thing that knows whether the group is provably gone. Every other
    /// shape started nothing that could survive — a refusal never spawned, a
    /// reuse never ran, and a completed gate exited on its own — so the honest
    /// answer there is a confirmed cleanup with no reviewer involved.
    pub fn cleanup_evidence(&self) -> (bool, String) {
        match self.first_deadline_exhausted() {
            Some(GateOutcome::DeadlineExhausted {
                cleanup_confirmed,
                cleanup_diagnostics,
                ..
            }) => (*cleanup_confirmed, cleanup_diagnostics.clone()),
            _ => (
                true,
                "no reviewer was started and no gate was left running".to_string(),
            ),
        }
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

/// What one invocation left behind, as *diagnostics*.
///
/// Persisted next to the manifest in the external store so an operator reading
/// the cache can see how the last attempt ended. It is deliberately inert: no
/// code path reads it back to decide admission, retry, archive, merge, or the
/// next workflow action, and deleting the whole store changes nothing but how
/// much work the next attempt repeats. The refusal that *is* consulted lives in
/// [`LiveAcceptanceHolds`], in memory, for this process only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceDiagnostics {
    /// The hold category, when the invocation settled as a non-resumable hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_category: Option<AcceptanceHoldCategory>,
    /// The fingerprint that was current when the diagnostics were recorded.
    pub fingerprint: String,
    /// Whether the owned process group was proven quiescent.
    pub cleanup_confirmed: bool,
    /// One-line cleanup diagnostics.
    #[serde(default)]
    pub cleanup_diagnostics: String,
    /// Concrete evidence lines from the invocation.
    #[serde(default)]
    pub evidence: Vec<String>,
    pub recorded_at: DateTime<Utc>,
    /// The process that wrote this record. Present so a reader can tell "the
    /// owner that produced this is gone" without inferring it, and absent from
    /// every decision.
    #[serde(default)]
    pub recorded_by_pid: u32,
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
    ///
    /// Always empty when [`Self::declaration_defects`] is not: an invalid
    /// required manifest offers no gate to run and no gate to reuse.
    pub gates: Vec<ManifestGate>,
    /// Typed defects of the proposal's declaration list, in declaration order.
    ///
    /// Empty for a valid manifest. Non-empty is an *invalid required manifest*,
    /// which is what distinguishes "a required declaration could not be
    /// supervised" from a legitimately empty allowlist.
    #[serde(default)]
    pub declaration_defects: Vec<DeclarationDefect>,
    pub created_at: DateTime<Utc>,
    /// Shared effective absolute deadline, in seconds, for gates plus review.
    #[serde(default)]
    pub absolute_deadline_secs: u64,
    /// Work budget after the finalization reserve is taken out.
    #[serde(default)]
    pub work_budget_secs: u64,
}

impl AcceptanceExecutionManifest {
    /// SHA-256 over canonical JSON of every field that can change review
    /// authority or gate validity.
    ///
    /// Timestamps and budgets are excluded: a fingerprint that moved on its own
    /// would let an unchanged retry look changed. Gate ordering is deterministic
    /// by verification ID, which is enforced at construction and re-checked by
    /// [`parse_manifest`].
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
            "declaration_defects": self
                .declaration_defects
                .iter()
                .map(DeclarationDefect::to_json)
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

    /// Whether the proposal's required declarations could not all be planned.
    ///
    /// Distinct from `!has_gates()`: an empty allowlist with no defects is a
    /// legitimate boundary, an invalid required manifest is a `fail` verdict.
    pub fn is_declaration_invalid(&self) -> bool {
        !self.declaration_defects.is_empty()
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
                    "command": render_command(&gate.env_names, &gate.argv),
                    "env_names": gate.env_names,
                    "automation": gate.automation_path,
                }))
                .collect::<Vec<_>>(),
            "declaration_defects": self
                .declaration_defects
                .iter()
                .map(DeclarationDefect::to_json)
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
    if !manifest.declaration_defects.is_empty() && !manifest.gates.is_empty() {
        return Err(ManifestDefect::Malformed(
            "a manifest with declaration defects must carry no gate".to_string(),
        ));
    }
    Ok(manifest)
}

// ============================================================================
// Storage
// ============================================================================

/// Conflux-owned external manifest storage for one change.
///
/// Rooted at the same already-resolved external directory [`EvidenceStore`]
/// uses, and for the same reason: there is no constructor that can derive a path
/// from a workspace, so no caller can accidentally write a manifest into a
/// target. The change ID is already a component of that root, so the file name
/// here is fixed.
#[derive(Debug, Clone)]
pub struct ManifestStore {
    root: PathBuf,
}

impl ManifestStore {
    /// Bind the store to one already-resolved external directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Absolute manifest path.
    pub fn path(&self) -> PathBuf {
        self.root.join(MANIFEST_FILE_NAME)
    }

    /// Read and structurally validate the stored manifest.
    ///
    /// Unlike the verification envelope this does not test the file mode: the
    /// manifest is runtime-derived rather than runtime-*witnessed*, so a forged
    /// one buys nothing. Every field it could lie about — commit, tree, blob,
    /// tool, artifact digest — is recomputed from live Git state before it is
    /// compared, and since the manifest no longer carries admission authority a
    /// forged one cannot refuse anything at all.
    pub fn load(&self) -> Result<AcceptanceExecutionManifest, ManifestDefect> {
        let path = self.path();
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
    /// becoming a readable half-record: a reader sees the previous complete file
    /// or the new complete file, never a prefix of either.
    pub fn store(&self, manifest: &AcceptanceExecutionManifest) -> std::io::Result<()> {
        if !is_storable_change_id(&manifest.change_id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("change id '{}' is not storable", manifest.change_id),
            ));
        }
        self.write_atomically(MANIFEST_FILE_NAME, &serde_json::to_vec_pretty(manifest)?)
    }

    /// File name of the inert per-invocation diagnostics record.
    pub const DIAGNOSTICS_FILE_NAME: &'static str = "diagnostics.json";

    /// Absolute path of the diagnostics record.
    pub fn diagnostics_path(&self) -> PathBuf {
        self.root.join(Self::DIAGNOSTICS_FILE_NAME)
    }

    /// Record how one invocation ended, for an operator reading the cache.
    ///
    /// Write-only from the runtime's point of view: nothing loads this back to
    /// decide anything, which is precisely what keeps external state out of
    /// workflow control.
    pub fn record_diagnostics(&self, diagnostics: &AcceptanceDiagnostics) -> std::io::Result<()> {
        self.write_atomically(
            Self::DIAGNOSTICS_FILE_NAME,
            &serde_json::to_vec_pretty(diagnostics)?,
        )
    }

    fn write_atomically(&self, name: &str, serialized: &[u8]) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let temporary = self
            .root
            .join(format!(".{name}.tmp-{}", std::process::id()));
        std::fs::write(&temporary, serialized)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        match std::fs::rename(&temporary, self.root.join(name)) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                Err(error)
            }
        }
    }
}

// ============================================================================
// Live, process-local retry refusal
// ============================================================================

/// One non-resumable Acceptance hold, remembered for this process only.
///
/// Everything needed to answer "is the current input the same input" and
/// nothing else: no verdict, no findings, no history. It is discarded on
/// restart by construction, because a `HashMap` in a `OnceLock` has nowhere to
/// survive to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveAcceptanceHold {
    pub category: AcceptanceHoldCategory,
    /// The fingerprint the hold was recorded against.
    pub fingerprint: String,
    /// The revision name the review base was resolved from, so a later
    /// recomputation re-resolves the same ref rather than a different one.
    pub review_base_ref: Option<String>,
}

/// The current owner's typed Acceptance refusal state.
///
/// Constitutional law 1 allows ephemeral in-memory state held within a single
/// process lifetime, and forbids durable out-of-worktree state that changes the
/// next action. This registry is exactly the former: it suppresses a repeat of a
/// question the deadline already answered, and it vanishes with the process that
/// asked it.
#[derive(Debug, Default)]
pub struct LiveAcceptanceHolds {
    holds: Mutex<HashMap<String, LiveAcceptanceHold>>,
}

impl LiveAcceptanceHolds {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember a non-resumable hold for `change_id`.
    ///
    /// A category that does not refuse an unchanged retry is deliberately not
    /// remembered: storing it would only give a later reader something
    /// misleading to find, since admission would admit on it anyway.
    pub fn record(&self, change_id: &str, hold: LiveAcceptanceHold) {
        if !hold.category.refuses_unchanged_retry() {
            self.clear(change_id);
            return;
        }
        self.lock().insert(change_id.to_string(), hold);
    }

    /// Forget any hold for `change_id`.
    ///
    /// Called when a fresh attempt is admitted, so the refusal describes the
    /// latest answer rather than accumulating.
    pub fn clear(&self, change_id: &str) {
        self.lock().remove(change_id);
    }

    /// The hold currently recorded for `change_id`, if any.
    pub fn get(&self, change_id: &str) -> Option<LiveAcceptanceHold> {
        self.lock().get(change_id).cloned()
    }

    /// Classify one change's current fingerprint against live state.
    pub fn classify(&self, change_id: &str, current_fingerprint: &str) -> AcceptanceAdmission {
        classify_admission(current_fingerprint, self.get(change_id).as_ref())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, LiveAcceptanceHold>> {
        self.holds
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The process-wide registry.
///
/// A single instance because the refusal must be the same fact for the parallel
/// executor that records it and the shared operator-command service that reads
/// it; two registries would let a keypress and a gate disagree about whether a
/// change is retryable.
pub fn live_acceptance_holds() -> &'static Arc<LiveAcceptanceHolds> {
    static HOLDS: OnceLock<Arc<LiveAcceptanceHolds>> = OnceLock::new();
    HOLDS.get_or_init(|| Arc::new(LiveAcceptanceHolds::new()))
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

/// Decide whether the current input may be dispatched, given what this owner
/// still remembers.
///
/// Only a live *deadline* hold whose fingerprint equals the current one refuses.
/// Every other shape admits: no live hold at all (which is what a restarted
/// owner always sees), a hold from a different fingerprint, and a hold category
/// that says nothing about the input
/// ([`AcceptanceHoldCategory::refuses_unchanged_retry`]). That asymmetry is the
/// point — the guard exists to stop one specific loop within one process, not to
/// become a general gate on Acceptance.
pub fn classify_admission(
    current_fingerprint: &str,
    live: Option<&LiveAcceptanceHold>,
) -> AcceptanceAdmission {
    let Some(live) = live else {
        return AcceptanceAdmission::Admit;
    };
    // A runtime defect, an unusable state path, or legacy target evidence is an
    // environment fact, not an answer about this input. The fingerprint cannot
    // express "the worktree was dirty" or "the state root was unwritable", so an
    // equal fingerprint after one of those proves nothing and must not strand
    // the change.
    if !live.category.refuses_unchanged_retry() {
        return AcceptanceAdmission::Admit;
    }
    if live.fingerprint != current_fingerprint {
        return AcceptanceAdmission::Admit;
    }
    AcceptanceAdmission::Refuse {
        outcome: UNCHANGED_ACCEPTANCE_INPUT,
        category: live.category,
        fingerprint: current_fingerprint.to_string(),
        components: FINGERPRINT_COMPONENTS.to_vec(),
    }
}

// ============================================================================
// Construction
// ============================================================================

/// Live-owner admission for the shared operator command service.
///
/// Two facts are combined, and neither alone is enough. The *refusal* comes from
/// this process's own [`LiveAcceptanceHolds`], so a restarted owner refuses
/// nothing. The *comparison* is recomputed from the managed worktree, so a
/// repository change genuinely restores eligibility instead of waiting for
/// someone to clear a flag.
///
/// Holds a resolver rather than a path because a change's managed worktree is
/// created and destroyed by the parallel runtime: resolving per call means a
/// change whose worktree has gone is admitted (there is nothing to compare
/// against) instead of being refused against a path that no longer exists.
pub struct WorkspaceAcceptanceAdmission<R> {
    resolve_workspace: R,
    skill_name: String,
    /// The configured `state_base_dir`, so the recomputation reads the same
    /// external cache the executor wrote.
    state_base_dir: Option<String>,
    holds: Arc<LiveAcceptanceHolds>,
}

impl<R> WorkspaceAcceptanceAdmission<R>
where
    R: Fn(&str) -> Option<PathBuf> + Send + Sync,
{
    /// Bind to the process-wide live registry the executor records into.
    pub fn new(
        resolve_workspace: R,
        skill_name: impl Into<String>,
        state_base_dir: Option<String>,
    ) -> Self {
        Self::with_holds(
            resolve_workspace,
            skill_name,
            state_base_dir,
            live_acceptance_holds().clone(),
        )
    }

    /// Bind to an explicit registry. Used by tests that must not share process
    /// state with a concurrently running case.
    pub fn with_holds(
        resolve_workspace: R,
        skill_name: impl Into<String>,
        state_base_dir: Option<String>,
        holds: Arc<LiveAcceptanceHolds>,
    ) -> Self {
        Self {
            resolve_workspace,
            skill_name: skill_name.into(),
            state_base_dir,
            holds,
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
        // Nothing this owner remembers means nothing to refuse — and it is
        // checked first so the common path costs no Git subprocesses at all.
        let Some(live) = self.holds.get(change_id) else {
            return AcceptanceAdmission::Admit;
        };
        let Some(workspace) = (self.resolve_workspace)(change_id) else {
            return AcceptanceAdmission::Admit;
        };
        // Bound to the *existing* external store without creating one: this path
        // starts nothing, and materializing a cache directory as a side effect
        // of answering "may this be retried" would be a write nobody asked for.
        // A store that is absent simply contributes no artifact digests, which
        // is the same thing a deleted cache contributes.
        let project_root = super::manifest_builder::repository_project_root(&workspace)
            .await
            .unwrap_or_else(|| workspace.clone());
        let Ok(store_root) = super::evidence_location::store_path(
            self.state_base_dir.as_deref(),
            &project_root,
            &workspace,
            change_id,
        ) else {
            return AcceptanceAdmission::Admit;
        };
        // The base *ref* comes from the hold rather than being guessed:
        // recomputing against a different base would produce a mismatching
        // fingerprint for the wrong reason and silently admit every unchanged
        // retry.
        //
        // The deadline the manifest was built under does not participate in the
        // fingerprint, so any value reproduces the same digest. Zero is used to
        // say "no bound is being asserted here", because this path starts
        // nothing.
        let manifest = match super::manifest_builder::build_manifest_for_workspace(
            &super::verification_evidence::GitRepositoryFacts,
            &workspace,
            &EvidenceStore::new(store_root),
            change_id,
            &self.skill_name,
            live.review_base_ref.as_deref(),
            0,
        )
        .await
        {
            Ok(manifest) => manifest,
            // An underivable manifest admits: a change must not be stranded on a
            // defect it did not cause, and the worst case of admitting is one
            // more bounded attempt.
            Err(_) => return AcceptanceAdmission::Admit,
        };
        classify_admission(&manifest.fingerprint(), Some(&live))
    }
}

/// The shared declaration planner's answer for one proposal.
///
/// The only way any consumer — strict validation, `cflx openspec verify`, or the
/// native Acceptance manifest — derives the blocking gate set. `gates` is empty
/// whenever `defects` is not, so a consumer that forgets to look at the defects
/// still cannot run or reuse a gate from an invalid required manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationPlan {
    /// Supervisable requests of every required declaration, in canonical
    /// verification-ID order. Empty when any defect exists.
    pub gates: Vec<VerificationRequest>,
    /// Every declaration defect, in declaration order.
    pub defects: Vec<DeclarationDefect>,
}

impl DeclarationPlan {
    /// Whether the required set is usable as a blocking boundary.
    pub fn is_valid(&self) -> bool {
        self.defects.is_empty()
    }
}

/// Plan the blocking gate set of one proposal's ordered declaration list.
///
/// Classification is separate from eligibility, which is the whole point: a
/// required (`completion_role: change-blocking`) declaration that cannot be
/// supervised is a typed defect rather than something that silently leaves the
/// allowlist. Rules, in order:
///
/// 1. A non-empty trimmed `id` declared more than once makes every occurrence a
///    `declaration_duplicate_id` defect, whatever its role. No deduplication.
/// 2. Required-ness is read from `completion_role` alone; non-required and
///    legacy declarations produce neither gates nor defects.
/// 3. A required declaration off `pre-integration` or off `repository-local` is
///    `declaration_role_conflict` on the offending field.
/// 4. Identity, command, and automation, through the same derivation reuse uses.
///
/// Pure: no PATH lookup, no Git state, no network.
pub fn plan_declared_gates(declarations: &[VerificationDeclaration]) -> DeclarationPlan {
    let trimmed_id = |declaration: &VerificationDeclaration| {
        declaration
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
    };
    let mut positions_by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, declaration) in declarations.iter().enumerate() {
        if let Some(id) = trimmed_id(declaration) {
            positions_by_id.entry(id).or_default().push(index + 1);
        }
    }

    let mut plan = DeclarationPlan::default();
    for (index, declaration) in declarations.iter().enumerate() {
        let position = index + 1;
        let id = trimmed_id(declaration);
        let label = id
            .clone()
            .unwrap_or_else(|| format!("<position {position}>"));
        let defect = |field: &str, code: &str, detail: String| DeclarationDefect {
            verification_id: label.clone(),
            position,
            field: field.to_string(),
            code: code.to_string(),
            detail,
        };

        if let Some(positions) = id
            .as_ref()
            .and_then(|id| positions_by_id.get(id))
            .filter(|positions| positions.len() > 1)
        {
            let listed = positions
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            plan.defects.push(defect(
                "id",
                "declaration_duplicate_id",
                format!(
                    "verification id '{label}' is declared more than once (positions {listed})"
                ),
            ));
            continue;
        }

        let required = declaration
            .completion_role
            .as_deref()
            .and_then(VerificationCompletionRole::parse)
            == Some(VerificationCompletionRole::ChangeBlocking);
        if !required {
            continue;
        }

        let phase = declaration.phase.as_deref().map(str::trim).unwrap_or("");
        let local = declaration
            .execution_class
            .as_deref()
            .and_then(VerificationExecutionClass::parse)
            .is_some_and(VerificationExecutionClass::is_repository_local);
        let mut role_conflict = false;
        if phase != PRE_INTEGRATION_PHASE {
            role_conflict = true;
            plan.defects.push(defect(
                "phase",
                "declaration_role_conflict",
                format!(
                    "completion_role: change-blocking requires phase: {PRE_INTEGRATION_PHASE}, \
                     found '{phase}'"
                ),
            ));
        }
        if !local {
            role_conflict = true;
            plan.defects.push(defect(
                "execution_class",
                "declaration_role_conflict",
                format!(
                    "completion_role: change-blocking requires execution_class: repository-local, \
                     found '{}'",
                    declaration.execution_class.as_deref().unwrap_or("").trim()
                ),
            ));
        }
        if role_conflict {
            continue;
        }

        match eligible_request(declaration) {
            Ok(request) => plan.gates.push(request),
            Err(reason) => {
                let field = match &reason {
                    IneligibleReason::MissingId | IneligibleReason::UnstorableId(_) => "id",
                    IneligibleReason::MissingCommand => "rerun",
                    IneligibleReason::ShellSyntax(_)
                    | IneligibleReason::InvalidAssignment(_)
                    | IneligibleReason::AssignmentOnly
                        if declaration.rerun.is_none() =>
                    {
                        "evidence"
                    }
                    IneligibleReason::ShellSyntax(_)
                    | IneligibleReason::InvalidAssignment(_)
                    | IneligibleReason::AssignmentOnly => "rerun",
                    IneligibleReason::MissingAutomation => "automation",
                    // Unreachable after the role checks above; reported under
                    // the role field rather than dropped if that ever changes.
                    IneligibleReason::NotRepositoryLocal => "execution_class",
                    IneligibleReason::NotChangeBlocking => "completion_role",
                };
                plan.defects
                    .push(defect(field, reason.code(), reason.detail()));
            }
        }
    }

    if plan.defects.is_empty() {
        plan.gates
            .sort_by(|left, right| left.verification_id.cmp(&right.verification_id));
    } else {
        plan.gates.clear();
    }
    plan
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
