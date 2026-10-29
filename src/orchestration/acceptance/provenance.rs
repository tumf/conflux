//! Who decided an Acceptance settlement, and about which candidate.
//!
//! An operator looking at a stopped change used to see a sanitized error, a
//! blocker, or a `stalled` row — none of which said whether the runtime's own
//! gate phase settled it before any reviewer existed, or the semantic reviewer
//! did, or which candidate commit and which `proposal.md` content were judged.
//!
//! [`AcceptanceProvenance`] answers those questions from typed facts the
//! executor already holds — the frozen manifest, the typed gate outcomes, and
//! the typed verdict — and from nothing else. Three properties are structural:
//!
//! * **Identities and counts only.** The record has no argv and no environment
//!   map: it carries the deciding gate's `program`, `argument_count`, and
//!   `env_names`. A declared command can carry a secret outside an assignment,
//!   so there is nothing to redact at render time because a value never enters
//!   the type.
//! * **Closed vocabularies.** Every token is an enum, and the mapping from a
//!   typed result to [`ReasonCode`] is an exhaustive `match`: a new result
//!   variant fails to compile rather than silently publishing `null`.
//! * **Observability, never authority.** The record is process-local, published
//!   on the change row, rendered in the TUI, and logged once. Retry eligibility,
//!   admission, archive, merge, and next-action selection never read it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::execution_manifest::{
    AcceptanceExecutionManifest, AcceptanceHoldCategory, GateOutcome, GateResults, ManifestGate,
};
use super::gate_execution::{GateVerdict, TOOL_UNAVAILABLE};
use super::verification_evidence::RepositoryFacts;

/// Whether the runtime or the semantic reviewer settled the invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    /// Settled by the runtime before any reviewer subprocess was spawned.
    NativeVerification,
    /// Settled after a reviewer subprocess was spawned.
    Reviewer,
}

impl DecisionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeVerification => "native_verification",
            Self::Reviewer => "reviewer",
        }
    }
}

/// Closed reason token for one Acceptance settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    DeclarationInvalid,
    GateFailed,
    ToolUnavailable,
    GateDeadlineExhausted,
    StatePathUnavailable,
    LegacyTargetEvidence,
    ManifestMalformed,
    RuntimeDefect,
    ReviewerPass,
    ReviewerFail,
    ReviewerStalled,
    ReviewerPermissionStalled,
    ReviewerCommandFailed,
    ReviewerMissingVerdict,
    ReviewDeadlineExhausted,
    ReviewCleanupUnproven,
    Cancelled,
}

impl ReasonCode {
    /// Every token, in documentation order.
    pub const ALL: [Self; 17] = [
        Self::DeclarationInvalid,
        Self::GateFailed,
        Self::ToolUnavailable,
        Self::GateDeadlineExhausted,
        Self::StatePathUnavailable,
        Self::LegacyTargetEvidence,
        Self::ManifestMalformed,
        Self::RuntimeDefect,
        Self::ReviewerPass,
        Self::ReviewerFail,
        Self::ReviewerStalled,
        Self::ReviewerPermissionStalled,
        Self::ReviewerCommandFailed,
        Self::ReviewerMissingVerdict,
        Self::ReviewDeadlineExhausted,
        Self::ReviewCleanupUnproven,
        Self::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeclarationInvalid => "declaration_invalid",
            Self::GateFailed => "gate_failed",
            Self::ToolUnavailable => "tool_unavailable",
            Self::GateDeadlineExhausted => "gate_deadline_exhausted",
            Self::StatePathUnavailable => "state_path_unavailable",
            Self::LegacyTargetEvidence => "legacy_target_evidence",
            Self::ManifestMalformed => "manifest_malformed",
            Self::RuntimeDefect => "runtime_defect",
            Self::ReviewerPass => "reviewer_pass",
            Self::ReviewerFail => "reviewer_fail",
            Self::ReviewerStalled => "reviewer_stalled",
            Self::ReviewerPermissionStalled => "reviewer_permission_stalled",
            Self::ReviewerCommandFailed => "reviewer_command_failed",
            Self::ReviewerMissingVerdict => "reviewer_missing_verdict",
            Self::ReviewDeadlineExhausted => "review_deadline_exhausted",
            Self::ReviewCleanupUnproven => "review_cleanup_unproven",
            Self::Cancelled => "cancelled",
        }
    }

    /// The reason a non-resumable execution hold of `category` settles with.
    pub fn from_hold_category(category: AcceptanceHoldCategory) -> Self {
        match category {
            AcceptanceHoldCategory::DeclaredGateDeadlineExhausted => Self::GateDeadlineExhausted,
            AcceptanceHoldCategory::ReviewDeadlineExhausted => Self::ReviewDeadlineExhausted,
            AcceptanceHoldCategory::ReviewCleanupUnproven => Self::ReviewCleanupUnproven,
            AcceptanceHoldCategory::ManifestMalformed => Self::ManifestMalformed,
            AcceptanceHoldCategory::RuntimeDefect => Self::RuntimeDefect,
            AcceptanceHoldCategory::StatePathUnavailable => Self::StatePathUnavailable,
            AcceptanceHoldCategory::LegacyTargetEvidence => Self::LegacyTargetEvidence,
        }
    }
}

