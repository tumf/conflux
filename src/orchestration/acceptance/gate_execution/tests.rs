//! Runtime-owned gate execution regressions.
//!
//! The fakes here are in-memory doubles for the repository, the supervisor, and
//! the clock: no real Git, no real process, no real filesystem state beyond a
//! temporary directory the evidence store owns. That keeps these unit-scoped and
//! fast while still driving every branch the boundary can take.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};

use super::*;
use crate::orchestration::acceptance::execution_manifest::{
    AcceptanceExecutionManifest, AcceptanceHoldCategory, ManifestGate, SkillIdentity,
    MANIFEST_SCHEMA,
};
use crate::orchestration::acceptance::verification_evidence::{
    RerunReason, ReuseDecision, SupervisedOutcome, ToolIdentity, VerificationEvidence,
    EVIDENCE_AUTHORITY, EVIDENCE_SCHEMA,
};

const COMMIT: &str = "1111111111111111111111111111111111111111";
const TREE: &str = "2222222222222222222222222222222222222222";
const BLOB: &str = "3333333333333333333333333333333333333333";
const TOOL_DIGEST: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const ARTIFACT_DIGEST: &str = "5555555555555555555555555555555555555555555555555555555555555555";

fn tool() -> ToolIdentity {
    ToolIdentity {
        path: "/usr/bin/cargo".to_string(),
        executable_digest: TOOL_DIGEST.to_string(),
        version: Some("cargo 1.80.0".to_string()),
    }
}

fn gate(id: &str) -> ManifestGate {
    ManifestGate {
        verification_id: id.to_string(),
        argv: vec!["cargo".to_string(), "test".to_string(), id.to_string()],
        cwd: ".".to_string(),
        automation_path: "src/lib.rs".to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        artifact_path: format!(".cflx/verification-evidence/{id}.log"),
        artifact_digest: Some(ARTIFACT_DIGEST.to_string()),
    }
}

fn manifest(gates: Vec<ManifestGate>) -> AcceptanceExecutionManifest {
    AcceptanceExecutionManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        change_id: "alpha".to_string(),
        candidate_commit_oid: COMMIT.to_string(),
        candidate_tree_oid: TREE.to_string(),
        review_base_commit: BLOB.to_string(),
        review_range: format!("{BLOB}..{COMMIT}"),
        review_base_ref: Some("main".to_string()),
        change_input_digest: ARTIFACT_DIGEST.to_string(),
        skill: SkillIdentity {
            name: "cflx-accept".to_string(),
            digest: TOOL_DIGEST.to_string(),
        },
        executable_identity: TOOL_DIGEST.to_string(),
        gates,
        created_at: Utc::now(),
        absolute_deadline_secs: 3600,
        work_budget_secs: 3570,
        terminal: None,
    }
}

#[derive(Clone)]
struct FakeFacts {
    blobs: HashMap<String, String>,
    status: String,
    tool: ToolIdentity,
    artifact_digest: Option<String>,
    status_failure: Option<String>,
}

impl FakeFacts {
    fn new() -> Self {
        Self {
            blobs: HashMap::from([("src/lib.rs".to_string(), BLOB.to_string())]),
            status: String::new(),
            tool: tool(),
            artifact_digest: Some(ARTIFACT_DIGEST.to_string()),
            status_failure: None,
        }
    }
}

#[async_trait]
impl RepositoryFacts for FakeFacts {
    async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
        Ok(COMMIT.to_string())
    }
    async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
        Ok(TREE.to_string())
    }
    async fn tracked_blob_oid(&self, _workspace: &Path, path: &str) -> Result<String, String> {
        self.blobs
            .get(path)
            .cloned()
            .ok_or_else(|| format!("no blob for {path}"))
    }
    async fn hash_file(&self, _workspace: &Path, _path: &Path) -> Result<String, String> {
        self.artifact_digest
            .clone()
            .ok_or_else(|| "artifact unreadable".to_string())
    }
    async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
        match &self.status_failure {
            Some(error) => Err(error.clone()),
            None => Ok(self.status.clone()),
        }
    }
    async fn resolve_tool(
        &self,
        _workspace: &Path,
        _program: &str,
    ) -> Result<ToolIdentity, String> {
        Ok(self.tool.clone())
    }
}

/// Supervisor that records every argv it was asked to run.
///
/// The recording is the point: proving that *only* manifest gates executed is a
/// statement about what was never started, which only a record of every call can
/// make.
#[derive(Clone)]
struct RecordingSupervisor {
    exit_code: i32,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    /// When set, the supervised command never returns on its own.
    hang: bool,
}

impl RecordingSupervisor {
    fn passing() -> Self {
        Self {
            exit_code: 0,
            calls: Arc::new(Mutex::new(Vec::new())),
            hang: false,
        }
    }

