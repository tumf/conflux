//! Adversarial coverage for bound verification evidence reuse.
//!
//! Every test here is a way an unbound, stale, forged, or self-referential
//! record could otherwise turn "no evidence" into PASS. The Git and process
//! boundaries are in-memory doubles, so these stay unit-scoped: the only real
//! filesystem use is a temporary directory holding the sidecar the module is
//! defined to read from disk.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::*;
use crate::openspec::VerificationDeclaration;

const COMMIT: &str = "1111111111111111111111111111111111111111";
const TREE: &str = "2222222222222222222222222222222222222222";
const BLOB: &str = "3333333333333333333333333333333333333333";
const TOOL_DIGEST: &str = "4444444444444444444444444444444444444444";
const ARTIFACT_DIGEST: &str = "5555555555555555555555555555555555555555";

fn tool() -> ToolIdentity {
    ToolIdentity {
        path: "/usr/bin/cargo".to_string(),
        executable_digest: TOOL_DIGEST.to_string(),
        version: Some("cargo 1.80.0".to_string()),
    }
}

fn declaration() -> VerificationDeclaration {
    VerificationDeclaration {
        id: Some("bound-evidence-tests".to_string()),
        requirement: Some("acceptance reuses bound evidence".to_string()),
        phase: Some("pre-integration".to_string()),
        owner: Some("conflux-acceptance".to_string()),
        trigger: Some("pull-request-validation".to_string()),
        automation: Some("src/orchestration/acceptance.rs".to_string()),
        evidence: Some("cargo test orchestration::acceptance --lib".to_string()),
        rerun: Some("cargo test orchestration::acceptance --lib".to_string()),
        prerequisites: Some(Vec::new()),
        execution_class: Some("repository-local".to_string()),
        completion_role: Some("change-blocking".to_string()),
    }
}

fn request() -> VerificationRequest {
    eligible_request(&declaration()).expect("declaration is eligible")
}

fn evidence() -> VerificationEvidence {
    let started_at = "2026-08-23T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    VerificationEvidence {
        schema: EVIDENCE_SCHEMA.to_string(),
        authority: EVIDENCE_AUTHORITY.to_string(),
        verification_id: "bound-evidence-tests".to_string(),
        commit_oid: COMMIT.to_string(),
        tree_oid: TREE.to_string(),
        argv: vec![
            "cargo".to_string(),
            "test".to_string(),
            "orchestration::acceptance".to_string(),
            "--lib".to_string(),
        ],
        cwd: ".".to_string(),
        automation_path: "src/orchestration/acceptance.rs".to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        started_at,
        ended_at: started_at + chrono::Duration::seconds(300),
        exit_code: 0,
        artifact_path: format!("{EVIDENCE_DIR}/bound-evidence-tests.log"),
        artifact_digest: ARTIFACT_DIGEST.to_string(),
        clean_before: true,
        clean_after: true,
    }
}

fn current() -> CurrentBindings {
    CurrentBindings {
        commit_oid: COMMIT.to_string(),
        tree_oid: TREE.to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        clean: true,
        artifact_digest: Some(ARTIFACT_DIGEST.to_string()),
    }
}

fn decide(current: CurrentBindings) -> ReuseDecision {
    evaluate_reuse(&request(), &evidence(), &current, ReusePolicy::default())
}

fn rerun_code(decision: &ReuseDecision) -> String {
    match decision {
        ReuseDecision::Reuse { .. } => panic!("expected rerun, got reuse"),
        ReuseDecision::Rerun { reason, .. } => reason.code().to_string(),
    }
}

