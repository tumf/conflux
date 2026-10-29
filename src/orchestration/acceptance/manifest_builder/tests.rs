//! Manifest derivation regressions.
//!
//! The property under test is constitutional law 1: everything the manifest
//! binds comes from workspace file state, workspace Git state, and base-tree
//! comparison. The Git side is an in-memory double so these stay unit-scoped;
//! the file side is a real temporary directory because "which files were read"
//! is exactly what must not drift.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::*;
use crate::orchestration::acceptance::verification_evidence::{EvidenceStore, ToolIdentity};

const COMMIT: &str = "1111111111111111111111111111111111111111";
const TREE: &str = "2222222222222222222222222222222222222222";
const BASE: &str = "3333333333333333333333333333333333333333";
const BLOB: &str = "4444444444444444444444444444444444444444";
const TOOL_DIGEST: &str = "5555555555555555555555555555555555555555555555555555555555555555";

#[derive(Clone)]
struct FakeFacts {
    commit: String,
    tree: String,
    revisions: HashMap<String, String>,
    blobs: HashMap<String, String>,
    tool: Option<ToolIdentity>,
    artifact_digest: Option<String>,
    resolve_calls: Arc<Mutex<Vec<String>>>,
    /// Every program `resolve_tool` was asked for.
    tool_programs: Arc<Mutex<Vec<String>>>,
}

impl FakeFacts {
    fn new() -> Self {
        Self {
            commit: COMMIT.to_string(),
            tree: TREE.to_string(),
            revisions: HashMap::from([("main".to_string(), BASE.to_string())]),
            blobs: HashMap::from([("src/lib.rs".to_string(), BLOB.to_string())]),
            tool: Some(ToolIdentity {
                path: "/usr/bin/cargo".to_string(),
                executable_digest: TOOL_DIGEST.to_string(),
                version: Some("cargo 1.80.0".to_string()),
            }),
            artifact_digest: None,
            resolve_calls: Arc::new(Mutex::new(Vec::new())),
            tool_programs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl RepositoryFacts for FakeFacts {
    async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
        Ok(self.commit.clone())
    }
    async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
        Ok(self.tree.clone())
    }
    async fn resolve_revision(&self, _workspace: &Path, revision: &str) -> Result<String, String> {
        self.resolve_calls
            .lock()
            .unwrap()
            .push(revision.to_string());
        self.revisions
            .get(revision)
            .cloned()
            .ok_or_else(|| format!("unknown revision {revision}"))
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
            .ok_or_else(|| "no artifact".to_string())
    }
    async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
        Ok(String::new())
    }
    async fn resolve_tool(&self, _workspace: &Path, program: &str) -> Result<ToolIdentity, String> {
        self.tool_programs.lock().unwrap().push(program.to_string());
        self.tool
            .clone()
            .ok_or_else(|| "executable is not on PATH".to_string())
    }
}

/// An unobservable repository: every fact fails.
struct BlindFacts;

#[async_trait]
impl RepositoryFacts for BlindFacts {
    async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
        Err("git rev-parse HEAD failed".to_string())
    }
    async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
        Err("git rev-parse HEAD^{tree} failed".to_string())
    }
    async fn tracked_blob_oid(&self, _workspace: &Path, _path: &str) -> Result<String, String> {
        Err("no blob".to_string())
    }
    async fn hash_file(&self, _workspace: &Path, _path: &Path) -> Result<String, String> {
        Err("no file".to_string())
    }
    async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
        Err("no status".to_string())
    }
    async fn resolve_tool(
        &self,
        _workspace: &Path,
        _program: &str,
    ) -> Result<ToolIdentity, String> {
        Err("no tool".to_string())
    }
}

const PROPOSAL: &str = "---\n\
verifications:\n\
\x20 - id: focused-gate\n\
\x20   requirement: the focused gate passes\n\
\x20   phase: pre-integration\n\
\x20   owner: conflux\n\
\x20   trigger: pull-request-validation\n\
\x20   automation: src/lib.rs\n\
\x20   evidence: cargo test focused --lib\n\
\x20   rerun: cargo test focused --lib\n\
\x20   prerequisites: []\n\
\x20   execution_class: repository-local\n\
\x20   completion_role: change-blocking\n\
---\n\
# Change\n";