/// Closed next-step token. A pure function of the reason and the divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Remediation {
    FixDeclaration,
    SatisfyEnvironment,
    RepairCandidate,
    Retry,
    RefreshCandidate,
    None,
}

impl Remediation {
    pub const ALL: [Self; 6] = [
        Self::FixDeclaration,
        Self::SatisfyEnvironment,
        Self::RepairCandidate,
        Self::Retry,
        Self::RefreshCandidate,
        Self::None,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FixDeclaration => "fix_declaration",
            Self::SatisfyEnvironment => "satisfy_environment",
            Self::RepairCandidate => "repair_candidate",
            Self::Retry => "retry",
            Self::RefreshCandidate => "refresh_candidate",
            Self::None => "none",
        }
    }
}

/// The remediation table.
///
/// A defective declaration on a candidate whose `proposal.md` already diverged
/// from the bound review base most likely carries a stale proposal, so the
/// recommendation is to refresh the candidate rather than to edit a
/// declaration that may already be fixed upstream.
pub fn remediation_for(reason: ReasonCode, divergence: DivergenceStatus) -> Remediation {
    match reason {
        ReasonCode::DeclarationInvalid => match divergence {
            DivergenceStatus::Diverged => Remediation::RefreshCandidate,
            DivergenceStatus::Identical | DivergenceStatus::Unknown => Remediation::FixDeclaration,
        },
        ReasonCode::ToolUnavailable => Remediation::SatisfyEnvironment,
        ReasonCode::GateFailed | ReasonCode::ReviewerFail => Remediation::RepairCandidate,
        ReasonCode::GateDeadlineExhausted
        | ReasonCode::StatePathUnavailable
        | ReasonCode::LegacyTargetEvidence
        | ReasonCode::ManifestMalformed
        | ReasonCode::RuntimeDefect
        | ReasonCode::ReviewerStalled
        | ReasonCode::ReviewerPermissionStalled
        | ReasonCode::ReviewerCommandFailed
        | ReasonCode::ReviewerMissingVerdict
        | ReasonCode::ReviewDeadlineExhausted
        | ReasonCode::ReviewCleanupUnproven
        | ReasonCode::Cancelled => Remediation::Retry,
        ReasonCode::ReviewerPass => Remediation::None,
    }
}

/// Whether the candidate's `proposal.md` matches the bound review base's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DivergenceStatus {
    Identical,
    Diverged,
    Unknown,
}

impl DivergenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Diverged => "diverged",
            Self::Unknown => "unknown",
        }
    }
}

/// Why a divergence could not be computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DivergenceUnknownReason {
    /// No manifest exists, so there is no bound review base to compare with.
    NoManifest,
    /// The manifest bound no review base, or it did not resolve.
    BaseUnresolvable,
    /// The review base has no `proposal.md` for this change.
    SourceProposalMissing,
    /// The candidate commit has no `proposal.md` for this change.
    CandidateProposalMissing,
}