fn mismatch_field(decision: &ReuseDecision) -> String {
    match decision {
        ReuseDecision::Rerun {
            reason: RerunReason::Mismatch { field, .. },
            ..
        } => (*field).to_string(),
        other => panic!("expected a binding mismatch, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Schema and fail-closed parsing
// ---------------------------------------------------------------------------

#[test]
fn exact_record_round_trips_through_the_versioned_schema() {
    let bytes = serde_json::to_vec(&evidence()).expect("serialize");
    let parsed = parse_evidence(&bytes).expect("record parses");
    assert_eq!(parsed, evidence());
    assert_eq!(parsed.elapsed_seconds(), Some(300));
}

#[test]
fn unknown_schema_is_refused_rather_than_migrated() {
    let mut value = serde_json::to_value(evidence()).unwrap();
    value["schema"] = serde_json::json!("conflux-verification-evidence-v0");
    let defect = parse_evidence(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "evidence_unknown_schema");
}

#[test]
fn agent_authored_authority_marking_is_refused() {
    let mut record = evidence();
    record.authority = "apply-agent".to_string();
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "evidence_agent_authored");
}

#[test]
fn short_commit_id_is_refused() {
    let mut record = evidence();
    record.commit_oid = "1111111".to_string();
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "evidence_malformed");
    assert!(defect.detail().contains("full-length object id"));
}

#[test]
fn missing_required_field_is_refused() {
    let mut value = serde_json::to_value(evidence()).unwrap();
    value.as_object_mut().unwrap().remove("artifact_digest");
    let defect = parse_evidence(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "evidence_malformed");
}

#[test]
fn non_json_bytes_are_refused() {
    let defect = parse_evidence(b"not json at all").unwrap_err();
    assert_eq!(defect.code(), "evidence_malformed");
}

#[test]
fn unsuccessful_exit_is_never_a_candidate() {
    let mut record = evidence();
    record.exit_code = 101;
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert!(defect.detail().contains("not success"));
}

#[test]
fn dirty_capture_is_never_a_candidate() {
    for (before, after) in [(false, true), (true, false), (false, false)] {
        let mut record = evidence();
        record.clean_before = before;
        record.clean_after = after;
        let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
        assert!(defect.detail().contains("dirty"), "{before}/{after}");
    }
}

#[test]
fn reversed_timestamps_are_refused() {
    let mut record = evidence();
    record.ended_at = record.started_at - chrono::Duration::seconds(1);
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert!(defect.detail().contains("precedes"));
}

#[test]
fn artifact_outside_the_evidence_directory_is_refused() {
    let mut record = evidence();
    record.artifact_path = "target/output.log".to_string();
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert!(defect.detail().contains("outside"));
}

#[test]
fn escaping_cwd_is_refused() {
    assert_eq!(normalize_relative_cwd("../elsewhere"), None);
    assert_eq!(normalize_relative_cwd("/absolute"), None);
    assert_eq!(normalize_relative_cwd("./src/"), Some("src".to_string()));
    assert_eq!(normalize_relative_cwd(""), Some(".".to_string()));

    let mut record = evidence();
    record.cwd = "../elsewhere".to_string();
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert!(defect.detail().contains("repository-relative"));
}

#[test]
fn traversing_verification_id_cannot_name_a_sidecar() {
    assert!(!is_storable_verification_id("../escape"));
    assert!(!is_storable_verification_id("nested/id"));
    assert!(!is_storable_verification_id(""));
    assert!(is_storable_verification_id("bound-evidence-tests"));
}

// ---------------------------------------------------------------------------
// Declaration eligibility
// ---------------------------------------------------------------------------

#[test]
fn eligible_declaration_becomes_a_plain_argv_request() {
    let request = request();
    assert_eq!(request.verification_id, "bound-evidence-tests");
    assert_eq!(request.argv[0], "cargo");
    assert_eq!(request.cwd, ".");
    assert_eq!(request.automation_path, "src/orchestration/acceptance.rs");
}

#[test]
fn non_repository_local_declaration_is_ineligible() {
    let mut declaration = declaration();
    declaration.execution_class = Some("deployed-service".to_string());
    assert_eq!(
        eligible_request(&declaration).unwrap_err().code(),
        "declaration_not_repository_local"
    );
}

#[test]
fn observation_only_declaration_is_ineligible() {
    let mut declaration = declaration();
    declaration.completion_role = Some("operational-observation".to_string());
    assert_eq!(
        eligible_request(&declaration).unwrap_err().code(),
        "declaration_not_change_blocking"
    );
}

#[test]
fn shell_syntax_declaration_is_ineligible() {
    for command in [
        "cargo test | tee out.log",
        "cargo test && cargo clippy",
        "cargo test > out.log",
        "FOO=$BAR cargo test",
    ] {
        let mut declaration = declaration();
        declaration.rerun = Some(command.to_string());
        assert_eq!(
            eligible_request(&declaration).unwrap_err().code(),
            "declaration_shell_syntax",
            "{command}"
        );
    }
}

// ---------------------------------------------------------------------------
// Fail-closed validator: one test per mismatch class
// ---------------------------------------------------------------------------

#[test]
fn exact_match_is_reused() {
    let decision = decide(current());
    assert!(decision.is_reuse());
    assert_eq!(decision.outcome(), "reused");
    assert_eq!(decision.verification_id(), "bound-evidence-tests");
    assert_eq!(decision.to_json()["outcome"], "reused");
}

#[test]
fn stale_commit_reruns() {
    let mut current = current();
    current.commit_oid = "9999999999999999999999999999999999999999".to_string();
    assert_eq!(mismatch_field(&decide(current)), "commit_oid");
}

#[test]
fn stale_tree_reruns() {
    let mut current = current();
    current.tree_oid = "8888888888888888888888888888888888888888".to_string();
    assert_eq!(mismatch_field(&decide(current)), "tree_oid");
}

#[test]
fn changed_automation_blob_reruns() {
    let mut current = current();
    current.automation_blob_oid = "7777777777777777777777777777777777777777".to_string();
    assert_eq!(mismatch_field(&decide(current)), "automation_blob_oid");
}

#[test]
fn different_executable_reruns() {
    let mut current = current();
    current.tool.path = "/opt/homebrew/bin/cargo".to_string();
    assert_eq!(mismatch_field(&decide(current)), "tool_path");
}

#[test]
fn upgraded_executable_reruns() {
    let mut current = current();
    current.tool.executable_digest = "6666666666666666666666666666666666666666".to_string();
    assert_eq!(mismatch_field(&decide(current)), "tool_digest");
}

#[test]
fn different_tool_version_reruns() {
    let mut current = current();
    current.tool.version = Some("cargo 1.81.0".to_string());
    assert_eq!(mismatch_field(&decide(current)), "tool_version");
}

#[test]
fn changed_command_reruns() {
    let mut declaration = declaration();
    declaration.rerun = Some("cargo test orchestration --lib".to_string());
    let request = eligible_request(&declaration).unwrap();
    let decision = evaluate_reuse(&request, &evidence(), &current(), ReusePolicy::default());
    assert_eq!(mismatch_field(&decision), "argv");
}

#[test]
fn evidence_for_another_verification_id_cannot_satisfy_this_one() {
    let mut record = evidence();
    record.verification_id = "some-other-verification".to_string();
    let decision = evaluate_reuse(&request(), &record, &current(), ReusePolicy::default());
    assert_eq!(mismatch_field(&decision), "verification_id");
}

#[test]
fn rewritten_artifact_reruns() {
    let mut current = current();
    current.artifact_digest = Some("0000000000000000000000000000000000000000".to_string());
    assert_eq!(mismatch_field(&decide(current)), "artifact_digest");
}

#[test]
fn missing_artifact_reruns() {
    let mut current = current();
    current.artifact_digest = None;
    assert_eq!(rerun_code(&decide(current)), "evidence_unreadable");
}

#[test]
fn dirty_worktree_reruns() {
    let mut current = current();
    current.clean = false;
    assert_eq!(rerun_code(&decide(current)), "worktree_dirty");
}

#[test]
fn cheap_command_stays_on_the_rerun_path() {
    let mut record = evidence();
    record.ended_at = record.started_at + chrono::Duration::seconds(3);
    let decision = evaluate_reuse(&request(), &record, &current(), ReusePolicy::default());
    assert_eq!(rerun_code(&decision), "below_reuse_duration_threshold");
    assert!(decision.summary().contains("60s reuse threshold"));
}

#[test]
fn duration_policy_threshold_is_repository_tracked() {
    let policy = ReusePolicy {
        min_reuse_seconds: 1,
    };
    let mut record = evidence();
    record.ended_at = record.started_at + chrono::Duration::seconds(3);
    let decision = evaluate_reuse(&request(), &record, &current(), policy);
    assert!(decision.is_reuse());
    assert_eq!(ReusePolicy::default().min_reuse_seconds, 60);
}

#[test]
fn every_rerun_reason_states_an_actionable_detail() {
    let reasons = [
        RerunReason::Ineligible(IneligibleReason::MissingCommand),
        RerunReason::Defect(EvidenceDefect::Missing),
        RerunReason::Mismatch {
            field: "commit_oid",
            recorded: COMMIT.to_string(),
            current: TREE.to_string(),
        },
        RerunReason::DirtyWorktree(vec![" M src/lib.rs".to_string()]),
        RerunReason::Unobservable("git rev-parse failed".to_string()),
        RerunReason::BelowDurationThreshold {
            elapsed: 2,
            threshold: 60,
        },
    ];
    for reason in reasons {
        assert!(!reason.code().is_empty());
        assert!(reason.detail().len() > 10, "{}", reason.code());
    }
}

// ---------------------------------------------------------------------------
// Self-reference: the sidecar may not invalidate its own binding
// ---------------------------------------------------------------------------

#[test]
fn evidence_directory_changes_do_not_make_the_worktree_dirty() {
    let status = format!(
        "?? {EVIDENCE_DIR}/\n?? {EVIDENCE_DIR}/bound-evidence-tests.json\n M {EVIDENCE_DIR}/bound-evidence-tests.log\n"
    );
    assert!(dirty_entries_excluding_evidence(&status).is_empty());
}

#[test]
fn any_change_outside_the_evidence_directory_is_dirty() {
    let status = format!("?? {EVIDENCE_DIR}/bound-evidence-tests.json\n M src/lib.rs\n");
    let dirty = dirty_entries_excluding_evidence(&status);
    assert_eq!(dirty, vec!["M src/lib.rs".to_string()]);
}

#[test]
fn a_rename_out_of_the_evidence_directory_is_dirty() {
    let status = format!("R  {EVIDENCE_DIR}/a.log -> src/leaked.log\n");
    assert_eq!(dirty_entries_excluding_evidence(&status).len(), 1);
}

// ---------------------------------------------------------------------------
// Store authority
// ---------------------------------------------------------------------------

#[test]
fn store_writes_a_self_ignoring_read_only_sidecar() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let store = EvidenceStore::new(workspace.path());
    store.store(&evidence()).expect("store");

    let ignore = workspace.path().join(EVIDENCE_DIR).join(".gitignore");
    assert_eq!(std::fs::read_to_string(&ignore).unwrap(), "*\n");

    let loaded = store.load("bound-evidence-tests").expect("load");
    assert_eq!(loaded, evidence());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let envelope = workspace.path().join(EvidenceStore::envelope_relative_path(
            "bound-evidence-tests",
        ));
        let mode = std::fs::metadata(&envelope).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o400);
    }
}

