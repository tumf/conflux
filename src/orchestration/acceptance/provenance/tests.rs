//! Unit regressions for the provenance producer.
//!
//! Every repository fact comes from an in-memory [`RepositoryFacts`] double, so
//! the remediation table, the deciding-gate selection, the divergence
//! comparison, and the redaction-by-construction claim are all checked without
//! Git, processes, or the filesystem.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;

use super::*;
use crate::orchestration::acceptance::execution_manifest::{SkillIdentity, MANIFEST_SCHEMA};
use crate::orchestration::acceptance::verification_evidence::{
    empty_env_binding_digest, DeclarationDefect, EnvBindings, EvidenceStore, ToolIdentity,
};

const CANDIDATE: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "2222222222222222222222222222222222222222";
const CANDIDATE_BLOB: &str = "3333333333333333333333333333333333333333";
const OTHER_BLOB: &str = "4444444444444444444444444444444444444444";
const ARG_SENTINEL: &str = "ARG-SENTINEL-c0ffee";
const ENV_SENTINEL: &str = "ENV-SENTINEL-deadbeef";

/// Blob IDs keyed by `commit:path`, and nothing else.
#[derive(Default)]
struct BlobFacts {
    blobs: HashMap<String, String>,
}

impl BlobFacts {
    fn with(mut self, commit: &str, change: &str, blob: &str) -> Self {
        self.blobs.insert(
            format!("{commit}:{}", proposal_path(change)),
            blob.to_string(),
        );
        self
    }
}

#[async_trait]
impl RepositoryFacts for BlobFacts {
    async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
        Ok(CANDIDATE.to_string())
    }
    async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
        Err("unused".to_string())
    }
    async fn tracked_blob_oid(&self, _workspace: &Path, _path: &str) -> Result<String, String> {
        Err("HEAD blobs must never be consulted for provenance".to_string())
    }
    async fn blob_at_commit(
        &self,
        _workspace: &Path,
        commit: &str,
        path: &str,
    ) -> Result<String, String> {
        self.blobs
            .get(&format!("{commit}:{path}"))
            .cloned()
            .ok_or_else(|| "missing".to_string())
    }
    async fn hash_file(&self, _workspace: &Path, _path: &Path) -> Result<String, String> {
        Err("unused".to_string())
    }
    async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
        Err("unused".to_string())
    }
    async fn resolve_tool(
        &self,
        _workspace: &Path,
        _program: &str,
    ) -> Result<ToolIdentity, String> {
        Err("unused".to_string())
    }
}

fn gate(id: &str, resolved: bool) -> ManifestGate {
    ManifestGate {
        verification_id: id.to_string(),
        argv: vec![
            "cargo".to_string(),
            "test".to_string(),
            format!("--token={ARG_SENTINEL}"),
        ],
        env_names: vec!["API_TOKEN".to_string(), "RUST_LOG".to_string()],
        env_binding_digest: empty_env_binding_digest(),
        env: EnvBindings::from_assignments([("API_TOKEN", ENV_SENTINEL), ("RUST_LOG", "debug")]),
        cwd: ".".to_string(),
        automation_path: "src/lib.rs".to_string(),
        automation_blob_oid: String::new(),
        tool: ToolIdentity {
            path: if resolved {
                "/usr/bin/cargo".to_string()
            } else {
                String::new()
            },
            executable_digest: String::new(),
            version: None,
        },
        artifact_path: EvidenceStore::artifact_relative_path(id),
        artifact_digest: None,
    }
}

fn manifest(change: &str, gates: Vec<ManifestGate>) -> AcceptanceExecutionManifest {
    AcceptanceExecutionManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        change_id: change.to_string(),
        candidate_commit_oid: CANDIDATE.to_string(),
        candidate_tree_oid: CANDIDATE.to_string(),
        review_base_commit: BASE.to_string(),
        review_range: format!("{BASE}..{CANDIDATE}"),
        review_base_ref: Some("main".to_string()),
        change_input_digest: "digest".to_string(),
        skill: SkillIdentity::default(),
        executable_identity: String::new(),
        gates,
        declaration_defects: Vec::new(),
        created_at: Utc::now(),
        absolute_deadline_secs: 3600,
        work_budget_secs: 3570,
    }
}