fn workspace_with_change(change_id: &str) -> tempfile::TempDir {
    let workspace = tempfile::tempdir().unwrap();
    let change_dir = workspace.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(change_dir.join("specs/parallel-execution")).unwrap();
    std::fs::write(change_dir.join("proposal.md"), PROPOSAL).unwrap();
    std::fs::write(change_dir.join("tasks.md"), "- [x] Do the thing\n").unwrap();
    std::fs::write(
        change_dir.join("specs/parallel-execution/spec.md"),
        "## MODIFIED Requirements\n",
    )
    .unwrap();
    std::fs::create_dir_all(workspace.path().join("openspec")).unwrap();
    std::fs::write(
        workspace.path().join("openspec/CONSTITUTION.md"),
        "# Conflux Constitution\n",
    )
    .unwrap();
    workspace
}

/// The Conflux-owned store the builder reads artifact digests from.
///
/// Deliberately a sibling of the workspace rather than a path inside it: the
/// builder must never be handed somewhere in the target, and these tests would
/// rather fail loudly than quietly pass on a store that happens to coincide.
fn external_store(workspace: &tempfile::TempDir) -> EvidenceStore {
    EvidenceStore::new(
        workspace
            .path()
            .parent()
            .expect("a tempdir has a parent")
            .join(format!(
                "cflx-acceptance-store-{}",
                workspace
                    .path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )),
    )
}

async fn build(
    facts: &FakeFacts,
    workspace: &tempfile::TempDir,
    change_id: &str,
) -> AcceptanceExecutionManifest {
    build_manifest_for_workspace(
        facts,
        workspace.path(),
        &external_store(workspace),
        change_id,
        "cflx-accept",
        Some("main"),
        300,
    )
    .await
    .expect("a readable workspace must produce a manifest")
}

/// The manifest binds the candidate, the resolved review base and range, the
/// canonical change inputs, the skill, and the one eligible gate — and derives
/// all of it from the workspace.
#[tokio::test]
async fn acceptance_execution_boundary_manifest_binds_workspace_evidence() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();

    let manifest = build(&facts, &workspace, "alpha").await;

    assert_eq!(manifest.candidate_commit_oid, COMMIT);
    assert_eq!(manifest.candidate_tree_oid, TREE);
    assert_eq!(manifest.review_base_commit, BASE);
    assert_eq!(manifest.review_range, format!("{BASE}..{COMMIT}"));
    assert_eq!(manifest.review_base_ref.as_deref(), Some("main"));
    assert_eq!(manifest.skill.name, "cflx-accept");
    assert!(
        !manifest.skill.digest.is_empty(),
        "the embedded reviewer skill must be bound by digest"
    );
    assert_eq!(
        manifest
            .gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec!["focused-gate"]
    );
    assert_eq!(manifest.gates[0].automation_blob_oid, BLOB);
    assert_eq!(manifest.gates[0].tool.executable_digest, TOOL_DIGEST);
    assert_eq!(manifest.work_budget_secs, 270);
    // The manifest must survive its own serialization unchanged.
    let bytes = serde_json::to_vec(&manifest).unwrap();
    assert_eq!(
        crate::orchestration::acceptance::execution_manifest::parse_manifest(&bytes)
            .unwrap()
            .fingerprint(),
        manifest.fingerprint()
    );
}

/// Rebuilding from the same worktree gives the same fingerprint. This is the
/// restart property: nothing is carried over between the two calls.
#[tokio::test]
async fn acceptance_execution_boundary_rebuild_is_deterministic() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();

    let first = build(&facts, &workspace, "alpha").await;
    let second = build(&facts, &workspace, "alpha").await;

    assert_eq!(first.fingerprint(), second.fingerprint());
}

/// Editing a change input the reviewer judges against moves the fingerprint,
/// which is what makes a refused retry admissible again after a real repair.
#[tokio::test]
async fn acceptance_execution_boundary_editing_change_inputs_moves_the_fingerprint() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();
    let before = build(&facts, &workspace, "alpha").await;

    std::fs::write(
        workspace.path().join("openspec/changes/alpha/tasks.md"),
        "- [x] Do the thing\n- [x] Do the other thing\n",
    )
    .unwrap();
    let after = build(&facts, &workspace, "alpha").await;

    assert_ne!(before.fingerprint(), after.fingerprint());
}

