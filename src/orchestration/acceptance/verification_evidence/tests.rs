//! Adversarial coverage for bound verification evidence reuse.
//!
//! Every test here is a way an unbound, stale, forged, or self-referential
//! record could otherwise turn "no evidence" into PASS. The Git and process
//! boundaries are in-memory doubles, so these stay unit-scoped: the only real
//! filesystem use is a temporary directory holding the records the module is
//! defined to read from disk.
//!
//! The target tree and the Conflux-owned store are separate directories in
//! every case, because that separation is now the contract: nothing this module
//! writes may land inside the tree it is verifying.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::*;
use crate::openspec::VerificationDeclaration;

const COMMIT: &str = "1111111111111111111111111111111111111111";
const TREE: &str = "2222222222222222222222222222222222222222";
const BLOB: &str = "3333333333333333333333333333333333333333";
const TOOL_DIGEST: &str = "4444444444444444444444444444444444444444";
const ARTIFACT_DIGEST: &str = "5555555555555555555555555555555555555555";
const REVIEW_BASE: &str = "6666666666666666666666666666666666666666";

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
        review_base_commit: REVIEW_BASE.to_string(),
        review_range: format!("{REVIEW_BASE}..{COMMIT}"),
        argv: vec![
            "cargo".to_string(),
            "test".to_string(),
            "orchestration::acceptance".to_string(),
            "--lib".to_string(),
        ],
        env_names: Vec::new(),
        env_binding_digest: empty_env_binding_digest(),
        cwd: ".".to_string(),
        automation_path: "src/orchestration/acceptance.rs".to_string(),
        automation_blob_oid: BLOB.to_string(),
        tool: tool(),
        started_at,
        ended_at: started_at + chrono::Duration::seconds(300),
        exit_code: 0,
        artifact_path: EvidenceStore::artifact_relative_path("bound-evidence-tests"),
        artifact_digest: ARTIFACT_DIGEST.to_string(),
        clean_before: true,
        clean_after: true,
    }
}