fn identity() -> CandidateIdentity {
    CandidateIdentity {
        proposal_content_id: Some(CANDIDATE_BLOB.to_string()),
        divergence: CandidateDivergence {
            status: DivergenceStatus::Identical,
            source_ref: Some("main".to_string()),
            source_commit: Some(BASE.to_string()),
            source_blob: Some(CANDIDATE_BLOB.to_string()),
            candidate_blob: Some(CANDIDATE_BLOB.to_string()),
            reason: None,
        },
    }
}

fn kind(reason: ReasonCode, reviewer_started: bool) -> SettlementKind {
    SettlementKind {
        reason,
        reviewer_started,
    }
}

fn assert_redacted(provenance: &AcceptanceProvenance) {
    let json = serde_json::to_string(provenance).expect("serializes");
    let lines = provenance
        .display_lines()
        .into_iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    let log = provenance.log_line("alpha");
    for surface in [&json, &lines, &log] {
        assert!(!surface.contains(ARG_SENTINEL), "{surface}");
        assert!(!surface.contains(ENV_SENTINEL), "{surface}");
    }
}

#[test]
fn acceptance_provenance_remediation_table_is_pure_and_total() {
    use DivergenceStatus::{Diverged, Identical, Unknown};
    for divergence in [Identical, Diverged, Unknown] {
        for reason in ReasonCode::ALL {
            let expected = match reason {
                ReasonCode::DeclarationInvalid if divergence == Diverged => {
                    Remediation::RefreshCandidate
                }
                ReasonCode::DeclarationInvalid => Remediation::FixDeclaration,
                ReasonCode::ToolUnavailable => Remediation::SatisfyEnvironment,
                ReasonCode::GateFailed | ReasonCode::ReviewerFail => Remediation::RepairCandidate,
                ReasonCode::ReviewerPass => Remediation::None,
                _ => Remediation::Retry,
            };
            assert_eq!(remediation_for(reason, divergence), expected, "{reason:?}");
        }
    }
}

#[test]
fn acceptance_provenance_tokens_serialize_as_their_documented_strings() {
    for reason in ReasonCode::ALL {
        assert_eq!(
            serde_json::to_value(reason).unwrap(),
            serde_json::Value::String(reason.as_str().to_string())
        );
    }
    for remediation in Remediation::ALL {
        assert_eq!(
            serde_json::to_value(remediation).unwrap(),
            serde_json::Value::String(remediation.as_str().to_string())
        );
    }
    assert_eq!(
        serde_json::to_value(DecisionSource::NativeVerification).unwrap(),
        "native_verification"
    );
}

#[test]
fn acceptance_provenance_declaration_invalid_names_the_defect_without_a_reviewer() {
    let mut manifest = manifest("alpha", Vec::new());
    let defect = DeclarationDefect {
        verification_id: "shell-gate".to_string(),
        position: 2,
        field: "rerun".to_string(),
        code: "declaration_shell_syntax".to_string(),
        detail: "uses shell syntax".to_string(),
    };
    manifest.declaration_defects = vec![defect.clone()];
    let verdict = GateVerdict::DeclarationInvalid {
        defects: vec![defect],
    };
    let gates = GateResults::default();
    let provenance = build_provenance(
        "alpha",
        Some(BoundaryFacts {
            manifest: &manifest,
            gates: &gates,
            verdict: &verdict,
        }),
        kind(ReasonCode::DeclarationInvalid, false),
        identity(),
    );

    assert_eq!(
        provenance.decision_source,
        DecisionSource::NativeVerification
    );
    assert!(!provenance.reviewer_started);
    assert_eq!(
        provenance.defect_code.as_deref(),
        Some("declaration_shell_syntax")
    );
    assert_eq!(provenance.verification_id.as_deref(), Some("shell-gate"));
    assert_eq!(
        provenance.declaration_path.as_deref(),
        Some("openspec/changes/alpha/proposal.md")
    );
    assert_eq!(provenance.candidate_commit.as_deref(), Some(CANDIDATE));
    assert_eq!(provenance.change_input_digest.as_deref(), Some("digest"));
    assert_eq!(provenance.remediation, Remediation::FixDeclaration);
    assert!(provenance.program.is_none());
}