    fn failing() -> Self {
        Self {
            exit_code: 101,
            ..Self::passing()
        }
    }

    fn hanging() -> Self {
        Self {
            hang: true,
            ..Self::passing()
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl CommandSupervisor for RecordingSupervisor {
    async fn run(&self, argv: &[String], _cwd: &Path) -> Result<SupervisedOutcome, String> {
        self.calls.lock().unwrap().push(argv.to_vec());
        if self.hang {
            // Never resolves; the caller's bounded timeout is what must end it.
            std::future::pending::<()>().await;
        }
        Ok(SupervisedOutcome {
            exit_code: self.exit_code,
            output: b"gate output\n".to_vec(),
        })
    }
}

/// Deterministic clock advancing a fixed amount per reading, so measured
/// durations are assertions rather than wall-clock accidents.
struct StepClock {
    next: Mutex<DateTime<Utc>>,
}

impl StepClock {
    fn new() -> Self {
        Self {
            next: Mutex::new(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
        }
    }
}

impl Clock for StepClock {
    fn now(&self) -> DateTime<Utc> {
        let mut next = self.next.lock().unwrap();
        let current = *next;
        *next = current + chrono::Duration::seconds(120);
        current
    }
}

fn executor(
    workspace: &Path,
    facts: FakeFacts,
    supervisor: RecordingSupervisor,
) -> RuntimeVerificationExecutor<FakeFacts, RecordingSupervisor, StepClock> {
    RuntimeVerificationExecutor::new(workspace.to_path_buf(), facts, supervisor, StepClock::new())
}

// ============================================================================
// Only runtime-owned declared gates execute
// ============================================================================

/// The whole boundary in one assertion: exactly the manifest's gates ran, in
/// canonical order, and nothing else was started.
#[tokio::test]
async fn acceptance_execution_boundary_runs_only_manifest_declared_gates() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate"), gate("beta-gate")]);

    let results = execute_declared_gates(&executor, &manifest, &[], GateBudget::unbounded()).await;

    assert_eq!(
        supervisor
            .calls()
            .iter()
            .map(|argv| argv.join(" "))
            .collect::<Vec<_>>(),
        vec!["cargo test alpha-gate", "cargo test beta-gate"],
        "only the manifest's own gates may be started, in canonical order"
    );
    assert!(results.outcomes.iter().all(GateOutcome::is_successful));
    assert_eq!(classify_gate_results(&results), GateVerdict::Proceed);
}

/// A change with no eligible declaration starts no command at all. An empty
/// allowlist is a boundary, not an invitation.
#[tokio::test]
async fn acceptance_execution_boundary_empty_manifest_starts_nothing() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());

    let results = execute_declared_gates(
        &executor,
        &manifest(Vec::new()),
        &[],
        GateBudget::unbounded(),
    )
    .await;

    assert!(results.is_empty());
    assert!(supervisor.calls().is_empty());
    assert_eq!(classify_gate_results(&results), GateVerdict::Proceed);
}

// ============================================================================
// Exact evidence reuse
// ============================================================================

fn stored_evidence(id: &str) -> VerificationEvidence {
    VerificationEvidence {
        schema: EVIDENCE_SCHEMA.to_string(),
        authority: EVIDENCE_AUTHORITY.to_string(),
        verification_id: id.to_string(),
        commit_oid: COMMIT.to_string(),
        tree_oid: TREE.to_string(),
        argv: vec!["cargo".to_string(), "test".to_string(), id.to_string()],
        cwd: ".".to_string(),
        automation_path: "src/lib.rs".to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        started_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ended_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 2, 0).unwrap(),
        exit_code: 0,
        artifact_path: format!(".cflx/verification-evidence/{id}.log"),
        artifact_digest: ARTIFACT_DIGEST.to_string(),
        clean_before: true,
        clean_after: true,
    }
}

#[tokio::test]
async fn acceptance_execution_boundary_reuses_exact_bound_evidence_without_running_it() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate")]);
    let decisions = vec![ReuseDecision::Reuse {
        verification_id: "alpha-gate".to_string(),
        evidence: Box::new(stored_evidence("alpha-gate")),
    }];

    let results =
        execute_declared_gates(&executor, &manifest, &decisions, GateBudget::unbounded()).await;

    assert!(
        supervisor.calls().is_empty(),
        "exact bound evidence must be reused without executing the command again"
    );
    assert!(matches!(
        results.outcomes.as_slice(),
        [GateOutcome::Reused { .. }]
    ));
    assert_eq!(classify_gate_results(&results), GateVerdict::Proceed);
}

