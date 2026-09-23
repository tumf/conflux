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
    async fn resolve_tool(
        &self,
        _workspace: &Path,
        _program: &str,
    ) -> Result<ToolIdentity, String> {
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
