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
    EvidenceStore, RerunReason, ReuseDecision, SupervisedOutcome, ToolIdentity,
    VerificationEvidence, EVIDENCE_AUTHORITY, EVIDENCE_SCHEMA,
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
        artifact_path: EvidenceStore::artifact_relative_path(id),
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
        declaration_defects: Vec::new(),
        created_at: Utc::now(),
        absolute_deadline_secs: 3600,
        work_budget_secs: 3570,
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

/// The target tree the gates run in. A sibling of the store below, never its
/// ancestor, so a target-local write would be visible as an unexpected entry.
fn target_of(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let target = temp.path().join("target");
    std::fs::create_dir_all(&target).expect("create target");
    target
}

/// The Conflux-owned external store, shaped like the real layout.
fn store_of(temp: &tempfile::TempDir) -> EvidenceStore {
    EvidenceStore::new(
        temp.path()
            .join("cflx-state/acceptance/project-slug/workspace-slug/alpha"),
    )
}

fn executor(
    temp: &tempfile::TempDir,
    facts: FakeFacts,
    supervisor: RecordingSupervisor,
) -> RuntimeVerificationExecutor<FakeFacts, RecordingSupervisor, StepClock> {
    RuntimeVerificationExecutor::new(
        target_of(temp),
        store_of(temp),
        facts,
        supervisor,
        StepClock::new(),
    )
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());

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
        review_base_commit: BLOB.to_string(),
        review_range: format!("{BLOB}..{COMMIT}"),
        argv: vec!["cargo".to_string(), "test".to_string(), id.to_string()],
        cwd: ".".to_string(),
        automation_path: "src/lib.rs".to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        started_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ended_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 2, 0).unwrap(),
        exit_code: 0,
        artifact_path: EvidenceStore::artifact_relative_path(id),
        artifact_digest: ARTIFACT_DIGEST.to_string(),
        clean_before: true,
        clean_after: true,
    }
}

#[tokio::test]
async fn acceptance_execution_boundary_reuses_exact_bound_evidence_without_running_it() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let store = store_of(&workspace);
    store.store(&stored_evidence("alpha-gate")).unwrap();
    std::fs::write(store.artifact_path("alpha-gate"), b"gate output\n").ok();
    let mut facts = FakeFacts::new();
    facts.status = " M src/lib.rs\n".to_string();

    let decisions = plan_manifest_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
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
        &target_of(&workspace),
        &store_of(&workspace),
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
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

/// A gate that hits the shared deadline leaves nothing running.
///
/// The double above spawns no process, so it can only prove the *routing*. This
/// one uses `DirectCommandSupervisor` and a real long-running child, because the
/// defect it guards is invisible to an in-memory supervisor: a `timeout` wrapped
/// around the capture future would drop that future and leave `sleep` alive in
/// the worktree, while the hold written afterwards claimed a confirmed cleanup.
///
/// Two assertions carry the contract. The outcome's own cleanup evidence must be
/// confirmed — which the sweep only reports after a membership probe finds the
/// owned group empty — and the process group must still be gone when the outcome
/// is read, which is a statement about ordering: termination and reaping happen
/// *before* `DeadlineExhausted` is returned, not after.
#[tokio::test]
async fn acceptance_execution_boundary_gate_deadline_terminates_and_reaps_a_real_child() {
    use crate::orchestration::acceptance::verification_evidence::{
        DirectCommandSupervisor, SupervisedRun,
    };

    let workspace = tempfile::tempdir().unwrap();
    let supervisor = DirectCommandSupervisor;

    let run = supervisor
        .run_bounded(
            &["sh".to_string(), "-c".to_string(), "sleep 30".to_string()],
            workspace.path(),
            Some(std::time::Duration::from_millis(150)),
        )
        .await
        .expect("spawning a real child must succeed");

    let report = match run {
        SupervisedRun::DeadlineExhausted(report) => report,
        SupervisedRun::Completed(outcome) => {
            panic!("a 30-second sleep cannot finish inside 150ms, got {outcome:?}")
        }
    };
    assert!(
        report.is_confirmed(),
        "the gate's owned process group must be proven empty before the outcome is \
         returned: {}",
        report.diagnostics()
    );

    // The proof is re-taken here, from the caller's side of the return. If
    // termination happened lazily — on drop, or on a detached task — the group
    // could still have members at this point.
    #[cfg(unix)]
    if let Some(pgid) = report.pgid() {
        use nix::errno::Errno;
        use nix::sys::signal::killpg;
        use nix::unistd::Pid;

        assert_eq!(
            killpg(Pid::from_raw(pgid as i32), None),
            Err(Errno::ESRCH),
            "no member of the gate's process group may survive the DeadlineExhausted outcome"
        );
    }
}