#[test]
fn store_replaces_a_previous_read_only_sidecar() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let store = EvidenceStore::new(workspace.path());
    store.store(&evidence()).expect("first store");
    let mut second = evidence();
    second.commit_oid = "abababababababababababababababababababab".to_string();
    store.store(&second).expect("second store");
    assert_eq!(
        store.load("bound-evidence-tests").unwrap().commit_oid,
        second.commit_oid
    );
}

#[test]
fn missing_sidecar_reports_missing_rather_than_failure() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let store = EvidenceStore::new(workspace.path());
    assert_eq!(
        store.load("bound-evidence-tests"),
        Err(EvidenceDefect::Missing)
    );
}

#[cfg(unix)]
#[test]
fn a_writable_envelope_is_reported_as_agent_authored() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().expect("tempdir");
    let store = EvidenceStore::new(workspace.path());
    store.ensure_directory().expect("directory");
    let envelope = workspace.path().join(EvidenceStore::envelope_relative_path(
        "bound-evidence-tests",
    ));
    std::fs::write(&envelope, serde_json::to_vec(&evidence()).unwrap()).expect("write");
    std::fs::set_permissions(&envelope, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    let defect = store.load("bound-evidence-tests").unwrap_err();
    assert_eq!(defect.code(), "evidence_agent_authored");
}

