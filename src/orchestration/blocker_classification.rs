//! Orchestrator-owned lifecycle classification for execution holds.
//!
//! Conflux — not an individual agent — decides whether a change is `blocked` or
//! `stalled`. Apply and Acceptance report structured facts; this module is the
//! single boundary that validates those facts and turns them into a lifecycle
//! classification, so equivalent observations classify identically in every
//! mode and on every surface.
//!
//! Two independent concepts:
//!
//! - **lifecycle status**: `blocked` (useful execution is ineligible because a
//!   named prerequisite has not changed) or `stalled` (automatic execution
//!   stopped after no semantic progress, repeated findings, or an exhausted
//!   retry/repair budget);
//! - **blocker kind**: dependency or external, which is what keeps the two
//!   flavours of `blocked` distinguishable.
//!
//! A compatibility verdict token (`gated`, legacy `blocked`) is transport
//! syntax. It is accepted as input, but it never determines classification on
//! its own: only a complete, validated structured payload becomes external
//! `blocked`.
//!
//! Everything here is a pure function over reported facts. No filesystem, no
//! git, no durable state — the classification lives in in-memory reducer state
//! for one process lifetime only.

use crate::acceptance::SUPPORTED_BLOCKER_CATEGORIES;
use crate::runtime::proposal::{BlockerOrigin, ExternalBlockerInfo};

/// A claim that a non-repository prerequisite prevents useful execution.
///
/// "Claim" is deliberate: constructing one asserts nothing. Only
/// [`classify_execution_hold`] can promote a claim to a validated
/// [`ExternalBlockerInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBlockerClaim {
    /// Phase that observed the prerequisite.
    pub origin: BlockerOrigin,
    /// Category the reporter selected explicitly.
    pub category: String,
    /// Concrete evidence entries.
    pub evidence: Vec<String>,
    /// Owning team/role or named prerequisite, when supplied.
    pub prerequisite_owner: Option<String>,
    /// Verifiable condition that clears the wait.
    pub unblock_condition: Option<String>,
    /// Operator-facing action.
    pub next_action: String,
    /// Whether execution can resume once the prerequisite is satisfied.
    pub resumable: bool,
}

/// Why automatic execution stopped without a valid external prerequisite.
///
/// These are the runtime's own judgements about its execution budget. They are
/// never external waits, so no category is invented for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStopReason {
    /// The same finding came back after a repair attempt.
    RepeatedFinding,
    /// A cycle produced no semantic progress.
    NoSemanticProgress,
    /// The retry or repair budget is spent.
    RetryExhausted,
    /// A repeated unresolved permission/tool-policy denial.
    PermissionDenial,
}

impl ExecutionStopReason {
    /// Stable machine-readable reason recorded in stalled metadata.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepeatedFinding => "repeated_acceptance_findings",
            Self::NoSemanticProgress => "no_semantic_progress",
            Self::RetryExhausted => "retry_budget_exhausted",
            Self::PermissionDenial => "permission_denial",
        }
    }
}

/// What an execution phase actually reported, before classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionHold {
    /// A structured external prerequisite claim awaiting validation.
    ExternalPrerequisiteClaim(ExternalBlockerClaim),
    /// The runtime stopped automatic execution on its own judgement.
    ExecutionStopped {
        reason: ExecutionStopReason,
        detail: String,
    },
    /// A bare `gated`/legacy `blocked` token with no usable structured payload.
    BareCompatibilityVerdict { token: String, detail: String },
}

/// Why a claim failed validation. Every variant keeps the hold off the external
/// `blocked` path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimRejection {
    /// Category absent or blank.
    MissingCategory,
    /// Category is not one the runtime supports.
    UnsupportedCategory(String),
    /// No concrete evidence entry survived trimming.
    EmptyEvidence,
    /// No verifiable unblock condition was supplied.
    MissingUnblockCondition,
    /// No next action was supplied.
    MissingNextAction,
}