#[test]
fn acceptance_provenance_tool_unavailable_names_the_unresolved_token() {
    let manifest = manifest("alpha", vec![gate("missing-tool", false)]);
    let gates = GateResults {
        outcomes: vec![GateOutcome::Refused {
            verification_id: "missing-tool".to_string(),
            code: TOOL_UNAVAILABLE.to_string(),
            detail: "not on PATH".to_string(),
        }],
    };
    let verdict = super::super::gate_execution::classify_boundary(&manifest, &gates);
    let provenance = build_provenance(
        "alpha",
        Some(BoundaryFacts {
            manifest: &manifest,
            gates: &gates,
            verdict: &verdict,
        }),
        kind(ReasonCode::ToolUnavailable, false),
        identity(),
    );

    assert_eq!(provenance.program.as_deref(), Some("cargo"));
    assert_eq!(provenance.argument_count, Some(2));
    assert_eq!(provenance.remediation, Remediation::SatisfyEnvironment);
    assert_eq!(provenance.verification_id.as_deref(), Some("missing-tool"));
    assert_redacted(&provenance);
}

#[test]
fn acceptance_provenance_failed_gate_carries_counts_and_names_never_values() {
    let manifest = manifest("alpha", vec![gate("focused", true)]);
    let gates = GateResults {
        outcomes: vec![GateOutcome::Failed {
            verification_id: "focused".to_string(),
            exit_code: 1,
            artifact_path: "gates/focused/output.log".to_string(),
        }],
    };
    let verdict = super::super::gate_execution::classify_boundary(&manifest, &gates);
    let provenance = build_provenance(
        "alpha",
        Some(BoundaryFacts {
            manifest: &manifest,
            gates: &gates,
            verdict: &verdict,
        }),
        kind(ReasonCode::GateFailed, false),
        identity(),
    );

    assert_eq!(provenance.program.as_deref(), Some("/usr/bin/cargo"));
    assert_eq!(provenance.argument_count, Some(2));
    assert_eq!(provenance.env_names, vec!["API_TOKEN", "RUST_LOG"]);
    assert_eq!(provenance.remediation, Remediation::RepairCandidate);
    assert_redacted(&provenance);
    let log = provenance.log_line("alpha");
    assert!(log.contains("program=/usr/bin/cargo"), "{log}");
    assert!(log.contains("argument_count=2"), "{log}");
    assert!(log.contains("env_names=[API_TOKEN,RUST_LOG]"), "{log}");
}

#[test]
fn acceptance_provenance_reviewer_verdicts_name_no_gate() {
    let manifest = manifest("alpha", vec![gate("focused", true)]);
    let gates = GateResults {
        outcomes: vec![GateOutcome::Executed {
            verification_id: "focused".to_string(),
            artifact_path: "a".to_string(),
            elapsed_seconds: 1,
        }],
    };
    let verdict = GateVerdict::Proceed;
    for (reason, remediation) in [
        (ReasonCode::ReviewerFail, Remediation::RepairCandidate),
        (ReasonCode::ReviewerPass, Remediation::None),
    ] {
        let provenance = build_provenance(
            "alpha",
            Some(BoundaryFacts {
                manifest: &manifest,
                gates: &gates,
                verdict: &verdict,
            }),
            kind(reason, true),
            identity(),
        );
        assert_eq!(provenance.decision_source, DecisionSource::Reviewer);
        assert!(provenance.reviewer_started);
        assert!(provenance.verification_id.is_none());
        assert!(provenance.declaration_path.is_none());
        assert!(provenance.program.is_none());
        assert_eq!(provenance.remediation, remediation);
    }
}

#[test]
fn acceptance_provenance_without_a_manifest_is_unknown_and_unattributed() {
    let provenance = build_provenance(
        "alpha",
        None,
        kind(ReasonCode::StatePathUnavailable, false),
        CandidateIdentity {
            proposal_content_id: None,
            divergence: CandidateDivergence::unknown(DivergenceUnknownReason::NoManifest),
        },
    );
    assert!(provenance.candidate_commit.is_none());
    assert!(provenance.change_input_digest.is_none());
    assert_eq!(provenance.remediation, Remediation::Retry);
    assert_eq!(
        provenance.candidate_divergence.render(),
        "unknown (no_manifest)"
    );
}