// ---------------------------------------------------------------------------
// Runtime executor with injected adapters
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct FakeFacts {
    commit: Arc<Mutex<String>>,
    tree: Arc<Mutex<String>>,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    status: Arc<Mutex<String>>,
    tool: Arc<Mutex<ToolIdentity>>,
    /// Digest returned for any hashed file, keyed by call order.
    digests: Arc<Mutex<Vec<String>>>,
    failure: Arc<Mutex<Option<String>>>,
}

impl FakeFacts {
    fn new() -> Self {
        let facts = Self {
            commit: Arc::new(Mutex::new(COMMIT.to_string())),
            tree: Arc::new(Mutex::new(TREE.to_string())),
            blobs: Arc::new(Mutex::new(HashMap::from([(
                "src/orchestration/acceptance.rs".to_string(),
                BLOB.to_string(),
            )]))),
            status: Arc::new(Mutex::new(String::new())),
            tool: Arc::new(Mutex::new(tool())),
            digests: Arc::new(Mutex::new(Vec::new())),
            failure: Arc::new(Mutex::new(None)),
        };
        facts
            .digests
            .lock()
            .unwrap()
            .push(ARTIFACT_DIGEST.to_string());
        facts
    }

    fn check(&self) -> Result<(), String> {
        match self.failure.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[async_trait]
impl RepositoryFacts for FakeFacts {
    async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
        self.check()?;
        Ok(self.commit.lock().unwrap().clone())
    }

    async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
        self.check()?;
        Ok(self.tree.lock().unwrap().clone())
    }