impl ClaimRejection {
    /// Operator-facing reason used in protocol-correction diagnostics.
    pub fn reason(&self) -> String {
        match self {
            Self::MissingCategory => "external blocker claim has no explicit category".to_string(),
            Self::UnsupportedCategory(category) => format!(
                "external blocker category '{category}' is not one of: {}",
                SUPPORTED_BLOCKER_CATEGORIES.join(", ")
            ),
            Self::EmptyEvidence => {
                "external blocker claim has no concrete evidence entries".to_string()
            }
            Self::MissingUnblockCondition => {
                "external blocker claim has no verifiable unblock condition".to_string()
            }
            Self::MissingNextAction => "external blocker claim has no next action".to_string(),
        }
    }
}

/// The orchestrator's lifecycle decision for one execution hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleClassification {
    /// A validated non-repository prerequisite: lifecycle `blocked`, blocker
    /// kind external.
    ExternalBlocked(ExternalBlockerInfo),
    /// Automatic execution stopped: lifecycle `stalled`, no blocker kind.
    Stalled { reason: String, detail: String },
    /// Not enough valid evidence to classify. Bounded protocol correction owns
    /// this; neither `blocked` nor `stalled` is set.
    ProtocolCorrection { reason: String },
}

impl LifecycleClassification {
    /// Operator-facing lifecycle word, or `None` while protocol correction is
    /// still deciding.
    pub fn display_status(&self) -> Option<&'static str> {
        match self {
            Self::ExternalBlocked(_) => Some("blocked"),
            Self::Stalled { .. } => Some("stalled"),
            Self::ProtocolCorrection { .. } => None,
        }
    }
}

/// Validate an external prerequisite claim.
///
/// Structural and explicit only: the category must be supported, the evidence
/// concrete, and both a verifiable unblock condition and a next action present.
/// Nothing is inferred from prose, and a repository-fixable claim can never pass
/// because no repository-fixable category is supported.
pub fn validate_external_claim(
    claim: &ExternalBlockerClaim,
) -> std::result::Result<ExternalBlockerInfo, ClaimRejection> {
    let category = claim.category.trim();
    if category.is_empty() {
        return Err(ClaimRejection::MissingCategory);
    }
    if !SUPPORTED_BLOCKER_CATEGORIES.contains(&category) {
        return Err(ClaimRejection::UnsupportedCategory(category.to_string()));
    }

    let evidence = claim
        .evidence
        .iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    if evidence.is_empty() {
        return Err(ClaimRejection::EmptyEvidence);
    }

    let unblock_condition = claim
        .unblock_condition
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ClaimRejection::MissingUnblockCondition)?;

    let next_action = claim.next_action.trim();
    if next_action.is_empty() {
        return Err(ClaimRejection::MissingNextAction);
    }

    Ok(ExternalBlockerInfo {
        origin: claim.origin,
        category: category.to_string(),
        evidence,
        prerequisite_owner: claim
            .prerequisite_owner
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        unblock_condition: unblock_condition.to_string(),
        next_action: next_action.to_string(),
        resumable: claim.resumable,
    })
}

/// Classify one execution hold into a canonical lifecycle status.
///
/// This is the only place the `blocked` versus `stalled` decision is made.
pub fn classify_execution_hold(hold: &ExecutionHold) -> LifecycleClassification {
    match hold {
        ExecutionHold::ExternalPrerequisiteClaim(claim) => match validate_external_claim(claim) {
            Ok(blocker) => LifecycleClassification::ExternalBlocked(blocker),
            // An incomplete claim is a protocol problem, not evidence of either
            // lifecycle state. Guessing `stalled` here would let a malformed
            // payload silently stop a change that only needed a corrective
            // re-invocation.
            Err(rejection) => LifecycleClassification::ProtocolCorrection {
                reason: rejection.reason(),
            },
        },
        ExecutionHold::ExecutionStopped { reason, detail } => LifecycleClassification::Stalled {
            reason: reason.as_str().to_string(),
            detail: detail.clone(),
        },
        ExecutionHold::BareCompatibilityVerdict { token, detail } => {
            LifecycleClassification::ProtocolCorrection {
                reason: format!(
                    "compatibility verdict token '{token}' carries no structured blocker facts: \
                     {detail}"
                ),
            }
        }
    }
}