/// The constitution outranks the proposal, so amending it genuinely changes
/// what Acceptance is judging against and must change the fingerprint.
#[tokio::test]
async fn acceptance_execution_boundary_constitution_participates_in_change_inputs() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();
    let before = build(&facts, &workspace, "alpha").await;

    std::fs::write(
        workspace.path().join("openspec/CONSTITUTION.md"),
        "# Conflux Constitution\n\n### 4. New law\n",
    )
    .unwrap();
    let after = build(&facts, &workspace, "alpha").await;

    assert_ne!(before.fingerprint(), after.fingerprint());
}

/// A base branch that moved produces a different range, and therefore a
/// different fingerprint: the review really is a different one.
#[tokio::test]
async fn acceptance_execution_boundary_moved_review_base_moves_the_fingerprint() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();
    let before = build(&facts, &workspace, "alpha").await;

    let mut moved = FakeFacts::new();
    moved.revisions.insert("main".to_string(), "9".repeat(40));
    let after = build(&moved, &workspace, "alpha").await;

    assert_ne!(before.fingerprint(), after.fingerprint());
    assert_eq!(after.review_base_commit, "9".repeat(40));
}

/// An unresolvable base leaves the range empty rather than inventing one: a
/// fabricated range would bind evidence to a comparison nobody performed.
#[tokio::test]
async fn acceptance_execution_boundary_unresolvable_base_leaves_the_range_empty() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();

    let manifest = build_manifest_for_workspace(
        &facts,
        workspace.path(),
        &external_store(&workspace),
        "alpha",
        "cflx-accept",
        Some("no-such-branch"),
        300,
    )
    .await
    .unwrap();

    assert!(manifest.review_base_commit.is_empty());
    assert!(manifest.review_range.is_empty());
    // Still a valid manifest, and still round-trips.
    let bytes = serde_json::to_vec(&manifest).unwrap();
    assert!(crate::orchestration::acceptance::execution_manifest::parse_manifest(&bytes).is_ok());
}

/// A gate whose declared executable cannot be resolved is recorded with an
/// empty tool identity, which the gate phase turns into the resumable external
/// prerequisite rather than a silent pass.
#[tokio::test]
async fn acceptance_execution_boundary_unresolvable_tool_is_recorded_not_guessed() {
    let workspace = workspace_with_change("alpha");
    let mut facts = FakeFacts::new();
    facts.tool = None;

    let manifest = build(&facts, &workspace, "alpha").await;

    assert_eq!(manifest.gates[0].tool, ToolIdentity::default());
}

/// A change with no proposal at all produces an empty allowlist rather than an
/// error: nothing declared means nothing blocking.
#[tokio::test]
async fn acceptance_execution_boundary_missing_proposal_yields_an_empty_allowlist() {
    let workspace = tempfile::tempdir().unwrap();
    let facts = FakeFacts::new();

    let manifest = build(&facts, &workspace, "alpha").await;

    assert!(!manifest.has_gates());
}

/// An unobservable repository is a runtime defect, never a verdict.
#[tokio::test]
async fn acceptance_execution_boundary_unobservable_repository_is_a_build_error() {
    let workspace = workspace_with_change("alpha");

    let error = build_manifest_for_workspace(
        &BlindFacts,
        workspace.path(),
        &external_store(&workspace),
        "alpha",
        "cflx-accept",
        Some("main"),
        300,
    )
    .await
    .expect_err("an unreadable repository must not produce a manifest");

    assert!(matches!(error, ManifestBuildError::Unobservable(_)));
    assert!(error.detail().contains("could not be proven"));
}

/// A change ID that could escape the evidence directory is refused outright.
#[tokio::test]
async fn acceptance_execution_boundary_refuses_an_unstorable_change_id() {
    let workspace = tempfile::tempdir().unwrap();
    let facts = FakeFacts::new();

    let error = build_manifest_for_workspace(
        &facts,
        workspace.path(),
        &external_store(&workspace),
        "../escape",
        "cflx-accept",
        None,
        300,
    )
    .await
    .expect_err("an unstorable change id must be refused");

    assert!(matches!(error, ManifestBuildError::UnstorableChangeId(_)));
}