    async fn tracked_blob_oid(&self, _workspace: &Path, path: &str) -> Result<String, String> {
        self.check()?;
        self.blobs
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| format!("no blob for {path}"))
    }

    async fn hash_file(&self, _workspace: &Path, _path: &Path) -> Result<String, String> {
        self.check()?;
        let mut digests = self.digests.lock().unwrap();
        if digests.len() > 1 {
            Ok(digests.remove(0))
        } else {
            digests
                .first()
                .cloned()
                .ok_or_else(|| "no digest configured".to_string())
        }
    }

    async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
        self.check()?;
        Ok(self.status.lock().unwrap().clone())
    }

    async fn resolve_tool(
        &self,
        _workspace: &Path,
        _program: &str,
    ) -> Result<ToolIdentity, String> {
        self.check()?;
        Ok(self.tool.lock().unwrap().clone())
    }
}

#[derive(Clone)]
struct FakeSupervisor {
    exit_code: i32,
    output: Vec<u8>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    /// Mutation applied to the fake repository while the command "runs".
    during: Arc<dyn Fn() + Send + Sync>,
}

impl FakeSupervisor {
    fn succeeding() -> Self {
        Self {
            exit_code: 0,
            output: b"test result: ok".to_vec(),
            calls: Arc::new(Mutex::new(Vec::new())),
            during: Arc::new(|| {}),
        }
    }
}

#[async_trait]
impl CommandSupervisor for FakeSupervisor {
    async fn run(&self, argv: &[String], _cwd: &Path) -> Result<SupervisedOutcome, String> {
        self.calls.lock().unwrap().push(argv.to_vec());
        (self.during)();
        Ok(SupervisedOutcome {
            exit_code: self.exit_code,
            output: self.output.clone(),
        })
    }
}

/// Advances a fixed amount per observation so elapsed duration is deterministic.
struct StepClock {
    start: DateTime<Utc>,
    step: Arc<Mutex<i64>>,
}

impl StepClock {
    fn new() -> Self {
        Self {
            start: "2026-08-23T00:00:00Z".parse().unwrap(),
            step: Arc::new(Mutex::new(0)),
        }
    }
}