fn current() -> CurrentBindings {
    CurrentBindings {
        commit_oid: COMMIT.to_string(),
        tree_oid: TREE.to_string(),
        review: Some(ReviewBinding {
            base_commit: REVIEW_BASE.to_string(),
            range: format!("{REVIEW_BASE}..{COMMIT}"),
        }),
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

/// The target tree under test. Always a *sibling* of the store below, never its
/// ancestor, so an accidental target-local write would be visible as an entry
/// somewhere it does not belong.
fn target_of(temp: &tempfile::TempDir) -> PathBuf {
    let target = temp.path().join("target");
    std::fs::create_dir_all(&target).expect("create target");
    target
}

/// The Conflux-owned external store, shaped like the real layout.
fn store_of(temp: &tempfile::TempDir) -> EvidenceStore {
    EvidenceStore::new(
        temp.path()
            .join("cflx-state/acceptance/project-slug/workspace-slug/change-a"),
    )
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

/// The artifact path is not merely constrained to a prefix — it must be exactly
/// the store-relative path this verification's artifact would occupy. Anything
/// else is a record describing a file the store does not own, whether it points
/// into the target, at another gate, or nowhere at all.
#[test]
fn an_artifact_path_the_store_does_not_own_is_refused() {
    for hostile in [
        "target/output.log",
        "/absolute/output.log",
        "gates/../escape.log",
        "gates/some-other-gate.log",
    ] {
        let mut record = evidence();
        record.artifact_path = hostile.to_string();
        let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
        assert_eq!(defect.code(), "evidence_malformed", "'{hostile}'");
        assert!(
            defect.detail().contains("store-relative"),
            "'{hostile}': {}",
            defect.detail()
        );
    }
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
// Cleanliness: no Conflux-owned path is excluded any more
// ---------------------------------------------------------------------------

/// The exclusion is gone, not relaxed. Constitutional law 4 says a cleanliness
/// exception does not make a Conflux-owned write permissible, so the honest
/// implementation has no exception at all — and since nothing lands in the
/// target now, there is nothing left for one to protect.
#[test]
fn a_conflux_owned_path_in_the_target_is_dirty_like_any_other() {
    let status = format!(
        "?? {LEGACY_EVIDENCE_DIR}/\n\
         ?? {LEGACY_EVIDENCE_DIR}/bound-evidence-tests.json\n\
          M {LEGACY_EVIDENCE_DIR}/bound-evidence-tests.log\n"
    );
    assert_eq!(
        dirty_entries(&status).len(),
        3,
        "no path is exempt from the tracked/untracked porcelain status"
    );
}

#[test]
fn every_status_entry_counts_as_dirty() {
    let status = " M src/lib.rs\n?? untracked.txt\nA  staged.rs\n";
    assert_eq!(
        dirty_entries(status),
        vec![
            "M src/lib.rs".to_string(),
            "?? untracked.txt".to_string(),
            "A  staged.rs".to_string(),
        ]
    );
}

#[test]
fn a_clean_status_has_no_dirty_entries() {
    assert!(dirty_entries("").is_empty());
}

/// A rename is one entry, whichever side of it you look at: there is no longer
/// a rule that inspects the paths at all.
#[test]
fn a_rename_is_one_dirty_entry() {
    assert_eq!(dirty_entries("R  a.log -> src/leaked.log\n").len(), 1);
}

// ---------------------------------------------------------------------------
// Store authority
// ---------------------------------------------------------------------------

/// The envelope is read-only and runtime-owned, it lives in the external store,
/// and — the part this change adds — no ignore file is written anywhere. Hiding
/// an artifact was never what made one permissible; not writing it into the
/// target is.
#[test]
fn store_writes_a_read_only_envelope_outside_the_target() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let target = target_of(&workspace);
    let store = store_of(&workspace);
    store.store(&evidence()).expect("store");

    let loaded = store.load("bound-evidence-tests").expect("load");
    assert_eq!(loaded, evidence());

    assert!(
        !store.root().starts_with(&target),
        "the store must not be inside the target: {:?}",
        store.root()
    );
    assert_eq!(
        std::fs::read_dir(&target)
            .expect("read target")
            .filter_map(Result::ok)
            .count(),
        0,
        "storing evidence must write nothing into the target"
    );
    assert!(
        !store.directory().join(".gitignore").exists() && !store.root().join(".gitignore").exists(),
        "no self-ignoring file may be written any more"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let envelope = store.root().join(EvidenceStore::envelope_relative_path(
            "bound-evidence-tests",
        ));
        let mode = std::fs::metadata(&envelope).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o400);
    }
}

/// The store-relative artifact path is part of the record's validity: an
/// envelope naming a repository-relative path — the version-1 shape — is
/// malformed rather than migrated.
#[test]
fn a_target_relative_artifact_path_is_malformed() {
    let mut record = evidence();
    record.artifact_path = format!("{LEGACY_EVIDENCE_DIR}/bound-evidence-tests.log");
    let defect = parse_evidence(&serde_json::to_vec(&record).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "evidence_malformed");
    assert!(
        defect.detail().contains("store-relative"),
        "{}",
        defect.detail()
    );
}

/// Legacy detection is existence, nothing more: no read, no parse, no delete.
#[test]
fn legacy_target_evidence_is_detected_by_existence_alone() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let target = target_of(&workspace);
    assert!(legacy_target_evidence(&target).is_none());

    let legacy = target.join(LEGACY_EVIDENCE_DIR);
    std::fs::create_dir_all(&legacy).expect("create legacy");
    std::fs::write(legacy.join("focused-gate.json"), b"unreadable").expect("write legacy");

    let found = legacy_target_evidence(&target).expect("existing legacy evidence is reported");
    assert_eq!(found, legacy);
    assert!(
        legacy.join("focused-gate.json").is_file(),
        "the probe must not delete what it finds"
    );
}

#[test]
fn store_replaces_a_previous_read_only_sidecar() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let store = store_of(&workspace);
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
    let store = store_of(&workspace);
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
    let store = store_of(&workspace);
    store.ensure_directory().expect("directory");
    let envelope = store.root().join(EvidenceStore::envelope_relative_path(
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
    /// Bindings handed to each call, so parser-to-supervisor wiring is asserted.
    envs: Arc<Mutex<Vec<EnvBindings>>>,
    /// Mutation applied to the fake repository while the command "runs".
    during: Arc<dyn Fn() + Send + Sync>,
}

impl FakeSupervisor {
    fn succeeding() -> Self {
        Self {
            exit_code: 0,
            output: b"test result: ok".to_vec(),
            calls: Arc::new(Mutex::new(Vec::new())),
            envs: Arc::new(Mutex::new(Vec::new())),
            during: Arc::new(|| {}),
        }
    }
}

#[async_trait]
impl CommandSupervisor for FakeSupervisor {
    async fn run(
        &self,
        argv: &[String],
        env: &EnvBindings,
        _cwd: &Path,
    ) -> Result<SupervisedOutcome, String> {
        self.calls.lock().unwrap().push(argv.to_vec());
        self.envs.lock().unwrap().push(env.clone());
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

/// An executor bound to the target it runs in and the external store it writes
/// to — two separate directories, as production always has them.
fn executor(
    temp: &tempfile::TempDir,
    facts: FakeFacts,
    supervisor: FakeSupervisor,
) -> RuntimeVerificationExecutor<FakeFacts, FakeSupervisor, StepClock> {
    RuntimeVerificationExecutor::new(
        target_of(temp),
        store_of(temp),
        facts,
        supervisor,
        StepClock::new(),
    )
}

#[tokio::test]
async fn runtime_capture_writes_a_reusable_envelope() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    let supervisor = FakeSupervisor::succeeding();
    let outcome = executor(&workspace, facts.clone(), supervisor.clone())
        .capture(&request())
        .await;

    let CaptureOutcome::Captured {
        evidence: captured,
        artifact_location,
    } = &outcome
    else {
        panic!("expected capture, got {outcome:?}");
    };
    assert_eq!(captured.authority, EVIDENCE_AUTHORITY);
    assert_eq!(captured.commit_oid, COMMIT);
    assert_eq!(captured.exit_code, 0);
    assert_eq!(captured.elapsed_seconds(), Some(300));
    assert_eq!(supervisor.calls.lock().unwrap().len(), 1);

    let store = store_of(&workspace);
    assert_eq!(store.load("bound-evidence-tests").unwrap(), **captured);
    let artifact = store.artifact_path("bound-evidence-tests");
    assert_eq!(std::fs::read(&artifact).unwrap(), b"test result: ok");

    // The envelope keeps the store-relative spelling; the report carries the
    // path that actually opens.
    assert_eq!(
        captured.artifact_path,
        EvidenceStore::artifact_relative_path("bound-evidence-tests")
    );
    assert_eq!(artifact_location, &artifact);
    assert_eq!(outcome.artifact_location(), Some(artifact.as_path()));
    assert_eq!(
        outcome.to_json()["artifact_location"].as_str(),
        Some(artifact.display().to_string().as_str())
    );
}

#[tokio::test]
async fn a_failing_command_captures_no_envelope() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut supervisor = FakeSupervisor::succeeding();
    supervisor.exit_code = 101;
    supervisor.output = b"error: 1 test failed".to_vec();
    let outcome = executor(&workspace, FakeFacts::new(), supervisor)
        .capture(&request())
        .await;

    assert!(matches!(
        outcome,
        CaptureOutcome::CommandFailed { exit_code: 101, .. }
    ));
    assert_eq!(
        store_of(&workspace).load("bound-evidence-tests"),
        Err(EvidenceDefect::Missing)
    );
}

#[tokio::test]
async fn a_failing_command_reports_an_artifact_path_that_opens() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut supervisor = FakeSupervisor::succeeding();
    supervisor.exit_code = 101;
    supervisor.output = b"error: 1 test failed".to_vec();
    let outcome = executor(&workspace, FakeFacts::new(), supervisor)
        .capture(&request())
        .await;

    let CaptureOutcome::CommandFailed {
        artifact_path,
        artifact_location,
        ..
    } = &outcome
    else {
        panic!("expected a command failure, got {outcome:?}");
    };

    // The store-relative spelling stays exactly what the envelope schema uses,
    // and resolves to nothing on its own.
    assert_eq!(
        artifact_path,
        &EvidenceStore::artifact_relative_path("bound-evidence-tests")
    );
    assert!(!Path::new(artifact_path).is_absolute());

    // The reported location is the real file under the external store root, not
    // a bare string: a caller can read the failing output from it directly.
    let store = store_of(&workspace);
    assert!(artifact_location.is_absolute());
    assert!(artifact_location.starts_with(store.root()));
    assert!(!artifact_location.starts_with(target_of(&workspace)));
    assert_eq!(
        std::fs::read(artifact_location).expect("the reported artifact exists on disk"),
        b"error: 1 test failed"
    );

    // And the same location travels on the machine-readable report.
    let reported = outcome.to_json()["artifact_location"]
        .as_str()
        .expect("reports name an openable artifact")
        .to_string();
    assert_eq!(
        std::fs::read(&reported).expect("the reported location is readable"),
        b"error: 1 test failed"
    );
}

#[tokio::test]
async fn capture_is_refused_when_the_worktree_is_dirty_before_execution() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    *facts.status.lock().unwrap() = " M src/lib.rs\n".to_string();
    let supervisor = FakeSupervisor::succeeding();
    let outcome = executor(&workspace, facts, supervisor.clone())
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

    let outcome = executor(&workspace, facts, supervisor)
        .capture(&request())
        .await;
    let CaptureOutcome::Refused(refusal) = outcome else {
        panic!("expected refusal");
    };
    assert_eq!(refusal.code, "binding_moved_during_execution");
    assert_eq!(
        store_of(&workspace).load("bound-evidence-tests"),
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

    let outcome = executor(&workspace, facts, supervisor)
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
    let outcome = executor(&workspace, facts, FakeSupervisor::succeeding())
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
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;

    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
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
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;
    *facts.commit.lock().unwrap() = "dddddddddddddddddddddddddddddddddddddddd".to_string();

    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
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
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;
    *facts.status.lock().unwrap() = " M src/lib.rs\n".to_string();

    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "worktree_dirty"
    );
}