impl DivergenceUnknownReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoManifest => "no_manifest",
            Self::BaseUnresolvable => "base_unresolvable",
            Self::SourceProposalMissing => "source_proposal_missing",
            Self::CandidateProposalMissing => "candidate_proposal_missing",
        }
    }
}

/// Explicit comparison of the candidate `proposal.md` with the bound review
/// base's. Never guessed: no branch name is inferred and no remote contacted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
pub struct CandidateDivergence {
    pub status: DivergenceStatus,
    /// The revision name the review base was bound from.
    pub source_ref: Option<String>,
    /// Full-length commit the review base resolved to.
    pub source_commit: Option<String>,
    /// Blob ID of `proposal.md` at the review base.
    pub source_blob: Option<String>,
    /// Blob ID of `proposal.md` at the candidate commit.
    pub candidate_blob: Option<String>,
    /// Why the status is `unknown`; `null` otherwise.
    pub reason: Option<DivergenceUnknownReason>,
}

impl CandidateDivergence {
    pub fn unknown(reason: DivergenceUnknownReason) -> Self {
        Self {
            status: DivergenceStatus::Unknown,
            source_ref: None,
            source_commit: None,
            source_blob: None,
            candidate_blob: None,
            reason: Some(reason),
        }
    }

    /// The operator-facing `Source proposal:` value.
    pub fn render(&self) -> String {
        match self.status {
            DivergenceStatus::Identical => "identical".to_string(),
            DivergenceStatus::Diverged => format!(
                "diverged from {}@{}",
                self.source_ref.as_deref().unwrap_or("?"),
                short_id(self.source_commit.as_deref().unwrap_or("?"))
            ),
            DivergenceStatus::Unknown => format!(
                "unknown ({})",
                self.reason
                    .map_or("unspecified", DivergenceUnknownReason::as_str)
            ),
        }
    }
}

/// One typed provenance record per Acceptance settlement.
///
/// No field ever holds a command argument value, an environment value, prompt
/// text, or process output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(utoipa::ToSchema))]
pub struct AcceptanceProvenance {
    pub decision_source: DecisionSource,
    /// `true` only when a reviewer subprocess was actually spawned.
    pub reviewer_started: bool,
    pub reason_code: ReasonCode,
    /// The planner's declaration defect code for `declaration_invalid`.
    pub defect_code: Option<String>,
    /// The deciding gate or defective declaration.
    pub verification_id: Option<String>,
    /// `openspec/changes/<id>/proposal.md` when `verification_id` is set.
    pub declaration_path: Option<String>,
    /// Full manifest candidate commit; `null` only without a manifest.
    pub candidate_commit: Option<String>,
    /// Blob ID of the candidate `proposal.md` at `candidate_commit`.
    pub proposal_content_id: Option<String>,
    /// The manifest's change-input digest.
    pub change_input_digest: Option<String>,
    /// Resolved executable of the deciding gate, or its unresolved first token
    /// for `tool_unavailable`.
    pub program: Option<String>,
    /// Number of arguments after the program for the deciding gate.
    pub argument_count: Option<u32>,
    /// The deciding gate's environment binding names. Never values.
    #[serde(default)]
    pub env_names: Vec<String>,
    pub remediation: Remediation,
    pub candidate_divergence: CandidateDivergence,
}

/// The typed shape of one settlement, before identities are attached.
///
/// Built by the executor from the typed result it is about to return, through
/// an exhaustive mapping, so [`ReasonCode`] is the whole vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementKind {
    pub reason: ReasonCode,
    pub reviewer_started: bool,
}

/// The repository facts one settlement is attributed to.
///
/// Borrowed from the executor's own boundary: the frozen manifest, its typed
/// gate outcomes, and the gate phase's verdict.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryFacts<'a> {
    pub manifest: &'a AcceptanceExecutionManifest,
    pub gates: &'a GateResults,
    pub verdict: &'a GateVerdict,
}