/// The same deadline through the gate phase: the typed hold carries the sweep's
/// own verdict rather than a constant, so "we stopped it" and "it is gone" stay
/// separate facts.
#[tokio::test]
async fn acceptance_execution_boundary_gate_deadline_hold_reports_real_cleanup_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let mut sleeping = gate("alpha-gate");
    sleeping.argv = vec!["sh".to_string(), "-c".to_string(), "sleep 30".to_string()];
    let executor =
        crate::orchestration::acceptance::verification_evidence::RuntimeVerificationExecutor::new(
            target_of(&workspace),
            store_of(&workspace),
            FakeFacts::new(),
            crate::orchestration::acceptance::verification_evidence::DirectCommandSupervisor,
            StepClock::new(),
        );

    let results = execute_declared_gates(
        &executor,
        &manifest(vec![sleeping]),
        &[],
        GateBudget::bounded(std::time::Duration::from_millis(150)),
    )
    .await;

    match results.outcomes.as_slice() {
        [GateOutcome::DeadlineExhausted {
            cleanup_confirmed,
            cleanup_diagnostics,
            ..
        }] => assert!(
            *cleanup_confirmed,
            "the terminated gate's cleanup must be proven, not assumed: {cleanup_diagnostics}"
        ),
        other => panic!("a gate that outlives its budget must report a deadline, got {other:?}"),
    }
    let (confirmed, diagnostics) = results.cleanup_evidence();
    assert!(confirmed, "{diagnostics}");
    assert_eq!(
        classify_gate_results(&results),
        GateVerdict::Hold {
            category: AcceptanceHoldCategory::DeclaredGateDeadlineExhausted,
            evidence: results.summary_lines(),
        }
    );
}

/// Evidence bound to a *different* review base is not evidence about this
/// review, so the gate is executed rather than reused — even though every other
/// binding is exact.
#[tokio::test]
async fn acceptance_execution_boundary_review_base_mismatch_forces_rerun() {
    let workspace = tempfile::tempdir().unwrap();
    let store = store_of(&workspace);
    // Exact in every binding except the review it was produced for.
    let mut foreign = stored_evidence("alpha-gate");
    foreign.review_base_commit = "9".repeat(40);
    foreign.review_range = format!("{}..{COMMIT}", "9".repeat(40));
    store.store(&foreign).unwrap();
    std::fs::write(store.artifact_path("alpha-gate"), b"gate output\n").ok();

    let decisions = plan_manifest_reuse(
        &FakeFacts::new(),
        &target_of(&workspace),
        &store_of(&workspace),
        &manifest(vec![gate("alpha-gate")]),
        ReusePolicy::default(),
    )
    .await;

    match decisions.as_slice() {
        [ReuseDecision::Rerun {
            reason: RerunReason::Mismatch { field, .. },
            ..
        }] => assert_eq!(
            *field, "review_base_commit",
            "the rerun reason must name the binding that actually moved"
        ),
        other => panic!("a foreign review base must force a rerun, got {other:?}"),
    }

    // And the exact same envelope, bound to *this* review, is reused.
    store.store(&stored_evidence("alpha-gate")).unwrap();
    let decisions = plan_manifest_reuse(
        &FakeFacts::new(),
        &target_of(&workspace),
        &store_of(&workspace),
        &manifest(vec![gate("alpha-gate")]),
        ReusePolicy::default(),
    )
    .await;
    assert!(
        matches!(decisions.as_slice(), [ReuseDecision::Reuse { .. }]),
        "evidence bound to this review is still reused: {decisions:?}"
    );
}