/// Build an external prerequisite claim from reported blocker facts.
///
/// Facts arrive as [`crate::events::StalledBlocker`] from both Apply and
/// Acceptance. The phase string chooses the origin; anything that is not
/// `apply` is attributed to acceptance, which is the only other phase that
/// reports blocker facts.
pub fn claim_from_reported_facts(blocker: &crate::events::StalledBlocker) -> ExternalBlockerClaim {
    ExternalBlockerClaim {
        origin: if blocker.phase.trim().eq_ignore_ascii_case("apply") {
            BlockerOrigin::Apply
        } else {
            BlockerOrigin::Acceptance
        },
        category: blocker.category.clone(),
        evidence: blocker.evidence.clone(),
        prerequisite_owner: blocker.prerequisite_owner.clone(),
        unblock_condition: blocker.unblock_condition.clone(),
        next_action: blocker.next_action.clone(),
        resumable: blocker.resumable,
    }
}

/// Classify reported blocker facts, falling back to `stalled` when the facts do
/// not qualify as an external prerequisite.
///
/// The reducer uses this when it must land on a lifecycle state immediately —
/// the phase has already finished and there is no corrective re-invocation left
/// to make. `stalled` is the conservative answer: it keeps the change out of
/// ordinary dispatch and visible to an operator without claiming a prerequisite
/// nobody verified.
pub fn classify_reported_facts(blocker: &crate::events::StalledBlocker) -> LifecycleClassification {
    let hold = ExecutionHold::ExternalPrerequisiteClaim(claim_from_reported_facts(blocker));
    match classify_execution_hold(&hold) {
        LifecycleClassification::ProtocolCorrection { reason } => {
            LifecycleClassification::Stalled {
                reason: format!("unvalidated_external_claim:{}", blocker.category),
                detail: format!("{reason}; {}", blocker.summary()),
            }
        }
        classified => classified,
    }
}

/// Classify an execution stop the runtime decided on its own.
///
/// Kept behind the same classifier as reported claims so a stop can never
/// acquire an external category, and so both serial and parallel produce
/// identical stalled metadata for the same judgement.
pub fn classify_execution_stop(
    reason: ExecutionStopReason,
    detail: impl Into<String>,
) -> LifecycleClassification {
    classify_execution_hold(&ExecutionHold::ExecutionStopped {
        reason,
        detail: detail.into(),
    })
}

/// Operator-facing diagnostic for a bare compatibility verdict token.
///
/// Routed through the classifier so the "token spelling alone sets neither
/// `blocked` nor `stalled`" rule has exactly one implementation, and so the
/// message an operator reads is the same one the classifier produced.
pub fn bare_compatibility_diagnostic(token: &str, detail: impl Into<String>) -> String {
    match classify_execution_hold(&ExecutionHold::BareCompatibilityVerdict {
        token: token.to_string(),
        detail: detail.into(),
    }) {
        LifecycleClassification::ProtocolCorrection { reason } => reason,
        other => unreachable!("a bare compatibility token must need correction, got {other:?}"),
    }
}

/// Whether reported facts describe a repeated permission/tool-policy denial.
///
/// Keyed on the structured gate the permission classifier sets, never on
/// narrative words in the summary.
pub fn is_permission_denial(blocker: &crate::events::StalledBlocker) -> bool {
    blocker.gate == "permission_policy"
}