/// Repository identities resolved for one settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateIdentity {
    pub proposal_content_id: Option<String>,
    pub divergence: CandidateDivergence,
}

/// The repository-relative proposal path of a change.
pub fn proposal_path(change_id: &str) -> String {
    format!("openspec/changes/{change_id}/proposal.md")
}

/// Resolve `proposal_content_id` and `candidate_divergence` from the bound
/// review base only.
pub async fn resolve_candidate_identity<F: RepositoryFacts>(
    facts: &F,
    workspace: &Path,
    manifest: Option<&AcceptanceExecutionManifest>,
) -> CandidateIdentity {
    let Some(manifest) = manifest else {
        return CandidateIdentity {
            proposal_content_id: None,
            divergence: CandidateDivergence::unknown(DivergenceUnknownReason::NoManifest),
        };
    };
    let path = proposal_path(&manifest.change_id);
    let candidate_blob = facts
        .blob_at_commit(workspace, &manifest.candidate_commit_oid, &path)
        .await
        .ok();
    let base_ref = manifest.review_base_ref.clone();
    let base_commit =
        (!manifest.review_base_commit.is_empty()).then(|| manifest.review_base_commit.clone());
    let divergence = match (&base_ref, &base_commit) {
        (Some(_), Some(commit)) => {
            let source_blob = facts.blob_at_commit(workspace, commit, &path).await.ok();
            match (&source_blob, &candidate_blob) {
                (None, _) => CandidateDivergence {
                    reason: Some(DivergenceUnknownReason::SourceProposalMissing),
                    status: DivergenceStatus::Unknown,
                    source_ref: base_ref.clone(),
                    source_commit: base_commit.clone(),
                    source_blob: None,
                    candidate_blob: candidate_blob.clone(),
                },
                (Some(_), None) => CandidateDivergence {
                    reason: Some(DivergenceUnknownReason::CandidateProposalMissing),
                    status: DivergenceStatus::Unknown,
                    source_ref: base_ref.clone(),
                    source_commit: base_commit.clone(),
                    source_blob: source_blob.clone(),
                    candidate_blob: None,
                },
                (Some(source), Some(candidate)) => CandidateDivergence {
                    status: if source == candidate {
                        DivergenceStatus::Identical
                    } else {
                        DivergenceStatus::Diverged
                    },
                    source_ref: base_ref.clone(),
                    source_commit: base_commit.clone(),
                    source_blob: source_blob.clone(),
                    candidate_blob: candidate_blob.clone(),
                    reason: None,
                },
            }
        }
        _ => CandidateDivergence {
            candidate_blob: candidate_blob.clone(),
            source_ref: base_ref.clone(),
            ..CandidateDivergence::unknown(DivergenceUnknownReason::BaseUnresolvable)
        },
    };
    CandidateIdentity {
        proposal_content_id: candidate_blob,
        divergence,
    }
}

/// The gate a settlement is attributed to, read from typed outcomes only.
struct DecidingGate {
    verification_id: String,
    defect_code: Option<String>,
    gate: Option<ManifestGate>,
    unresolved: bool,
}