/// A `Rerun` decision is a decision to *run*, never an assumption. The stale
/// record contributes nothing to the outcome.
#[tokio::test]
async fn acceptance_execution_boundary_stale_evidence_executes_the_command() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate")]);
    let decisions = vec![ReuseDecision::Rerun {
        verification_id: "alpha-gate".to_string(),
        reason: RerunReason::Mismatch {
            field: "commit_oid",
            recorded: "old".to_string(),
            current: COMMIT.to_string(),
        },
    }];

    let results =
        execute_declared_gates(&executor, &manifest, &decisions, GateBudget::unbounded()).await;

    assert_eq!(supervisor.calls().len(), 1);
    assert!(matches!(
        results.outcomes.as_slice(),
        [GateOutcome::Executed { .. }]
    ));
}

/// The reuse planner refuses every gate when the tree is dirty outside the
/// evidence directory, because a capture bound to a different worktree is not
/// evidence about this one.
#[tokio::test]
async fn acceptance_execution_boundary_dirty_worktree_forces_rerun_for_every_gate() {
    let workspace = tempfile::tempdir().unwrap();
    let store = crate::orchestration::acceptance::verification_evidence::EvidenceStore::new(
        workspace.path().to_path_buf(),
    );
    store.store(&stored_evidence("alpha-gate")).unwrap();
    std::fs::write(
        workspace
            .path()
            .join(".cflx/verification-evidence/alpha-gate.log"),
        b"gate output\n",
    )
    .ok();
    let mut facts = FakeFacts::new();
    facts.status = " M src/lib.rs\n".to_string();

    let decisions = plan_manifest_reuse(
        &facts,
        workspace.path(),
        &manifest(vec![gate("alpha-gate")]),
        ReusePolicy::default(),
    )
    .await;

    assert!(matches!(
        decisions.as_slice(),
        [ReuseDecision::Rerun {
            reason: RerunReason::DirtyWorktree(_),
            ..
        }]
    ));
}

/// Unobservable Git state is a rerun reason for every gate at once, from a
/// single status read: two reads could straddle a write and let one gate
/// believe the tree was clean while another saw it dirty.
#[tokio::test]
async fn acceptance_execution_boundary_unobservable_state_reruns_every_gate() {
    let workspace = tempfile::tempdir().unwrap();
    let mut facts = FakeFacts::new();
    facts.status_failure = Some("git status failed".to_string());

    let decisions = plan_manifest_reuse(
        &facts,
        workspace.path(),
        &manifest(vec![gate("alpha-gate"), gate("beta-gate")]),
        ReusePolicy::default(),
    )
    .await;

    assert_eq!(decisions.len(), 2);
    assert!(decisions.iter().all(|decision| matches!(
        decision,
        ReuseDecision::Rerun {
            reason: RerunReason::Unobservable(_),
            ..
        }
    )));
}

// ============================================================================
// Failure, deadline, and prerequisite routing
// ============================================================================

/// A failing declared gate is a blocking failure of the candidate, and the
/// remaining gates are not started: the verdict is already determined and the
/// shared budget belongs to the review that has to explain it.
#[tokio::test]
async fn acceptance_execution_boundary_failing_gate_stops_the_gate_phase() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::failing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate"), gate("beta-gate")]);

    let results = execute_declared_gates(&executor, &manifest, &[], GateBudget::unbounded()).await;

    assert_eq!(
        supervisor.calls().len(),
        1,
        "the second gate must not run after the first already decided the verdict"
    );
    match classify_gate_results(&results) {
        GateVerdict::Failed { evidence } => {
            assert!(evidence[0].contains("FAILED"), "{evidence:?}");
            assert!(evidence[0].contains("alpha-gate"), "{evidence:?}");
        }
        other => panic!("a failing declared gate must fail the change, got {other:?}"),
    }
}

/// A gate that cannot finish inside the shared budget becomes the typed
/// `acceptance_declared_gate_deadline_exhausted` hold — never more waiting, and
/// never a finding against the change.
#[tokio::test(start_paused = true)]
async fn acceptance_execution_boundary_gate_deadline_becomes_a_typed_hold() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::hanging();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate")]);

    let results = execute_declared_gates(
        &executor,
        &manifest,
        &[],
        GateBudget::bounded(std::time::Duration::from_secs(30)),
    )
    .await;

    assert!(matches!(
        results.outcomes.as_slice(),
        [GateOutcome::DeadlineExhausted { .. }]
    ));
    match classify_gate_results(&results) {
        GateVerdict::Hold { category, .. } => assert_eq!(
            category,
            AcceptanceHoldCategory::DeclaredGateDeadlineExhausted
        ),
        other => panic!("gate deadline exhaustion must be a typed hold, got {other:?}"),
    }
}