/// The old self-reference problem is solved by *relocation*, not by an
/// exception. A capture writes into the external store, so the target's status
/// is untouched and stays clean, and the very next evaluation reuses the record
/// without anything having to be excluded from the comparison.
#[tokio::test]
async fn a_capture_leaves_the_target_clean_without_any_exclusion() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let target = target_of(&workspace);
    let facts = FakeFacts::new();
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;

    // `FakeFacts` reports the status the runtime would observe: still empty,
    // because nothing was written into the target at all.
    assert!(facts.status.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(&target)
            .expect("read target")
            .filter_map(Result::ok)
            .count(),
        0,
        "a capture must write nothing into the target"
    );
    assert!(
        store_of(&workspace)
            .artifact_path("bound-evidence-tests")
            .is_file(),
        "the artifact belongs in the external store"
    );

    let plan = plan_reuse(
        &facts,
        &target,
        &store_of(&workspace),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.reused_ids(), vec!["bound-evidence-tests"]);
}

/// Deleting the external cache reruns the declared gate. It never proves PASS,
/// and it never changes what the next workflow action would be.
#[tokio::test]
async fn a_deleted_cache_reruns_rather_than_proving_anything() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;
    assert_eq!(
        plan_reuse(
            &facts,
            &target_of(&workspace),
            &store_of(&workspace),
            &[declaration()],
            ReusePolicy::default(),
        )
        .await
        .reused_ids(),
        vec!["bound-evidence-tests"]
    );

    std::fs::remove_dir_all(store_of(&workspace).root()).expect("delete the cache");

    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.rerun_ids(), vec!["bound-evidence-tests"]);
    assert_eq!(
        plan.to_json()["verifications"][0]["reason"],
        "evidence_missing",
        "cache loss is a rerun reason, never a verdict"
    );
}