/// A moved review *range* invalidates evidence on its own: the same base with a
/// different candidate is a different comparison.
#[tokio::test]
async fn acceptance_execution_boundary_review_range_mismatch_forces_rerun() {
    let workspace = tempfile::tempdir().unwrap();
    let store = store_of(&workspace);
    let mut foreign = stored_evidence("alpha-gate");
    foreign.review_range = format!("{BLOB}..{}", "9".repeat(40));
    store.store(&foreign).unwrap();
    std::fs::write(store.artifact_path("alpha-gate"), b"gate output\n").ok();

    let decisions = plan_manifest_reuse(
        &FakeFacts::new(),
        &target_of(&workspace),
        &store_of(&workspace),
        &manifest(vec![gate("alpha-gate")]),
        ReusePolicy::default(),
    )
    .await;

    match decisions.as_slice() {
        [ReuseDecision::Rerun {
            reason: RerunReason::Mismatch { field, .. },
            ..
        }] => assert_eq!(*field, "review_range"),
        other => panic!("a foreign review range must force a rerun, got {other:?}"),
    }
}

/// External prerequisite outranks a gate-deadline hold in the same result set.
#[test]
fn acceptance_execution_boundary_external_prerequisite_outranks_runtime_holds() {
    let results = GateResults {
        outcomes: vec![
            GateOutcome::DeadlineExhausted {
                verification_id: "alpha-gate".to_string(),
                cleanup_confirmed: true,
                cleanup_diagnostics: String::new(),
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
    let executor = executor(&workspace, facts, supervisor.clone());

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

// ============================================================================
// Invalid required manifest: nothing runs, and the verdict outranks everything
// ============================================================================

fn shell_syntax_defect(
) -> crate::orchestration::acceptance::verification_evidence::DeclarationDefect {
    crate::orchestration::acceptance::verification_evidence::DeclarationDefect {
        verification_id: "alpha-gate".to_string(),
        position: 1,
        field: "rerun".to_string(),
        code: "declaration_shell_syntax".to_string(),
        detail: "command is not a plain argv the runtime can supervise: unbalanced quote"
            .to_string(),
    }
}

/// Even a manifest that (wrongly) carries a gate beside a defect starts no
/// command and reads no evidence: the refusal does not depend on the builder's
/// empty-gate invariant.
#[tokio::test]
async fn required_verification_fail_closed_invalid_manifest_runs_and_reuses_nothing() {
    let workspace = tempfile::tempdir().unwrap();
    let supervisor = RecordingSupervisor::passing();
    let executor = executor(&workspace, FakeFacts::new(), supervisor.clone());
    let mut manifest = manifest(vec![gate("alpha-gate")]);
    manifest.declaration_defects = vec![shell_syntax_defect()];

    let decisions = plan_manifest_reuse(
        &FakeFacts::new(),
        &target_of(&workspace),
        &store_of(&workspace),
        &manifest,
        ReusePolicy::default(),
    )
    .await;
    let results =
        execute_declared_gates(&executor, &manifest, &decisions, GateBudget::unbounded()).await;

    assert!(decisions.is_empty(), "no evidence may be read for reuse");
    assert!(
        supervisor.calls().is_empty(),
        "no declared command may start"
    );
    assert!(results.is_empty());
    assert_eq!(
        classify_boundary(&manifest, &results),
        GateVerdict::DeclarationInvalid {
            defects: vec![shell_syntax_defect()],
        }
    );
}

/// Declaration defects are classified before `tool_unavailable`: a defective
/// declaration was never resolved against PATH, so it cannot be a prerequisite.
#[test]
fn required_verification_fail_closed_defects_outrank_tool_unavailable() {
    let mut manifest = manifest(Vec::new());
    manifest.declaration_defects = vec![shell_syntax_defect()];
    let results = GateResults {
        outcomes: vec![GateOutcome::Refused {
            verification_id: "beta-gate".to_string(),
            code: TOOL_UNAVAILABLE.to_string(),
            detail: "missing".to_string(),
        }],
    };

    assert!(matches!(
        classify_boundary(&manifest, &results),
        GateVerdict::DeclarationInvalid { .. }
    ));
    // A valid manifest keeps today's classification untouched.
    assert!(matches!(
        classify_boundary(&self::manifest(Vec::new()), &results),
        GateVerdict::ExternalPrerequisite { .. }
    ));
    assert_eq!(
        classify_boundary(&self::manifest(Vec::new()), &GateResults::default()),
        GateVerdict::Proceed
    );
}