/// The review base is resolved exactly once per build, from the name the caller
/// supplied — not re-derived later from whatever `HEAD` has become.
#[tokio::test]
async fn acceptance_execution_boundary_resolves_the_review_base_once() {
    let workspace = workspace_with_change("alpha");
    let facts = FakeFacts::new();

    let _ = build(&facts, &workspace, "alpha").await;

    assert_eq!(
        facts.resolve_calls.lock().unwrap().as_slice(),
        ["main".to_string()]
    );
}

/// Sanity: the path resolver used by the shared admission port never leaves the
/// worktree base directory, and a change with no worktree simply has no path.
#[test]
fn acceptance_execution_boundary_workspace_resolution_stays_inside_the_base() {
    let base = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(base.path().join("alpha")).unwrap();

    let resolve = |change_id: &str| -> Option<PathBuf> {
        let path = base.path().join(change_id.replace(['/', '\\', ' '], "-"));
        path.is_dir().then_some(path)
    };

    assert_eq!(resolve("alpha"), Some(base.path().join("alpha")));
    assert_eq!(resolve("missing"), None);
}

// ── Candidate proposal refresh: invalidation by content identity ───────────

/// After a candidate proposal refresh, nothing new has to happen for stale
/// Acceptance state to stop applying: the manifest's change inputs read the
/// candidate `proposal.md`, evidence binds the candidate commit, and a live hold
/// is keyed by the fingerprint those produce. Real Git, real refresh commit.
#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_invalidates_stale_acceptance_state_by_content_identity() {
    use crate::orchestration::acceptance::evidence_location::scoped_acceptance_root_for_test;
    use crate::orchestration::acceptance::execution_manifest::{
        AcceptanceAdmission, AcceptanceHoldCategory, LiveAcceptanceHold, LiveAcceptanceHolds,
        WorkspaceAcceptanceAdmission,
    };
    use crate::orchestration::acceptance::verification_evidence::{
        eligible_request, evaluate_reuse, CurrentBindings, GitRepositoryFacts, ReuseDecision,
        ReusePolicy, ReviewBinding, VerificationEvidence, EVIDENCE_AUTHORITY, EVIDENCE_SCHEMA,
    };
    use crate::orchestration::operator_command::AcceptanceAdmissionPort;
    use crate::orchestration::proposal_refresh::tests::{Fixture, CHANGE, CORRECTED_PROPOSAL};
    use crate::orchestration::proposal_refresh::{apply, RefreshOutcome};

    let fx = Fixture::new();
    let store_dir = tempfile::tempdir().unwrap();
    let store = EvidenceStore::new(store_dir.path().join("store"));
    let manifest = |label: &'static str| {
        let worktree = fx.worktree.clone();
        let store = &store;
        async move {
            build_manifest_for_workspace(
                &GitRepositoryFacts,
                &worktree,
                store,
                CHANGE,
                "cflx-accept",
                Some("develop"),
                300,
            )
            .await
            .unwrap_or_else(|error| panic!("{label} manifest: {error:?}"))
        }
    };

    let before = manifest("pre-refresh").await;

    // A live hold for the pre-refresh input refuses an unchanged retry.
    let holds = Arc::new(LiveAcceptanceHolds::new());
    holds.record(
        CHANGE,
        LiveAcceptanceHold {
            category: AcceptanceHoldCategory::ReviewDeadlineExhausted,
            fingerprint: before.fingerprint(),
            review_base_ref: Some("develop".to_string()),
        },
    );
    let resolve = {
        let worktree = fx.worktree.clone();
        move |_: &str| Some(worktree.clone())
    };
    let admission =
        WorkspaceAcceptanceAdmission::with_holds(resolve, "cflx-accept", None, holds.clone());
    let acceptance_root = store_dir.path().join("acceptance-root");
    assert!(
        matches!(
            scoped_acceptance_root_for_test(acceptance_root.clone(), admission.classify(CHANGE))
                .await,
            AcceptanceAdmission::Refuse { .. }
        ),
        "the unchanged pre-refresh input must be refused"
    );

    // A gate evidence record bound to the pre-refresh candidate.
    let declaration = crate::openspec::VerificationDeclaration {
        id: Some("refresh-gate".to_string()),
        requirement: Some("gate".to_string()),
        phase: Some("pre-integration".to_string()),
        owner: Some("conflux-acceptance".to_string()),
        trigger: Some("pull-request-validation".to_string()),
        automation: Some("src/impl.rs".to_string()),
        evidence: Some("cargo test --lib".to_string()),
        rerun: Some("cargo test --lib".to_string()),
        prerequisites: Some(Vec::new()),
        execution_class: Some("repository-local".to_string()),
        completion_role: Some("change-blocking".to_string()),
    };
    let request = eligible_request(&declaration).expect("the declaration is eligible");
    let tool = crate::orchestration::acceptance::verification_evidence::ToolIdentity {
        path: "/usr/bin/cargo".to_string(),
        executable_digest: "d".repeat(64),
        version: Some("cargo 1.80.0".to_string()),
    };
    let started_at = chrono::Utc::now() - chrono::Duration::seconds(600);
    let record = VerificationEvidence {
        schema: EVIDENCE_SCHEMA.to_string(),
        authority: EVIDENCE_AUTHORITY.to_string(),
        verification_id: "refresh-gate".to_string(),
        commit_oid: before.candidate_commit_oid.clone(),
        tree_oid: before.candidate_tree_oid.clone(),
        review_base_commit: before.review_base_commit.clone(),
        review_range: before.review_range.clone(),
        argv: request.argv.clone(),
        env_names: request.env.names(),
        env_binding_digest: request.env.digest(),
        cwd: request.cwd.clone(),
        automation_path: request.automation_path.clone(),
        automation_blob_oid: "b".repeat(40),
        tool: tool.clone(),
        started_at,
        ended_at: started_at + chrono::Duration::seconds(300),
        exit_code: 0,
        artifact_path: EvidenceStore::artifact_relative_path("refresh-gate"),
        artifact_digest: "a".repeat(64),
        clean_before: true,
        clean_after: true,
    };
    let bindings = |manifest: &AcceptanceExecutionManifest| CurrentBindings {
        commit_oid: manifest.candidate_commit_oid.clone(),
        tree_oid: manifest.candidate_tree_oid.clone(),
        review: Some(ReviewBinding {
            base_commit: record.review_base_commit.clone(),
            range: record.review_range.clone(),
        }),
        automation_blob_oid: "b".repeat(40),
        tool: tool.clone(),
        clean: true,
        artifact_digest: Some("a".repeat(64)),
    };
    assert!(
        evaluate_reuse(
            &request,
            &record,
            &bindings(&before),
            ReusePolicy::default()
        )
        .is_reuse(),
        "the record is reusable against the candidate it was bound to"
    );

    // The refresh commit.
    let report = apply(&fx.root, &fx.worktree, &fx.request()).await;
    assert_eq!(
        report.outcome,
        Some(RefreshOutcome::Refreshed),
        "{report:?}"
    );

    let after = manifest("post-refresh").await;
    assert_ne!(after.change_input_digest, before.change_input_digest);
    assert_ne!(after.fingerprint(), before.fingerprint());
    assert_ne!(after.candidate_commit_oid, before.candidate_commit_oid);

    match evaluate_reuse(&request, &record, &bindings(&after), ReusePolicy::default()) {
        ReuseDecision::Rerun { reason, .. } => {
            assert_eq!(reason.code(), "binding_mismatch");
            assert!(
                reason.detail().starts_with("commit_oid changed"),
                "{}",
                reason.detail()
            );
        }
        other => panic!("pre-refresh evidence must rerun, got {other:?}"),
    }

    // The same live registry no longer refuses: the hold names a fingerprint the
    // candidate no longer produces.
    assert_eq!(
        scoped_acceptance_root_for_test(acceptance_root, admission.classify(CHANGE)).await,
        AcceptanceAdmission::Admit
    );

    // What the next Apply/Acceptance prompt points at is the corrected file.
    let prompt = crate::agent::prompt::build_apply_prompt_with_skill(
        "cflx-apply",
        Some(&fx.worktree),
        CHANGE,
        "",
        "",
        "",
        "",
    );
    let proposal_line = prompt
        .lines()
        .find_map(|line| line.strip_prefix("proposal_path: "))
        .expect("the prompt names the proposal path");
    assert_eq!(
        std::fs::read_to_string(fx.worktree.join(proposal_line)).unwrap(),
        CORRECTED_PROPOSAL
    );
}