impl Clock for StepClock {
    fn now(&self) -> DateTime<Utc> {
        let mut step = self.step.lock().unwrap();
        let now = self.start + chrono::Duration::seconds(*step * 300);
        *step += 1;
        now
    }
}

fn executor(
    workspace: &Path,
    facts: FakeFacts,
    supervisor: FakeSupervisor,
) -> RuntimeVerificationExecutor<FakeFacts, FakeSupervisor, StepClock> {
    RuntimeVerificationExecutor::new(workspace.to_path_buf(), facts, supervisor, StepClock::new())
}

#[tokio::test]
async fn runtime_capture_writes_a_reusable_envelope() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    let supervisor = FakeSupervisor::succeeding();
    let outcome = executor(workspace.path(), facts.clone(), supervisor.clone())
        .capture(&request())
        .await;

    let CaptureOutcome::Captured(captured) = outcome else {
        panic!("expected capture, got {outcome:?}");
    };
    assert_eq!(captured.authority, EVIDENCE_AUTHORITY);
    assert_eq!(captured.commit_oid, COMMIT);
    assert_eq!(captured.exit_code, 0);
    assert_eq!(captured.elapsed_seconds(), Some(300));
    assert_eq!(supervisor.calls.lock().unwrap().len(), 1);

    let store = EvidenceStore::new(workspace.path());
    assert_eq!(store.load("bound-evidence-tests").unwrap(), *captured);
    let artifact = workspace.path().join(EvidenceStore::artifact_relative_path(
        "bound-evidence-tests",
    ));
    assert_eq!(std::fs::read(artifact).unwrap(), b"test result: ok");
}

#[tokio::test]
async fn a_failing_command_captures_no_envelope() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut supervisor = FakeSupervisor::succeeding();
    supervisor.exit_code = 101;
    let outcome = executor(workspace.path(), FakeFacts::new(), supervisor)
        .capture(&request())
        .await;

    assert!(matches!(
        outcome,
        CaptureOutcome::CommandFailed { exit_code: 101, .. }
    ));
    assert_eq!(
        EvidenceStore::new(workspace.path()).load("bound-evidence-tests"),
        Err(EvidenceDefect::Missing)
    );
}

#[tokio::test]
async fn capture_is_refused_when_the_worktree_is_dirty_before_execution() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    *facts.status.lock().unwrap() = " M src/lib.rs\n".to_string();
    let supervisor = FakeSupervisor::succeeding();
    let outcome = executor(workspace.path(), facts, supervisor.clone())
        .capture(&request())
        .await;

    assert!(matches!(
        outcome,
        CaptureOutcome::Refused(CaptureRefusal {
            code: "worktree_dirty",
            ..
        })
    ));
    assert!(
        supervisor.calls.lock().unwrap().is_empty(),
        "a dirty worktree must be refused before the command runs"
    );
}

#[tokio::test]
async fn capture_is_refused_when_a_binding_moves_while_the_command_runs() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    let moved = facts.commit.clone();
    let mut supervisor = FakeSupervisor::succeeding();
    supervisor.during = Arc::new(move || {
        *moved.lock().unwrap() = "cccccccccccccccccccccccccccccccccccccccc".to_string();
    });

    let outcome = executor(workspace.path(), facts, supervisor)
        .capture(&request())
        .await;
    let CaptureOutcome::Refused(refusal) = outcome else {
        panic!("expected refusal");
    };
    assert_eq!(refusal.code, "binding_moved_during_execution");
    assert_eq!(
        EvidenceStore::new(workspace.path()).load("bound-evidence-tests"),
        Err(EvidenceDefect::Missing)
    );
}

#[tokio::test]
async fn capture_is_refused_when_the_command_dirties_the_worktree() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    let status = facts.status.clone();
    let mut supervisor = FakeSupervisor::succeeding();
    supervisor.during = Arc::new(move || {
        *status.lock().unwrap() = " M src/generated.rs\n".to_string();
    });

    let outcome = executor(workspace.path(), facts, supervisor)
        .capture(&request())
        .await;
    assert!(matches!(
        outcome,
        CaptureOutcome::Refused(CaptureRefusal {
            code: "worktree_dirty",
            ..
        })
    ));
}