#[tokio::test]
async fn missing_evidence_reruns_with_an_actionable_reason() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let plan = plan_reuse(
        &FakeFacts::new(),
        &target_of(&workspace),
        &store_of(&workspace),
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
        &target_of(&workspace),
        &store_of(&workspace),
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
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;
    *facts.failure.lock().unwrap() = Some("git rev-parse failed".to_string());

    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
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
    executor(&workspace, facts.clone(), FakeSupervisor::succeeding())
        .capture(&request())
        .await;

    // A "restart" is nothing more than dropping every in-memory structure and
    // reading the same worktree again: no cache, no store handle, no prior plan.
    let first = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store_of(&workspace),
        &[declaration()],
        ReusePolicy::default(),
    )
    .await;
    let restarted_facts = FakeFacts::new();
    let second = plan_reuse(
        &restarted_facts,
        &target_of(&workspace),
        &store_of(&workspace),
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
        &target_of(&workspace),
        &store_of(&workspace),
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

// ---------------------------------------------------------------------------
// Leading environment assignments (`rerun_leading_env`)
// ---------------------------------------------------------------------------

/// A value no Conflux-generated metadata may ever contain.
const ENV_SENTINEL: &str = "rerun-leading-env-sentinel-7f3a9c";

fn declaration_with(command: &str) -> VerificationDeclaration {
    VerificationDeclaration {
        rerun: Some(command.to_string()),
        evidence: Some(command.to_string()),
        ..declaration()
    }
}

fn request_with(command: &str) -> VerificationRequest {
    eligible_request(&declaration_with(command))
        .unwrap_or_else(|reason| panic!("'{command}' must be eligible: {}", reason.code()))
}

fn bindings_of(request: &VerificationRequest) -> Vec<(String, String)> {
    request.env.pairs().to_vec()
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

#[test]
fn rerun_leading_env_parses_quoting_empty_values_duplicates_and_explicit_env() {
    type Case<'a> = (&'a str, Vec<(String, String)>, Vec<&'a str>);
    let cases: Vec<Case> = vec![
        (
            "'A=hello world' cmd",
            vec![pair("A", "hello world")],
            vec!["cmd"],
        ),
        ("A=x=y cmd", vec![pair("A", "x=y")], vec!["cmd"]),
        ("A= cmd", vec![pair("A", "")], vec!["cmd"]),
        (
            "A=1 B=2 cmd",
            vec![pair("A", "1"), pair("B", "2")],
            vec!["cmd"],
        ),
        ("A=1 A=2 cmd", vec![pair("A", "2")], vec!["cmd"]),
        ("env A=1 cmd", vec![], vec!["env", "A=1", "cmd"]),
        ("cmd a=b", vec![], vec!["cmd", "a=b"]),
        (
            "_X1=v cargo test --lib k=v",
            vec![pair("_X1", "v")],
            vec!["cargo", "test", "--lib", "k=v"],
        ),
    ];
    for (command, bindings, argv) in cases {
        let request = request_with(command);
        assert_eq!(bindings_of(&request), bindings, "bindings of '{command}'");
        assert_eq!(request.argv, argv, "argv of '{command}'");
    }
}

#[test]
fn rerun_leading_env_invalid_names_and_assignment_only_are_declaration_defects() {
    for command in ["1ABC=x cmd", "A-B=x cmd", "=x cmd", "A=1 9B=2 cmd"] {
        let reason = eligible_request(&declaration_with(command)).unwrap_err();
        assert_eq!(
            reason.code(),
            "declaration_invalid_assignment",
            "'{command}'"
        );
    }
    let reason =
        eligible_request(&declaration_with(&format!("1BAD={ENV_SENTINEL} cmd"))).unwrap_err();
    assert!(
        !reason.detail().contains(ENV_SENTINEL),
        "detail never echoes text"
    );
    assert!(!reason.detail().contains("1BAD"), "{}", reason.detail());
    for command in ["A=1", "A=1 B=2"] {
        let reason = eligible_request(&declaration_with(command)).unwrap_err();
        assert_eq!(reason.code(), "declaration_missing_command", "'{command}'");
    }
}

#[test]
fn rerun_leading_env_shell_constructs_are_still_shell_syntax() {
    for command in [
        "FOO=$BAR cmd",
        "A=1 cmd | tee out",
        "A=1 cmd > out",
        "A=1 cmd && other",
        "A=$(id) cmd",
    ] {
        let reason = eligible_request(&declaration_with(command)).unwrap_err();
        assert_eq!(reason.code(), "declaration_shell_syntax", "'{command}'");
    }
}

#[test]
fn rerun_leading_env_digest_is_canonical_and_identity_bearing() {
    let digest = |command: &str| request_with(command).env.digest();
    // The empty-bindings digest is SHA-256 of the empty byte string.
    assert_eq!(
        empty_env_binding_digest(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(digest("cmd"), empty_env_binding_digest());
    // Canonical order: assignment order does not matter, the last duplicate does.
    assert_eq!(digest("A=1 B=2 cmd"), digest("B=2 A=1 cmd"));
    assert_eq!(digest("A=0 A=1 cmd"), digest("A=1 cmd"));
    let mut hasher = Sha256::new();
    hasher.update(b"A\x001\x00B\x002\x00");
    assert_eq!(digest("A=1 B=2 cmd"), format!("{:x}", hasher.finalize()));
    // Any value, name, or presence change moves it.
    for other in ["A=2 cmd", "A=1 B=1 cmd", "cmd", "AA=1 cmd"] {
        assert_ne!(digest("A=1 cmd"), digest(other), "'{other}'");
    }
    assert_eq!(request_with("B=2 A=1 cmd").env.names(), vec!["A", "B"]);
}

#[test]
fn rerun_leading_env_redacted_rendering_and_debug_never_show_values() {
    let request = request_with(&format!("SECRET={ENV_SENTINEL} OTHER=x cargo test --lib"));
    assert_eq!(
        request.render(),
        "OTHER=<redacted> SECRET=<redacted> cargo test --lib"
    );
    let debug = format!("{request:?}");
    assert!(!debug.contains(ENV_SENTINEL), "{debug}");
    assert!(debug.contains("SECRET=<redacted>"), "{debug}");
}

#[tokio::test]
async fn rerun_leading_env_bindings_reach_the_supervisor_and_the_envelope_by_digest() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let facts = FakeFacts::new();
    let supervisor = FakeSupervisor::succeeding();
    let request = request_with(&format!(
        "DJANGO_SETTINGS_MODULE={ENV_SENTINEL} cargo test orchestration::acceptance --lib"
    ));
    let outcome = executor(&workspace, facts.clone(), supervisor.clone())
        .capture(&request)
        .await;
    let CaptureOutcome::Captured { evidence, .. } = &outcome else {
        panic!("expected capture, got {outcome:?}");
    };

    // The supervisor was handed the executable argv and the bindings apart.
    assert_eq!(
        *supervisor.calls.lock().unwrap(),
        vec![request.argv.clone()]
    );
    assert_eq!(
        supervisor.envs.lock().unwrap()[0].pairs(),
        [pair("DJANGO_SETTINGS_MODULE", ENV_SENTINEL)]
    );
    assert_eq!(evidence.argv[0], "cargo");
    assert_eq!(evidence.env_names, vec!["DJANGO_SETTINGS_MODULE"]);
    assert_eq!(evidence.env_binding_digest, request.env.digest());

    // Neither the stored envelope nor any report carries the value.
    let store = store_of(&workspace);
    let stored = std::fs::read_to_string(
        store
            .directory()
            .join(format!("{}.json", request.verification_id)),
    )
    .expect("envelope written");
    assert!(stored.contains("DJANGO_SETTINGS_MODULE"));
    assert!(!stored.contains(ENV_SENTINEL), "{stored}");
    assert!(!outcome.to_json().to_string().contains(ENV_SENTINEL));
    assert!(!format!("{outcome:?}").contains(ENV_SENTINEL));

    // The same declaration reuses it; reports stay value-free.
    let plan = plan_reuse(
        &facts,
        &target_of(&workspace),
        &store,
        &[declaration_with(&format!(
            "DJANGO_SETTINGS_MODULE={ENV_SENTINEL} cargo test orchestration::acceptance --lib"
        ))],
        ReusePolicy::default(),
    )
    .await;
    assert_eq!(plan.reused_ids(), vec!["bound-evidence-tests"]);
    for decision in &plan.decisions {
        assert!(!decision.summary().contains(ENV_SENTINEL));
    }
    assert!(!plan.to_json().to_string().contains(ENV_SENTINEL));
}

#[test]
fn rerun_leading_env_binding_change_reruns_with_env_binding_digest() {
    let recorded = request_with("A=1 cargo test orchestration::acceptance --lib");
    let record = VerificationEvidence {
        env_names: recorded.env.names(),
        env_binding_digest: recorded.env.digest(),
        ..evidence()
    };
    assert!(
        evaluate_reuse(&recorded, &record, &current(), ReusePolicy::default()).is_reuse(),
        "the exact bindings reuse"
    );
    for changed in [
        format!("A={ENV_SENTINEL} cargo test orchestration::acceptance --lib"),
        "A=1 B=1 cargo test orchestration::acceptance --lib".to_string(),
        "cargo test orchestration::acceptance --lib".to_string(),
    ] {
        let decision = evaluate_reuse(
            &request_with(&changed),
            &record,
            &current(),
            ReusePolicy::default(),
        );
        assert_eq!(
            mismatch_field(&decision),
            "env_binding_digest",
            "'{changed}'"
        );
        assert!(!decision.summary().contains(ENV_SENTINEL));
        assert!(!decision.to_json().to_string().contains(ENV_SENTINEL));
    }
}

#[test]
fn rerun_leading_env_pre_field_envelope_reads_as_empty_bindings() {
    let mut value = serde_json::to_value(evidence()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("env_names");
    object.remove("env_binding_digest");
    let legacy = parse_evidence(&serde_json::to_vec(&value).unwrap()).expect("still parses");
    assert_eq!(legacy.env_binding_digest, empty_env_binding_digest());
    assert!(legacy.env_names.is_empty());

    assert!(
        evaluate_reuse(&request(), &legacy, &current(), ReusePolicy::default()).is_reuse(),
        "an assignment-free declaration still reuses a pre-field envelope"
    );
    let with_env = request_with("A=1 cargo test orchestration::acceptance --lib");
    assert_eq!(
        mismatch_field(&evaluate_reuse(
            &with_env,
            &legacy,
            &current(),
            ReusePolicy::default()
        )),
        "env_binding_digest"
    );
}

#[test]
fn rerun_leading_env_short_binding_digest_is_malformed() {
    let record = VerificationEvidence {
        env_binding_digest: "abc".to_string(),
        ..evidence()
    };
    let bytes = serde_json::to_vec(&record).unwrap();
    assert!(matches!(
        parse_evidence(&bytes),
        Err(EvidenceDefect::Malformed(_))
    ));
}

/// Real child process, production supervisor, no shell.
#[tokio::test]
async fn rerun_leading_env_direct_supervisor_applies_bindings_to_the_child_only() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let request =
        request_with("DJANGO_SETTINGS_MODULE=config.test_settings printenv DJANGO_SETTINGS_MODULE");
    assert_eq!(request.argv, vec!["printenv", "DJANGO_SETTINGS_MODULE"]);
    assert!(std::env::var_os("DJANGO_SETTINGS_MODULE").is_none());

    let with = DirectCommandSupervisor
        .run(&request.argv, &request.env, workspace.path())
        .await
        .expect("printenv spawns");
    assert_eq!(with.exit_code, 0);
    assert_eq!(
        String::from_utf8_lossy(&with.output).trim(),
        "config.test_settings"
    );

    let without = DirectCommandSupervisor
        .run(&request.argv, &EnvBindings::default(), workspace.path())
        .await
        .expect("printenv spawns");
    assert_ne!(without.exit_code, 0, "unset without the binding");

    assert!(
        std::env::var_os("DJANGO_SETTINGS_MODULE").is_none(),
        "the owner environment is unchanged"
    );
}

/// Bound supervision of a binding-carrying child keeps the deadline path.
#[tokio::test]
async fn rerun_leading_env_bounded_supervision_still_terminates_the_group() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let request = request_with("GATE_MODE=slow sleep 30");
    let run = DirectCommandSupervisor
        .run_bounded(
            &request.argv,
            &request.env,
            workspace.path(),
            Some(Duration::from_millis(100)),
        )
        .await
        .expect("sleep spawns");
    let SupervisedRun::DeadlineExhausted(report) = run else {
        panic!("expected deadline exhaustion, got {run:?}");
    };
    assert!(report.is_confirmed(), "{}", report.diagnostics());
}