fn deciding_gate(reason: ReasonCode, boundary: &BoundaryFacts<'_>) -> Option<DecidingGate> {
    let find_gate = |id: &str| {
        boundary
            .manifest
            .gates
            .iter()
            .find(|gate| gate.verification_id == id)
            .cloned()
    };
    let from_outcome = |outcome: Option<&GateOutcome>, unresolved: bool| {
        outcome.map(|outcome| DecidingGate {
            verification_id: outcome.verification_id().to_string(),
            defect_code: None,
            gate: find_gate(outcome.verification_id()),
            unresolved,
        })
    };
    match reason {
        ReasonCode::DeclarationInvalid => {
            let defect = match boundary.verdict {
                GateVerdict::DeclarationInvalid { defects } => defects.first(),
                _ => boundary.manifest.declaration_defects.first(),
            }?;
            Some(DecidingGate {
                verification_id: defect.verification_id.clone(),
                defect_code: Some(defect.code.clone()),
                gate: None,
                unresolved: false,
            })
        }
        ReasonCode::GateFailed => from_outcome(boundary.gates.first_failure(), false),
        ReasonCode::ToolUnavailable => from_outcome(
            boundary.gates.outcomes.iter().find(|outcome| {
                matches!(outcome, GateOutcome::Refused { code, .. } if code == TOOL_UNAVAILABLE)
            }),
            true,
        ),
        ReasonCode::GateDeadlineExhausted => {
            from_outcome(boundary.gates.first_deadline_exhausted(), false)
        }
        // A runtime defect is a gate fact only when the gate phase itself
        // decided it; a defect in any other phase names no gate.
        ReasonCode::RuntimeDefect => match boundary.verdict {
            GateVerdict::Hold { .. } => from_outcome(boundary.gates.first_refusal(), false),
            _ => None,
        },
        ReasonCode::StatePathUnavailable
        | ReasonCode::LegacyTargetEvidence
        | ReasonCode::ManifestMalformed
        | ReasonCode::ReviewerPass
        | ReasonCode::ReviewerFail
        | ReasonCode::ReviewerStalled
        | ReasonCode::ReviewerPermissionStalled
        | ReasonCode::ReviewerCommandFailed
        | ReasonCode::ReviewerMissingVerdict
        | ReasonCode::ReviewDeadlineExhausted
        | ReasonCode::ReviewCleanupUnproven
        | ReasonCode::Cancelled => None,
    }
}

/// Build the record for one settlement.
///
/// Pure over its inputs: everything repository-observed arrives already
/// resolved in `identity`, so this can be exercised exhaustively without Git.
pub fn build_provenance(
    change_id: &str,
    boundary: Option<BoundaryFacts<'_>>,
    kind: SettlementKind,
    identity: CandidateIdentity,
) -> AcceptanceProvenance {
    let deciding = boundary
        .as_ref()
        .and_then(|boundary| deciding_gate(kind.reason, boundary));
    let (program, argument_count, env_names) = match deciding
        .as_ref()
        .and_then(|d| d.gate.as_ref().map(|gate| (gate, d.unresolved)))
    {
        Some((gate, unresolved)) => {
            let program = if unresolved || gate.tool.path.trim().is_empty() {
                gate.argv.first().cloned()
            } else {
                Some(gate.tool.path.clone())
            };
            (
                program,
                Some(u32::try_from(gate.argv.len().saturating_sub(1)).unwrap_or(u32::MAX)),
                gate.env_names.clone(),
            )
        }
        None => (None, None, Vec::new()),
    };
    let verification_id = deciding.as_ref().map(|d| d.verification_id.clone());
    let manifest = boundary.as_ref().map(|boundary| boundary.manifest);
    AcceptanceProvenance {
        decision_source: if kind.reviewer_started {
            DecisionSource::Reviewer
        } else {
            DecisionSource::NativeVerification
        },
        reviewer_started: kind.reviewer_started,
        reason_code: kind.reason,
        defect_code: deciding.as_ref().and_then(|d| d.defect_code.clone()),
        declaration_path: verification_id.as_ref().map(|_| proposal_path(change_id)),
        verification_id,
        candidate_commit: manifest.map(|manifest| manifest.candidate_commit_oid.clone()),
        proposal_content_id: identity.proposal_content_id,
        change_input_digest: manifest.map(|manifest| manifest.change_input_digest.clone()),
        program,
        argument_count,
        env_names,
        remediation: remediation_for(kind.reason, identity.divergence.status),
        candidate_divergence: identity.divergence,
    }
}

/// First twelve characters of an object ID, for operator display.
pub fn short_id(id: &str) -> &str {
    &id[..12.min(id.len())]
}