/// Once the budget is gone, later gates report exhaustion rather than starting.
#[tokio::test(start_paused = true)]
async fn acceptance_execution_boundary_exhausted_budget_starts_no_further_gate() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::hanging();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let manifest = manifest(vec![gate("alpha-gate"), gate("beta-gate")]);

    let results = execute_declared_gates(
        &executor,
        &manifest,
        &[],
        GateBudget::bounded(std::time::Duration::from_secs(30)),
    )
    .await;

    assert_eq!(
        supervisor.calls().len(),
        1,
        "the second gate must not be started once the shared budget is spent"
    );
    assert_eq!(results.outcomes.len(), 2);
    assert!(results
        .outcomes
        .iter()
        .all(|outcome| matches!(outcome, GateOutcome::DeadlineExhausted { .. })));
}

/// An unresolvable declared tool is a named external prerequisite, and it
/// outranks runtime-defect classification: otherwise "install the tool" would
/// harden into a non-resumable hold nobody can clear by installing it.
#[tokio::test]
async fn acceptance_execution_boundary_missing_tool_keeps_the_external_contract() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(workspace.path(), FakeFacts::new(), supervisor.clone());
    let mut unresolvable = gate("alpha-gate");
    unresolvable.tool = ToolIdentity::default();
    let manifest = manifest(vec![unresolvable]);

    let results = execute_declared_gates(&executor, &manifest, &[], GateBudget::unbounded()).await;

    assert!(supervisor.calls().is_empty());
    match classify_gate_results(&results) {
        GateVerdict::ExternalPrerequisite { blocker } => {
            assert_eq!(blocker.category, "infrastructure");
            assert!(
                blocker.resumable,
                "an installable prerequisite must stay resumable"
            );
            assert!(blocker.unblock_condition.contains("alpha-gate"));
            assert!(!blocker.evidence.is_empty());
        }
        other => panic!("a missing declared tool must keep the external contract, got {other:?}"),
    }
}

/// External prerequisite outranks a gate-deadline hold in the same result set.
#[test]
fn acceptance_execution_boundary_external_prerequisite_outranks_runtime_holds() {
    let results = GateResults {
        outcomes: vec![
            GateOutcome::DeadlineExhausted {
                verification_id: "alpha-gate".to_string(),
            },
            GateOutcome::Refused {
                verification_id: "beta-gate".to_string(),
                code: TOOL_UNAVAILABLE.to_string(),
                detail: "no cargo on PATH".to_string(),
            },
        ],
    };

    assert!(matches!(
        classify_gate_results(&results),
        GateVerdict::ExternalPrerequisite { .. }
    ));
}

/// A refusal that is not a prerequisite is a runtime defect: an environment
/// fact, never a finding against the change.
#[test]
fn acceptance_execution_boundary_other_refusals_are_runtime_defects() {
    let results = GateResults {
        outcomes: vec![GateOutcome::Refused {
            verification_id: "alpha-gate".to_string(),
            code: "state_unobservable".to_string(),
            detail: "git rev-parse failed".to_string(),
        }],
    };

    match classify_gate_results(&results) {
        GateVerdict::Hold { category, .. } => {
            assert_eq!(category, AcceptanceHoldCategory::RuntimeDefect)
        }
        other => panic!("an unclassified refusal must be a runtime defect, got {other:?}"),
    }
}

/// A dirty worktree refuses capture rather than recording a binding the command
/// never actually had.
#[tokio::test]
async fn acceptance_execution_boundary_dirty_capture_is_refused_not_recorded() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let mut facts = FakeFacts::new();
    facts.status = " M src/lib.rs\n".to_string();
    let executor = executor(workspace.path(), facts, supervisor.clone());

    let results = execute_declared_gates(
        &executor,
        &manifest(vec![gate("alpha-gate")]),
        &[],
        GateBudget::unbounded(),
    )
    .await;

    assert!(supervisor.calls().is_empty());
    assert!(matches!(
        results.outcomes.as_slice(),
        [GateOutcome::Refused { code, .. }] if code == "worktree_dirty"
    ));
}

// ============================================================================
// Result projection
// ============================================================================

#[test]
fn acceptance_execution_boundary_gate_results_project_typed_statuses() {
    let results = GateResults {
        outcomes: vec![
            GateOutcome::Executed {
                verification_id: "alpha-gate".to_string(),
                artifact_path: "a.log".to_string(),
                elapsed_seconds: 120,
            },
            GateOutcome::Reused {
                verification_id: "beta-gate".to_string(),
                artifact_path: "b.log".to_string(),
                elapsed_seconds: 90,
            },
        ],
    };

    let json = results.to_json();
    assert_eq!(json["all_successful"], serde_json::json!(true));
    assert_eq!(json["gates"][0]["outcome"], serde_json::json!("executed"));
    assert_eq!(json["gates"][1]["outcome"], serde_json::json!("reused"));
    assert_eq!(results.summary_lines().len(), 2);
}