#[tokio::test]
async fn capture_is_refused_when_repository_state_cannot_be_observed() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    *facts.failure.lock().unwrap() = Some("git rev-parse failed".to_string());
    let outcome = executor(workspace.path(), facts, FakeSupervisor::succeeding())
        .capture(&request())
        .await;
    assert!(matches!(
        outcome,
        CaptureOutcome::Refused(CaptureRefusal {
            code: "state_unobservable",
            ..
        })
    ));
}

// ---------------------------------------------------------------------------
// End-to-end reuse decisions over the store
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captured_evidence_is_reused_on_the_next_evaluation() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;

    let plan = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.reused_ids(), vec!["bound-evidence-tests"]);
    assert!(plan.rerun_ids().is_empty());
    assert_eq!(plan.to_json()["verifications"][0]["outcome"], "reused");
}

#[tokio::test]
async fn a_new_commit_forces_rerun_of_previously_captured_evidence() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;
    *facts.commit.lock().unwrap() = "dddddddddddddddddddddddddddddddddddddddd".to_string();

    let plan = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.rerun_ids(), vec!["bound-evidence-tests"]);
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "binding_mismatch"
    );
}

#[tokio::test]
async fn evidence_from_a_dirty_worktree_is_not_reused() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;
    *facts.status.lock().unwrap() = " M src/lib.rs\n".to_string();

    let plan = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "worktree_dirty"
    );
}

#[tokio::test]
async fn the_written_sidecar_does_not_invalidate_its_own_binding() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;
    // The only difference from the bound commit is the sidecar the runtime just
    // wrote, reported the way `git status` would report an untracked directory.
    *facts.status.lock().unwrap() = format!("?? {EVIDENCE_DIR}/\n");

    let plan = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.reused_ids(), vec!["bound-evidence-tests"]);
}

#[tokio::test]
async fn missing_evidence_reruns_with_an_actionable_reason() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let plan = plan_reuse(
        &FakeFacts::new(),
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "evidence_missing"
    );
    assert!(plan.to_json()["verifications"][0]["detail"]
        .as_str()
        .unwrap()
        .contains("runtime-authored"));
}

#[tokio::test]
async fn a_deployed_observation_has_no_local_reuse_decision() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut declaration = declaration();
    declaration.execution_class = Some("deployed-service".to_string());
    declaration.completion_role = Some("operational-observation".to_string());
    let plan = plan_reuse(
        &FakeFacts::new(),
        workspace.path(),
        &[declaration],
        ReusePolicy::default(),
    )
    .await;
    assert!(plan.is_empty());
}

#[tokio::test]
async fn unobservable_repository_state_reruns_rather_than_reusing() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;
    *facts.failure.lock().unwrap() = Some("git rev-parse failed".to_string());

    let plan = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "state_unobservable"
    );
}

#[tokio::test]
async fn restart_derives_the_same_decision_from_the_worktree_alone() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(
        workspace.path(),
        facts.clone(),
        FakeSupervisor::succeeding(),
    )
    .capture(&request())
    .await;

    // A "restart" is nothing more than dropping every in-memory structure and
    // reading the same worktree again: no cache, no store handle, no prior plan.
    let first = plan_reuse(
        &facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    let restarted_facts = FakeFacts::new();
    let second = plan_reuse(
        &restarted_facts,
        workspace.path(),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(first, second);
    assert_eq!(second.reused_ids(), vec!["bound-evidence-tests"]);
}

#[tokio::test]
async fn duplicate_declared_ids_produce_one_decision() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let plan = plan_reuse(
        &FakeFacts::new(),
        workspace.path(),
        &[declaration(), declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.decisions.len(), 1);
}

#[test]
fn a_tool_that_reports_no_version_records_none() {
    assert_eq!(
        usable_tool_version("  cargo 1.80.0\n"),
        Some("cargo 1.80.0".to_string())
    );
    // `/bin/echo --version` echoes the flag; that is not a version.
    assert_eq!(usable_tool_version("--version\n"), None);
    assert_eq!(usable_tool_version("   \n"), None);
}