#[tokio::test]
async fn acceptance_provenance_divergence_compares_the_bound_base_only() {
    let workspace = Path::new("/nonexistent");
    let manifest = manifest("alpha", Vec::new());

    let diverged = BlobFacts::default()
        .with(CANDIDATE, "alpha", CANDIDATE_BLOB)
        .with(BASE, "alpha", OTHER_BLOB);
    let identity = resolve_candidate_identity(&diverged, workspace, Some(&manifest)).await;
    assert_eq!(
        identity.proposal_content_id.as_deref(),
        Some(CANDIDATE_BLOB)
    );
    assert_eq!(identity.divergence.status, DivergenceStatus::Diverged);
    assert_eq!(identity.divergence.source_ref.as_deref(), Some("main"));
    assert_eq!(identity.divergence.source_commit.as_deref(), Some(BASE));
    assert_eq!(identity.divergence.source_blob.as_deref(), Some(OTHER_BLOB));
    assert_eq!(
        identity.divergence.candidate_blob.as_deref(),
        Some(CANDIDATE_BLOB)
    );
    assert_eq!(
        identity.divergence.render(),
        format!("diverged from main@{}", &BASE[..12])
    );
    assert_eq!(
        remediation_for(ReasonCode::DeclarationInvalid, identity.divergence.status),
        Remediation::RefreshCandidate
    );

    let identical = BlobFacts::default()
        .with(CANDIDATE, "alpha", CANDIDATE_BLOB)
        .with(BASE, "alpha", CANDIDATE_BLOB);
    let identity = resolve_candidate_identity(&identical, workspace, Some(&manifest)).await;
    assert_eq!(identity.divergence.status, DivergenceStatus::Identical);
    assert_eq!(identity.divergence.render(), "identical");

    let source_missing = BlobFacts::default().with(CANDIDATE, "alpha", CANDIDATE_BLOB);
    let identity = resolve_candidate_identity(&source_missing, workspace, Some(&manifest)).await;
    assert_eq!(
        identity.divergence.reason,
        Some(DivergenceUnknownReason::SourceProposalMissing)
    );

    let candidate_missing = BlobFacts::default().with(BASE, "alpha", CANDIDATE_BLOB);
    let identity = resolve_candidate_identity(&candidate_missing, workspace, Some(&manifest)).await;
    assert!(identity.proposal_content_id.is_none());
    assert_eq!(
        identity.divergence.reason,
        Some(DivergenceUnknownReason::CandidateProposalMissing)
    );

    let mut unbound = manifest.clone();
    unbound.review_base_commit = String::new();
    let identity = resolve_candidate_identity(&identical, workspace, Some(&unbound)).await;
    assert_eq!(identity.divergence.status, DivergenceStatus::Unknown);
    assert_eq!(
        identity.divergence.reason,
        Some(DivergenceUnknownReason::BaseUnresolvable)
    );
    assert_eq!(
        identity.proposal_content_id.as_deref(),
        Some(CANDIDATE_BLOB),
        "the candidate identity does not depend on a base"
    );

    let identity = resolve_candidate_identity(&identical, workspace, None).await;
    assert_eq!(
        identity.divergence.reason,
        Some(DivergenceUnknownReason::NoManifest)
    );
    assert!(identity.proposal_content_id.is_none());
}

#[test]
fn acceptance_provenance_display_lines_are_fixed_and_ordered() {
    let manifest = manifest("alpha", vec![gate("focused", true)]);
    let gates = GateResults {
        outcomes: vec![GateOutcome::Failed {
            verification_id: "focused".to_string(),
            exit_code: 1,
            artifact_path: "a".to_string(),
        }],
    };
    let verdict = GateVerdict::Failed {
        evidence: Vec::new(),
    };
    let provenance = build_provenance(
        "alpha",
        Some(BoundaryFacts {
            manifest: &manifest,
            gates: &gates,
            verdict: &verdict,
        }),
        kind(ReasonCode::GateFailed, false),
        identity(),
    );
    let lines = provenance.display_lines();
    let labels: Vec<_> = lines.iter().map(|(label, _)| *label).collect();
    assert_eq!(
        labels,
        [
            "Decision",
            "Reason",
            "Verification",
            "Candidate",
            "Command",
            "Next",
            "Source proposal"
        ]
    );
    assert_eq!(lines[0].1, "native_verification (reviewer not started)");
    assert_eq!(
        lines[3].1,
        format!("{} proposal {}", &CANDIDATE[..12], &CANDIDATE_BLOB[..12])
    );
    assert_eq!(
        lines[4].1,
        "/usr/bin/cargo, 2 arguments, env: API_TOKEN, RUST_LOG"
    );
    assert_eq!(lines[5].1, "repair_candidate");
}