/// Map an acceptance retry-policy stop reason onto a classifier stop reason.
///
/// Unrecognized reasons fall back to "no semantic progress", which is the
/// conservative reading: automation stopped and an operator must look.
pub fn execution_stop_reason_for(retry_reason: &str) -> ExecutionStopReason {
    match retry_reason {
        "repeated_acceptance_findings" => ExecutionStopReason::RepeatedFinding,
        "acceptance_cycle_limit_exhausted" => ExecutionStopReason::RetryExhausted,
        _ => ExecutionStopReason::NoSemanticProgress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim() -> ExternalBlockerClaim {
        ExternalBlockerClaim {
            origin: BlockerOrigin::Acceptance,
            category: "credential".to_string(),
            evidence: vec!["STAGING_API_KEY is unset".to_string()],
            prerequisite_owner: Some("platform".to_string()),
            unblock_condition: Some("STAGING_API_KEY is present in CI".to_string()),
            next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
            resumable: true,
        }
    }

    /// One table drives every classification case the spec names: Apply,
    /// Acceptance, legacy token, missing fields, repository-fixable claims,
    /// repeated findings, and retry exhaustion.
    #[test]
    fn classification_table_covers_every_declared_hold() {
        let repository_fixable = ExternalBlockerClaim {
            category: "missing_test_coverage".to_string(),
            ..claim()
        };
        let cases: Vec<(&str, ExecutionHold, Option<&str>)> = vec![
            (
                "acceptance external prerequisite",
                ExecutionHold::ExternalPrerequisiteClaim(claim()),
                Some("blocked"),
            ),
            (
                "apply external prerequisite",
                ExecutionHold::ExternalPrerequisiteClaim(ExternalBlockerClaim {
                    origin: BlockerOrigin::Apply,
                    category: "infrastructure".to_string(),
                    evidence: vec!["docker daemon unavailable".to_string()],
                    unblock_condition: Some("docker daemon accepts connections".to_string()),
                    ..claim()
                }),
                Some("blocked"),
            ),
            (
                "bare legacy blocked token",
                ExecutionHold::BareCompatibilityVerdict {
                    token: "blocked".to_string(),
                    detail: "no blocker object".to_string(),
                },
                None,
            ),
            (
                "bare gated token",
                ExecutionHold::BareCompatibilityVerdict {
                    token: "gated".to_string(),
                    detail: "no blocker object".to_string(),
                },
                None,
            ),
            (
                "missing unblock condition",
                ExecutionHold::ExternalPrerequisiteClaim(ExternalBlockerClaim {
                    unblock_condition: None,
                    ..claim()
                }),
                None,
            ),
            (
                "missing evidence",
                ExecutionHold::ExternalPrerequisiteClaim(ExternalBlockerClaim {
                    evidence: vec!["   ".to_string()],
                    ..claim()
                }),
                None,
            ),
            (
                "missing next action",
                ExecutionHold::ExternalPrerequisiteClaim(ExternalBlockerClaim {
                    next_action: "  ".to_string(),
                    ..claim()
                }),
                None,
            ),
            (
                "repository-fixable claim",
                ExecutionHold::ExternalPrerequisiteClaim(repository_fixable),
                None,
            ),
            (
                "repeated acceptance finding",
                ExecutionHold::ExecutionStopped {
                    reason: ExecutionStopReason::RepeatedFinding,
                    detail: "same finding id returned".to_string(),
                },
                Some("stalled"),
            ),
            (
                "no semantic progress",
                ExecutionHold::ExecutionStopped {
                    reason: ExecutionStopReason::NoSemanticProgress,
                    detail: "identical fingerprint".to_string(),
                },
                Some("stalled"),
            ),
            (
                "retry exhaustion",
                ExecutionHold::ExecutionStopped {
                    reason: ExecutionStopReason::RetryExhausted,
                    detail: "cycle limit reached".to_string(),
                },
                Some("stalled"),
            ),
            (
                "permission denial",
                ExecutionHold::ExecutionStopped {
                    reason: ExecutionStopReason::PermissionDenial,
                    detail: "denied write to /etc".to_string(),
                },
                Some("stalled"),
            ),
        ];

        for (label, hold, expected) in cases {
            let classification = classify_execution_hold(&hold);
            assert_eq!(
                classification.display_status(),
                expected,
                "{label} classified as {classification:?}"
            );
            // No execution stop may ever masquerade as an external wait.
            if matches!(hold, ExecutionHold::ExecutionStopped { .. }) {
                assert!(
                    !matches!(classification, LifecycleClassification::ExternalBlocked(_)),
                    "{label} must not become an external blocker"
                );
            }
        }
    }

    /// A validated claim preserves every operator-facing field verbatim,
    /// including the origin, and never re-derives the category from prose.
    #[test]
    fn validated_claim_preserves_origin_and_fields_verbatim() {
        let prose_heavy = ExternalBlockerClaim {
            category: "human_decision".to_string(),
            evidence: vec!["  needs a credential token auth decision  ".to_string()],
            ..claim()
        };
        let info = validate_external_claim(&prose_heavy).unwrap();

        assert_eq!(info.category, "human_decision");
        assert_eq!(info.origin, BlockerOrigin::Acceptance);
        assert_eq!(info.evidence, ["needs a credential token auth decision"]);
        assert_eq!(info.prerequisite_owner.as_deref(), Some("platform"));
        assert_eq!(info.unblock_condition, "STAGING_API_KEY is present in CI");
        assert!(info.resumable);
    }

    #[test]
    fn claim_rejections_name_the_missing_field() {
        for (mutated, expected) in [
            (
                ExternalBlockerClaim {
                    category: "  ".to_string(),
                    ..claim()
                },
                ClaimRejection::MissingCategory,
            ),
            (
                ExternalBlockerClaim {
                    category: "flaky_test".to_string(),
                    ..claim()
                },
                ClaimRejection::UnsupportedCategory("flaky_test".to_string()),
            ),
            (
                ExternalBlockerClaim {
                    evidence: Vec::new(),
                    ..claim()
                },
                ClaimRejection::EmptyEvidence,
            ),
            (
                ExternalBlockerClaim {
                    unblock_condition: Some("   ".to_string()),
                    ..claim()
                },
                ClaimRejection::MissingUnblockCondition,
            ),
            (
                ExternalBlockerClaim {
                    next_action: String::new(),
                    ..claim()
                },
                ClaimRejection::MissingNextAction,
            ),
        ] {
            let rejection = validate_external_claim(&mutated).unwrap_err();
            assert_eq!(rejection, expected);
            assert!(!rejection.reason().is_empty());
        }
    }

    /// Reported facts that fail validation land on `stalled`, never on external
    /// `blocked`, and the stall detail keeps the reason so an operator can see
    /// what was missing.
    #[test]
    fn reported_facts_without_unblock_condition_classify_as_stalled() {
        let facts = crate::events::StalledBlocker {
            category: "credential".to_string(),
            phase: "acceptance".to_string(),
            gate: "acceptance".to_string(),
            error_summary: "missing key".to_string(),
            evidence: vec!["STAGING_API_KEY is unset".to_string()],
            unblock_condition: None,
            prerequisite_owner: None,
            next_action: "provision the key".to_string(),
            resumable: true,
            worktree_preserved: true,
        };

        match classify_reported_facts(&facts) {
            LifecycleClassification::Stalled { reason, detail } => {
                assert!(reason.starts_with("unvalidated_external_claim:"));
                assert!(detail.contains("unblock condition"));
            }
            other => panic!("expected stalled, got {other:?}"),
        }

        let complete = crate::events::StalledBlocker {
            unblock_condition: Some("STAGING_API_KEY is present in CI".to_string()),
            prerequisite_owner: Some("platform".to_string()),
            ..facts
        };
        match classify_reported_facts(&complete) {
            LifecycleClassification::ExternalBlocked(info) => {
                assert_eq!(info.origin, BlockerOrigin::Acceptance);
                assert_eq!(info.category, "credential");
            }
            other => panic!("expected external blocked, got {other:?}"),
        }
    }

    /// The reporting phase decides the origin; a permission-denial payload from
    /// apply is attributed to apply even though it never validates.
    #[test]
    fn origin_is_taken_from_the_reporting_phase() {
        let apply_facts = crate::events::StalledBlocker {
            category: "infrastructure".to_string(),
            phase: "apply".to_string(),
            gate: "apply".to_string(),
            error_summary: "docker unavailable".to_string(),
            evidence: vec!["cannot connect to the docker daemon".to_string()],
            unblock_condition: Some("the docker daemon accepts connections".to_string()),
            prerequisite_owner: None,
            next_action: "start docker then retry apply".to_string(),
            resumable: true,
            worktree_preserved: true,
        };

        assert_eq!(
            claim_from_reported_facts(&apply_facts).origin,
            BlockerOrigin::Apply
        );
        match classify_reported_facts(&apply_facts) {
            LifecycleClassification::ExternalBlocked(info) => {
                assert_eq!(info.origin, BlockerOrigin::Apply);
            }
            other => panic!("expected external blocked, got {other:?}"),
        }
    }
}