impl AcceptanceProvenance {
    /// The fixed operator-facing line set, shared by the TUI popup and its copy
    /// text so the two can never disagree.
    pub fn display_lines(&self) -> Vec<(&'static str, String)> {
        let decision = match self.decision_source {
            DecisionSource::NativeVerification if !self.reviewer_started => {
                "native_verification (reviewer not started)".to_string()
            }
            source => source.as_str().to_string(),
        };
        let reason = match &self.defect_code {
            Some(defect) => format!("{} ({defect})", self.reason_code.as_str()),
            None => self.reason_code.as_str().to_string(),
        };
        let verification = match (&self.verification_id, &self.declaration_path) {
            (Some(id), Some(path)) => format!("{id} in {path}"),
            (Some(id), None) => id.clone(),
            _ => "-".to_string(),
        };
        let candidate = format!(
            "{} proposal {}",
            self.candidate_commit.as_deref().map_or("-", short_id),
            self.proposal_content_id.as_deref().map_or("-", short_id)
        );
        let command = match &self.program {
            Some(program) => {
                let mut text = format!(
                    "{program}, {} arguments",
                    self.argument_count.unwrap_or_default()
                );
                if !self.env_names.is_empty() {
                    text.push_str(&format!(", env: {}", self.env_names.join(", ")));
                }
                text
            }
            None => "-".to_string(),
        };
        vec![
            ("Decision", decision),
            ("Reason", reason),
            ("Verification", verification),
            ("Candidate", candidate),
            ("Command", command),
            ("Next", self.remediation.as_str().to_string()),
            ("Source proposal", self.candidate_divergence.render()),
        ]
    }

    /// The one structured settlement log line: `key=value` pairs, every value an
    /// identity, a count, a name, or a closed token.
    pub fn log_line(&self, change_id: &str) -> String {
        let or_null = |value: Option<&str>| value.unwrap_or("null").to_string();
        format!(
            "acceptance_settlement change_id={change_id} decision_source={} \
             reviewer_started={} reason_code={} defect_code={} verification_id={} \
             candidate_commit={} proposal_content_id={} program={} argument_count={} \
             env_names=[{}] remediation={} divergence={}",
            self.decision_source.as_str(),
            self.reviewer_started,
            self.reason_code.as_str(),
            or_null(self.defect_code.as_deref()),
            or_null(self.verification_id.as_deref()),
            or_null(self.candidate_commit.as_deref()),
            or_null(self.proposal_content_id.as_deref()),
            or_null(self.program.as_deref()),
            self.argument_count
                .map_or_else(|| "null".to_string(), |count| count.to_string()),
            self.env_names.join(","),
            self.remediation.as_str(),
            self.candidate_divergence.status.as_str(),
        )
    }
}

/// A native `tool_unavailable` record for presentation tests elsewhere.
#[cfg(test)]
pub(crate) fn native_tool_unavailable_for_test(change_id: &str) -> AcceptanceProvenance {
    AcceptanceProvenance {
        decision_source: DecisionSource::NativeVerification,
        reviewer_started: false,
        reason_code: ReasonCode::ToolUnavailable,
        defect_code: None,
        verification_id: Some("focused-gate".to_string()),
        declaration_path: Some(proposal_path(change_id)),
        candidate_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
        proposal_content_id: Some("fedcba9876543210fedcba9876543210fedcba98".to_string()),
        change_input_digest: Some("digest".to_string()),
        program: Some("missing-tool".to_string()),
        argument_count: Some(2),
        env_names: vec!["API_TOKEN".to_string(), "RUST_LOG".to_string()],
        remediation: Remediation::SatisfyEnvironment,
        candidate_divergence: CandidateDivergence {
            status: DivergenceStatus::Diverged,
            source_ref: Some("main".to_string()),
            source_commit: Some("abcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string()),
            source_blob: Some("1111111111111111111111111111111111111111".to_string()),
            candidate_blob: Some("fedcba9876543210fedcba9876543210fedcba98".to_string()),
            reason: None,
        },
    }
}

#[cfg(test)]
#[path = "provenance/tests.rs"]
mod tests;