// ============================================================================
// Leading environment assignments (`rerun_leading_env`)
// ============================================================================

const ENV_SENTINEL: &str = "rerun-leading-env-sentinel-5b81d2";

fn workspace_with_command(change_id: &str, command: &str) -> tempfile::TempDir {
    let workspace = workspace_with_change(change_id);
    let proposal = PROPOSAL.replace("cargo test focused --lib", command);
    std::fs::write(
        workspace
            .path()
            .join("openspec/changes")
            .join(change_id)
            .join("proposal.md"),
        proposal,
    )
    .unwrap();
    workspace
}

#[tokio::test]
async fn rerun_leading_env_manifest_resolves_the_executable_and_persists_no_value() {
    let command = format!("DJANGO_SETTINGS_MODULE={ENV_SENTINEL} cargo test focused --lib");
    let workspace = workspace_with_command("alpha", &format!("'{command}'"));
    let facts = FakeFacts::new();
    let manifest = build(&facts, &workspace, "alpha").await;

    assert_eq!(
        facts.tool_programs.lock().unwrap().as_slice(),
        ["cargo".to_string()],
        "the tool is resolved from the executable, never from an assignment"
    );
    let gate = &manifest.gates[0];
    assert_eq!(gate.argv, vec!["cargo", "test", "focused", "--lib"]);
    assert_eq!(gate.env_names, vec!["DJANGO_SETTINGS_MODULE"]);
    assert_eq!(
        gate.env_binding_digest,
        crate::orchestration::acceptance::verification_evidence::EnvBindings::from_assignments([(
            "DJANGO_SETTINGS_MODULE",
            ENV_SENTINEL
        )])
        .digest()
    );
    // The values stay in memory for the runtime that starts the child.
    assert_eq!(
        gate.request().env.pairs(),
        [(
            "DJANGO_SETTINGS_MODULE".to_string(),
            ENV_SENTINEL.to_string()
        )]
    );

    // Persisted and reviewer-facing projections carry names and digest only.
    let stored = serde_json::to_string(&manifest).unwrap();
    assert!(stored.contains("DJANGO_SETTINGS_MODULE"));
    assert!(!stored.contains(ENV_SENTINEL), "{stored}");
    let review = manifest.to_review_json();
    assert!(!review.to_string().contains(ENV_SENTINEL), "{review}");
    assert_eq!(
        review["declared_blocking_verifications"][0]["command"],
        "DJANGO_SETTINGS_MODULE=<redacted> cargo test focused --lib"
    );
    assert_eq!(
        review["declared_blocking_verifications"][0]["env_names"],
        serde_json::json!(["DJANGO_SETTINGS_MODULE"])
    );
    assert!(!format!("{manifest:?}").contains(ENV_SENTINEL));
}

#[tokio::test]
async fn rerun_leading_env_invalid_assignment_freezes_a_declaration_defect() {
    let workspace = workspace_with_command("alpha", &format!("'1BAD={ENV_SENTINEL} cargo test'"));
    let facts = FakeFacts::new();
    let manifest = build(&facts, &workspace, "alpha").await;

    assert!(manifest.gates.is_empty());
    assert_eq!(manifest.declaration_defects.len(), 1);
    let defect = &manifest.declaration_defects[0];
    assert_eq!(defect.code, "declaration_invalid_assignment");
    assert_eq!(defect.field, "rerun");
    assert_eq!(defect.verification_id, "focused-gate");
    assert!(
        facts.tool_programs.lock().unwrap().is_empty(),
        "a defective declaration is never resolved against PATH"
    );
    assert!(!serde_json::to_string(&manifest)
        .unwrap()
        .contains(ENV_SENTINEL));
}
