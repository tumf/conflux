//! End-to-end regressions for the runtime-owned Acceptance execution boundary.
//!
//! These drive `execute_acceptance_in_workspace` against a real Git worktree and
//! two deliberately dumb reviewers:
//!
//! * a **canonical reviewer** that emits a verdict and asks for nothing else,
//!   used to prove that the runtime — not the reviewer — ran the declared gates;
//! * a **non-terminating reviewer** that never exits, used to prove the shared
//!   review deadline terminates it, reaps it, and settles as a typed
//!   non-resumable `stalled` hold rather than the generic runtime-limit `error`.
//!
//! Every reviewer writes a marker file before doing anything else, so "the
//! reviewer never started" is an observable fact rather than an inference. That
//! matters: most of the claims here are about work that must *not* happen.
//!
//! Two properties this file exists to defend, beyond the boundary itself:
//!
//! * **Nothing Conflux owns is written into the target.** Every case that runs a
//!   gate also asserts the target tree is exactly what Git says it should be,
//!   and the evidence is looked for in the external store instead.
//! * **Retry refusal is process-local.** A live owner refuses an unchanged
//!   retry; a restarted one admits a fresh bounded attempt from the same
//!   untouched worktree, because the refusal was never written down anywhere a
//!   restart could read it.
//!
//! Each case uses its own change ID. The live-hold registry is process-wide by
//! design — the executor and the shared operator service must see one fact — so
//! two concurrent cases sharing a change ID would share a refusal.
//!
//! The review budget is scoped to milliseconds for the deadline tests. The
//! production partition is whole seconds and its smallest positive work budget
//! is one second, which is a fine bound for an operator and a poor one for a
//! suite with a one-second total runtime target.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::Mutex;

use super::support::{create_test_config_with, init_git_repo, TestAssertionExt};
use crate::agent::AgentRunner;
use crate::ai_command_runner::AiCommandRunner;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::{acceptance_store_path_in, default_retry_patterns};
use crate::config::OrchestratorConfig;
use crate::orchestration::acceptance::evidence_location::scoped_acceptance_root_for_test;
use crate::orchestration::acceptance::execution_manifest::{
    AcceptanceAdmission, AcceptanceHoldCategory, LiveAcceptanceHold, LiveAcceptanceHolds,
    ManifestStore, WorkspaceAcceptanceAdmission,
};
use crate::orchestration::acceptance::manifest_builder::repository_project_root;
use crate::orchestration::acceptance::verification_evidence::{EvidenceStore, LEGACY_EVIDENCE_DIR};
use crate::orchestration::AcceptanceResult;
use crate::parallel::executor::{
    execute_acceptance_in_workspace, scoped_review_budget_millis_for_test,
};

const REVIEWER_MARKER: &str = "reviewer-started.txt";
const GATE: &str = "focused-gate";

/// A proposal whose only eligible declaration is one repository-local
/// change-blocking gate running `command`.
fn proposal_with_gate(change: &str, command: &str) -> String {
    format!(
        "---\n\
verifications:\n\
\x20 - id: {GATE}\n\
\x20   requirement: the focused gate passes\n\
\x20   phase: pre-integration\n\
\x20   owner: conflux\n\
\x20   trigger: pull-request-validation\n\
\x20   automation: openspec/changes/{change}/proposal.md\n\
\x20   evidence: {command}\n\
\x20   rerun: {command}\n\
\x20   prerequisites: []\n\
\x20   execution_class: repository-local\n\
\x20   completion_role: change-blocking\n\
---\n\
# Change\n"
    )
}

/// A proposal that declares nothing eligible: an empty blocking allowlist.
const PROPOSAL_WITHOUT_GATES: &str = "---\nchange_type: implementation\n---\n# Change\n";

/// One case's target repository plus the Conflux-owned external root its
/// Acceptance evidence must land under.
///
/// The two are separate temporary directories on purpose. "The store is not in
/// the target" is the whole contract, and a fixture that could satisfy it by
/// accident would not be testing anything.
struct Fixture {
    repo: TempDir,
    external: TempDir,
    change: String,
}

impl Fixture {
    async fn new(change: &str, proposal: &str) -> Self {
        let repo = TempDir::new().or_fail("unexpected error");
        init_git_repo(repo.path()).await;
        let change_dir = repo.path().join("openspec/changes").join(change);
        std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
        std::fs::write(change_dir.join("proposal.md"), proposal).or_fail("unexpected error");
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n\n- [x] 1. done\n",
        )
        .or_fail("unexpected error");
        commit_all(repo.path(), &format!("Apply: {change}")).await;
        Self {
            repo,
            external: TempDir::new().or_fail("unexpected error"),
            change: change.to_string(),
        }
    }

    fn path(&self) -> &Path {
        self.repo.path()
    }

    fn change(&self) -> &str {
        &self.change
    }

    /// The external directory this change's evidence must resolve to, computed
    /// exactly the way production computes it.
    async fn store_root(&self) -> PathBuf {
        self.store_root_under(self.external.path()).await
    }

    async fn store_root_under(&self, external: &Path) -> PathBuf {
        let project = repository_project_root(self.path())
            .await
            .unwrap_or_else(|| self.path().to_path_buf());
        acceptance_store_path_in(external, &project, self.path(), &self.change)
            .or_fail("the external store path must resolve")
    }

    async fn evidence(&self) -> EvidenceStore {
        EvidenceStore::new(self.store_root().await)
    }

    async fn manifests(&self) -> ManifestStore {
        ManifestStore::new(self.store_root().await)
    }

    fn reviewer_started(&self) -> bool {
        self.path().join(REVIEWER_MARKER).exists()
    }

    fn clear_reviewer_marker(&self) {
        let _ = std::fs::remove_file(self.path().join(REVIEWER_MARKER));
    }

    /// Run one bounded Acceptance invocation with the external root scoped to
    /// this fixture.
    async fn accept(
        &self,
        config: &OrchestratorConfig,
        ai_runner: &AiCommandRunner,
    ) -> (AcceptanceResult, u32) {
        self.accept_under(self.external.path().to_path_buf(), config, ai_runner)
            .await
    }

    async fn accept_under(
        &self,
        external: PathBuf,
        config: &OrchestratorConfig,
        ai_runner: &AiCommandRunner,
    ) -> (AcceptanceResult, u32) {
        scoped_acceptance_root_for_test(
            external,
            run_acceptance(&self.change, self.path(), config, ai_runner),
        )
        .await
    }

    /// The set of paths Git reports as staged, unstaged, or untracked.
    ///
    /// Read with no Conflux exclusion whatsoever, which is the point: if
    /// anything this runtime owns landed in the target, it shows up here.
    async fn porcelain_status(&self) -> String {
        let output = Command::new("git")
            .args(["status", "--porcelain=v1", "--untracked-files=all"])
            .current_dir(self.path())
            .output()
            .await
            .or_fail("unexpected error");
        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

async fn commit_all(repo_root: &std::path::Path, message: &str) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
}

/// A reviewer that records that it started, emits a canonical verdict, and asks
/// for nothing else. It runs no verification of its own, which is the point:
/// whatever gate evidence exists came from the runtime.
fn canonical_reviewer() -> String {
    format!("sh -c 'touch {REVIEWER_MARKER}; printf \"ACCEPTANCE: PASS\\n\"'")
}

/// A reviewer that records that it started and then never terminates.
fn non_terminating_reviewer() -> String {
    format!("sh -c 'touch {REVIEWER_MARKER}; while true; do sleep 1; done'")
}

fn config_with_reviewer(command: String) -> OrchestratorConfig {
    create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(command),
        ..Default::default()
    })
}

fn runner(acceptance_max_runtime_secs: u64) -> AiCommandRunner {
    AiCommandRunner::new(
        CommandQueueConfig {
            acceptance_max_runtime_secs,
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: default_retry_patterns(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        },
        Arc::new(Mutex::new(None)),
    )
}

async fn run_acceptance(
    change: &str,
    repo_root: &std::path::Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
) -> (AcceptanceResult, u32) {
    let mut agent = AgentRunner::new(config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));
    execute_acceptance_in_workspace(
        change,
        repo_root,
        &mut agent,
        None,
        None,
        ai_runner,
        config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
        crate::orchestration::acceptance::AcceptanceCommandMode::Normal,
    )
    .await
    .or_fail("acceptance must return a typed result")
}

fn hold_of(
    result: AcceptanceResult,
) -> crate::orchestration::acceptance::execution_manifest::AcceptanceExecutionHold {
    match result {
        AcceptanceResult::ExecutionHold { hold } => hold,
        other => panic!("expected a typed execution hold, got {other:?}"),
    }
}

// ============================================================================
// The runtime owns declared gate execution — and writes nothing into the target
// ============================================================================

/// The declared gate ran under runtime supervision, its bound evidence exists in
/// the Conflux-owned external store, the reviewer — which ran no verification at
/// all — still produced a verdict, and the target worktree is byte-for-byte what
/// the Apply commit left behind.
///
/// That last clause is constitutional law 4 as a regression: no envelope, no
/// artifact, no manifest, no ignore file, no `.cflx` directory.
#[tokio::test]
async fn acceptance_execution_boundary_runtime_executes_declared_gates_outside_the_target() {
    let fixture = Fixture::new(
        "gates-outside-target",
        &proposal_with_gate("gates-outside-target", "sh -c true"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a passing declared gate plus a canonical verdict is a PASS, got {result:?}"
    );
    assert!(
        fixture.reviewer_started(),
        "the semantic reviewer must still run"
    );

    // The evidence exists — externally.
    let store_root = fixture.store_root().await;
    assert!(
        store_root.starts_with(fixture.external.path()),
        "the store must resolve under the configured external root: {store_root:?}"
    );
    assert!(
        !store_root.starts_with(fixture.path()),
        "the store must never resolve inside the target: {store_root:?}"
    );
    assert!(
        store_root.join(format!("gates/{GATE}.json")).is_file(),
        "the runtime must write bound evidence for the gate it supervised"
    );
    assert!(store_root.join(format!("gates/{GATE}.log")).is_file());

    let manifest = fixture
        .manifests()
        .await
        .load()
        .expect("the manifest must be persisted to the external store");
    assert_eq!(
        manifest
            .gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec![GATE],
        "the manifest is the complete blocking allowlist"
    );
    assert_eq!(manifest.candidate_commit_oid.len(), 40);
    assert!(manifest.review_range.contains(".."));
    assert_eq!(
        manifest.gates[0].artifact_path,
        EvidenceStore::artifact_relative_path(GATE),
        "gate artifacts are bound store-relative, never target-relative"
    );

    // And nothing landed in the target. The reviewer's own marker is the single
    // expected entry; it is the *test's* artifact, not Conflux's.
    let status = fixture.porcelain_status().await;
    let unexpected: Vec<&str> = status
        .lines()
        .filter(|line| !line.ends_with(REVIEWER_MARKER))
        .collect();
    assert!(
        unexpected.is_empty(),
        "Acceptance must create no file or directory in the target: {unexpected:?}"
    );
    assert!(
        !fixture.path().join(".cflx").exists(),
        "no target-local Conflux directory may be created"
    );
    assert!(
        !fixture.path().join(LEGACY_EVIDENCE_DIR).exists(),
        "the legacy evidence path must never be recreated"
    );
}

/// A failing declared gate decides the verdict by itself: the reviewer is never
/// started, so no budget is spent rediscovering what the runtime already knows.
#[tokio::test]
async fn acceptance_execution_boundary_failing_gate_fails_without_starting_a_reviewer() {
    let fixture = Fixture::new(
        "failing-gate",
        &proposal_with_gate("failing-gate", "sh -c false"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, iteration) = fixture.accept(&config, &runner(3600)).await;

    match result {
        AcceptanceResult::Fail { findings } => {
            assert!(
                findings.iter().any(|finding| finding.text().contains(GATE)),
                "the failing gate must be the finding's evidence: {findings:?}"
            );
        }
        other => panic!("a failing declared gate must FAIL the change, got {other:?}"),
    }
    assert!(iteration > 0, "a gate-decided FAIL records a real attempt");
    assert!(
        !fixture.reviewer_started(),
        "the reviewer must not be started once the verdict is already determined"
    );
    assert!(
        fixture.porcelain_status().await.is_empty(),
        "even a failing gate must leave the target untouched"
    );
}

/// A change that declares nothing eligible still reaches a verdict. The empty
/// allowlist is a boundary, not a reason to hold.
#[tokio::test]
async fn acceptance_execution_boundary_empty_allowlist_still_reaches_a_verdict() {
    let fixture = Fixture::new("empty-allowlist", PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
    assert!(fixture.reviewer_started());
    let manifest = fixture
        .manifests()
        .await
        .load()
        .expect("even a gateless change gets a manifest");
    assert!(!manifest.has_gates());
}

/// A workspace that is not a Git working tree has no candidate to freeze and no
/// evidence to bind, so it gets no manifest — not a hold. Acceptance runs there
/// exactly as it did before the boundary existed.
///
/// The distinction matters: collapsing "never a repository" into "repository
/// unreadable" would turn every non-repository Acceptance into a permanent hold
/// nobody can clear.
#[tokio::test]
async fn acceptance_execution_boundary_non_repository_workspace_gets_no_manifest() {
    let workspace = TempDir::new().or_fail("unexpected error");
    let external = TempDir::new().or_fail("unexpected error");
    let change = "non-repository";
    let change_dir = workspace.path().join("openspec/changes").join(change);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("proposal.md"),
        proposal_with_gate(change, "sh -c true"),
    )
    .or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = scoped_acceptance_root_for_test(
        external.path().to_path_buf(),
        run_acceptance(change, workspace.path(), &config, &runner(3600)),
    )
    .await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a non-repository workspace must keep its pre-boundary behaviour, got {result:?}"
    );
    assert!(workspace.path().join(REVIEWER_MARKER).exists());
    assert_eq!(
        std::fs::read_dir(external.path())
            .or_fail("unexpected error")
            .filter_map(Result::ok)
            .count(),
        0,
        "no store may be created for a workspace with no candidate revision"
    );
}

// ============================================================================
// The external store fails closed, and never into the target
// ============================================================================

/// An Acceptance state path that cannot be created is a typed runtime hold
/// emitted *before* the gate launches — and it does not fall back to `.cflx` or
/// any other target path.
#[tokio::test]
async fn acceptance_execution_boundary_unusable_state_path_holds_before_the_gate() {
    let fixture = Fixture::new(
        "unusable-state-path",
        &proposal_with_gate("unusable-state-path", "sh -c true"),
    )
    .await;
    let blocker = TempDir::new().or_fail("unexpected error");
    // A regular file where the root directory must be: `create_dir_all` cannot
    // succeed here, and no retry will change that.
    let occupied = blocker.path().join("occupied");
    std::fs::write(&occupied, b"not a directory").or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    let (result, iteration) = fixture
        .accept_under(occupied.clone(), &config, &runner(3600))
        .await;

    let hold = hold_of(result);
    assert_eq!(hold.category, AcceptanceHoldCategory::StatePathUnavailable);
    assert!(
        hold.evidence
            .iter()
            .any(|line| line.contains("acceptance_store_")),
        "the hold must carry the resolver's own typed code: {:?}",
        hold.evidence
    );
    assert_eq!(iteration, 0, "a bounded hold consumes no attempt number");
    assert!(
        !fixture.reviewer_started(),
        "no reviewer may start once the store is known to be unusable"
    );
    assert!(
        fixture.porcelain_status().await.is_empty(),
        "a state-path failure must never fall back into the target"
    );
    assert!(!fixture.path().join(".cflx").exists());

    // Recoverable, not permanent: the environment fact is not a statement about
    // the input, so it must not refuse the next attempt.
    assert!(
        !hold.category.refuses_unchanged_retry(),
        "an unusable state path must not harden into `unchanged_acceptance_input`"
    );
}

/// A resolved store inside the target repository is refused before anything is
/// created, with the same typed hold. This is the containment law end to end.
#[tokio::test]
async fn acceptance_execution_boundary_store_inside_the_target_is_refused() {
    let fixture = Fixture::new(
        "store-inside-target",
        &proposal_with_gate("store-inside-target", "sh -c true"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture
        .accept_under(
            fixture.path().join(".cflx/acceptance"),
            &config,
            &runner(3600),
        )
        .await;

    let hold = hold_of(result);
    assert_eq!(hold.category, AcceptanceHoldCategory::StatePathUnavailable);
    assert!(
        hold.evidence
            .iter()
            .any(|line| line.contains("acceptance_store_inside_target")),
        "the refusal must name containment specifically: {:?}",
        hold.evidence
    );
    assert!(
        !fixture.path().join(".cflx").exists(),
        "containment must be refused before the directory is created"
    );
    assert!(!fixture.reviewer_started());
    assert!(fixture.porcelain_status().await.is_empty());
}

// ============================================================================
// Cleanliness and the legacy path
// ============================================================================

/// Legacy target-local evidence stops Acceptance with its own typed hold, and is
/// detected even though the old runtime self-ignored it. Nothing below that path
/// is opened, parsed, migrated, rewritten, or deleted: cleaning it up is an
/// explicit repository-owner action.
#[tokio::test]
async fn acceptance_execution_boundary_legacy_target_evidence_holds_without_touching_it() {
    let fixture = Fixture::new(
        "legacy-evidence",
        &proposal_with_gate("legacy-evidence", "sh -c true"),
    )
    .await;
    let legacy = fixture.path().join(LEGACY_EVIDENCE_DIR);
    std::fs::create_dir_all(&legacy).or_fail("unexpected error");
    // Exactly what the previous release left behind: a self-ignoring marker that
    // makes the whole subtree invisible to `git status`, plus a record.
    std::fs::write(legacy.join(".gitignore"), "*\n").or_fail("unexpected error");
    let record = legacy.join(format!("{GATE}.json"));
    std::fs::write(
        &record,
        b"{\"schema\":\"conflux-verification-evidence-v1\"}",
    )
    .or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    // The self-ignore really does hide it from the ordinary status question,
    // which is why a dedicated existence probe is needed rather than a wider
    // cleanliness rule.
    assert!(
        !fixture.porcelain_status().await.contains(".cflx"),
        "the legacy directory is invisible to tracked/untracked porcelain status"
    );

    let (result, iteration) = fixture.accept(&config, &runner(3600)).await;

    let hold = hold_of(result);
    assert_eq!(hold.category, AcceptanceHoldCategory::LegacyTargetEvidence);
    assert!(
        hold.evidence
            .iter()
            .any(|line| line.contains(LEGACY_EVIDENCE_DIR)),
        "the hold must name the path a repository owner has to remove: {:?}",
        hold.evidence
    );
    assert!(
        hold.summary(fixture.change()).contains("Remove the legacy"),
        "the operator-facing summary must state whose job the cleanup is: {}",
        hold.summary(fixture.change())
    );
    assert_eq!(iteration, 0);
    assert!(!fixture.reviewer_started(), "no reviewer may start");
    assert_eq!(
        std::fs::read(&record).or_fail("unexpected error"),
        b"{\"schema\":\"conflux-verification-evidence-v1\"}",
        "legacy content must be left exactly as it was found"
    );
    assert!(
        legacy.join(".gitignore").is_file(),
        "the legacy ignore file must not be removed either"
    );
}

/// Only that one path is probed. An unrelated ignored directory — a build
/// output tree, a dependency cache — is none of the runtime's business, and
/// evaluating it would reintroduce exactly the Conflux-specific status filtering
/// constitutional law 4 forbids.
#[tokio::test]
async fn acceptance_execution_boundary_unrelated_ignored_paths_are_not_evaluated() {
    let fixture = Fixture::new(
        "unrelated-ignored",
        &proposal_with_gate("unrelated-ignored", "sh -c true"),
    )
    .await;
    std::fs::write(fixture.path().join(".gitignore"), "build-output\n").or_fail("unexpected error");
    commit_all(fixture.path(), "chore: ignore build output").await;
    std::fs::create_dir_all(fixture.path().join("build-output")).or_fail("unexpected error");
    std::fs::write(fixture.path().join("build-output/artifact.bin"), b"junk")
        .or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "an unrelated ignored path must not hold anything, got {result:?}"
    );
    assert!(fixture.path().join("build-output/artifact.bin").is_file());
}

/// Cleanliness uses the ordinary tracked/untracked porcelain status with nothing
/// excluded, so an untracked target file refuses the capture rather than being
/// waved through. The old evidence-directory exception is gone, not widened.
#[tokio::test]
async fn acceptance_execution_boundary_untracked_target_content_refuses_the_gate() {
    let fixture = Fixture::new(
        "dirty-target",
        &proposal_with_gate("dirty-target", "sh -c true"),
    )
    .await;
    // A Conflux-shaped path, deliberately: under the previous rule anything
    // below `.cflx/` was invisible to the comparison.
    std::fs::create_dir_all(fixture.path().join(".cflx")).or_fail("unexpected error");
    std::fs::write(fixture.path().join(".cflx/scratch.json"), b"{}").or_fail("unexpected error");
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    let hold = hold_of(result);
    assert_eq!(
        hold.category,
        AcceptanceHoldCategory::RuntimeDefect,
        "a dirty target is an environment fact, never a verdict about the change"
    );
    assert!(
        hold.evidence
            .iter()
            .any(|line| line.contains("worktree_dirty")),
        "the hold must say the target was dirty: {:?}",
        hold.evidence
    );
    assert!(
        !hold.category.refuses_unchanged_retry(),
        "cleaning the worktree changes no fingerprint component, so this must stay retryable"
    );
}

// ============================================================================
// External records are cache: losing them costs work, never correctness
// ============================================================================

/// Deleting the external cache reruns the declared gate. It never implies PASS,
/// and it never changes the workflow action.
#[tokio::test]
async fn acceptance_execution_boundary_cache_loss_reruns_the_gate_without_proving_pass() {
    use crate::orchestration::acceptance::gate_execution::plan_manifest_reuse;
    use crate::orchestration::acceptance::verification_evidence::{
        GitRepositoryFacts, ReuseDecision, ReusePolicy,
    };

    let fixture = Fixture::new(
        "cache-loss",
        &proposal_with_gate("cache-loss", "sh -c true"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (first, _) = fixture.accept(&config, &runner(3600)).await;
    assert!(matches!(first, AcceptanceResult::Pass), "{first:?}");
    let store_root = fixture.store_root().await;
    let envelope = store_root.join(format!("gates/{GATE}.json"));
    assert!(envelope.is_file());
    let manifest = fixture.manifests().await.load().expect("manifest persists");

    // An operator deletes `~/.local/state/cflx/**`.
    std::fs::remove_dir_all(&store_root).or_fail("unexpected error");

    // The plan for the unchanged repository input says "rerun", with the reason
    // naming the absent record — not "reuse", and certainly not "pass".
    let decisions = plan_manifest_reuse(
        &GitRepositoryFacts,
        fixture.path(),
        &EvidenceStore::new(store_root.clone()),
        &manifest,
        ReusePolicy::default(),
    )
    .await;
    match decisions.as_slice() {
        [ReuseDecision::Rerun { reason, .. }] => assert_eq!(
            reason.code(),
            "evidence_missing",
            "a deleted cache is a rerun reason: {}",
            reason.detail()
        ),
        other => panic!("cache loss must force a rerun, got {other:?}"),
    }

    // And the next real invocation executes the gate again, rewriting the
    // record it could no longer prove.
    fixture.clear_reviewer_marker();
    let (second, _) = fixture.accept(&config, &runner(3600)).await;
    assert!(matches!(second, AcceptanceResult::Pass), "{second:?}");
    assert!(
        envelope.is_file(),
        "the rerun must write fresh bound evidence"
    );
    assert!(
        fixture.reviewer_started(),
        "cache loss costs work; it does not suppress the review"
    );
}

/// The invariant the refusal rests on: the fingerprint an invocation *settles*
/// with is the one the next attempt recomputes from the unchanged worktree.
///
/// An executed gate writes its artifact, and the artifact digest is a
/// fingerprint component — so a manifest frozen *before* the gates ran carries a
/// fingerprint that is already stale by the time anything terminal could be
/// recorded against it. Recording that value means the next attempt computes a
/// different one, the guard finds no match, and the review the deadline already
/// stopped is dispatched once more.
#[tokio::test]
async fn acceptance_execution_boundary_settled_fingerprint_survives_gate_execution() {
    use crate::orchestration::acceptance::manifest_builder::build_manifest_for_workspace;
    use crate::orchestration::acceptance::verification_evidence::GitRepositoryFacts;

    let fixture = Fixture::new(
        "settled-fingerprint",
        &proposal_with_gate("settled-fingerprint", "sh -c true"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let external = fixture.external.path().to_path_buf();
    let change = fixture.change().to_string();
    let boundary = scoped_acceptance_root_for_test(external, async {
        crate::parallel::executor::run_acceptance_boundary(
            &change,
            fixture.path(),
            config.get_accept_skill(),
            Some("main"),
            3600,
            None,
        )
        .await
    })
    .await
    .expect("a readable repository must produce a boundary")
    .expect("a Git working tree must produce a manifest");

    assert!(
        boundary.gates.outcomes.iter().any(|outcome| matches!(
            outcome,
            crate::orchestration::acceptance::execution_manifest::GateOutcome::Executed { .. }
        )),
        "the declared gate must really have executed: {:?}",
        boundary.gates.outcomes
    );

    // What a fresh attempt on the untouched worktree would compute.
    let next_attempt = build_manifest_for_workspace(
        &GitRepositoryFacts,
        fixture.path(),
        &fixture.evidence().await,
        fixture.change(),
        config.get_accept_skill(),
        Some("main"),
        3600,
    )
    .await
    .expect("the manifest must be derivable from the workspace");

    assert_eq!(
        boundary.manifest.fingerprint(),
        next_attempt.fingerprint(),
        "the settled fingerprint must equal what the next attempt recomputes for the same \
         commit, tree, inputs, declarations, and tool identity"
    );
    assert!(
        boundary
            .manifest
            .gates
            .iter()
            .all(|gate| gate.artifact_digest.is_some()),
        "the settled manifest must bind the artifacts the gates actually wrote"
    );
}

// ============================================================================
// The shared review deadline
// ============================================================================

/// The central regression: a reviewer that never terminates is terminated by
/// the runtime at the review deadline, its process group is reaped, and the
/// invocation settles as a non-resumable `stalled` execution hold with a stable
/// category — not as the generic runtime-limit `error`, and not into
/// missing-verdict retry.
#[tokio::test]
async fn acceptance_execution_boundary_non_terminating_reviewer_settles_as_stalled() {
    let fixture = Fixture::new("verdictless-reviewer", PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(non_terminating_reviewer());
    // A generous absolute limit, so what fires is the review deadline rather
    // than the outer runtime limit this change exists to stop reaching.
    let ai_runner = runner(3600);

    let (result, iteration) =
        scoped_review_budget_millis_for_test(150, fixture.accept(&config, &ai_runner)).await;

    let hold = hold_of(result);
    // Cleanup is *proven*, not accepted as either-or. `ReviewCleanupUnproven`
    // is a real category for a real failure — a reviewer whose process group
    // outlived termination — and accepting it here would have let the proposal's
    // "cleanup is confirmed" claim pass on a run that never proved it.
    assert_eq!(
        hold.category,
        AcceptanceHoldCategory::ReviewDeadlineExhausted,
        "the reviewer's owned process group must be proven quiescent: {}",
        hold.cleanup_diagnostics
    );
    assert!(
        hold.cleanup_confirmed,
        "cleanup evidence must be confirmed: {}",
        hold.cleanup_diagnostics
    );
    assert!(fixture.reviewer_started());
    assert_eq!(
        iteration, 0,
        "a bounded hold is not a verdict and consumes no attempt number"
    );

    let blocker = hold.to_stalled_blocker(fixture.change());
    assert_eq!(blocker.phase, "acceptance");
    assert!(!blocker.resumable);
    assert!(blocker.unblock_condition.is_none());
    assert!(blocker.worktree_preserved);
    assert!(
        blocker
            .evidence
            .iter()
            .any(|line| line == "process_group_cleanup=confirmed"),
        "the published blocker must carry the proof, not just the claim: {:?}",
        blocker.evidence
    );

    // The account of the hold is persisted externally, for an operator to read —
    // and it changed nothing in the target.
    assert!(
        fixture.manifests().await.diagnostics_path().is_file(),
        "the invocation's diagnostics belong in the external store"
    );
    let status = fixture.porcelain_status().await;
    let unexpected: Vec<&str> = status
        .lines()
        .filter(|line| !line.ends_with(REVIEWER_MARKER))
        .collect();
    assert!(
        unexpected.is_empty(),
        "a hold must leave the target untouched: {unexpected:?}"
    );
}

/// The result classifies as a hold rather than as a runtime limit, so it never
/// reaches verdict routing, escalation, or automatic re-dispatch.
#[tokio::test]
async fn acceptance_execution_boundary_hold_is_not_a_runtime_limit_or_a_verdict() {
    let fixture = Fixture::new("hold-classification", PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(non_terminating_reviewer());
    let ai_runner = runner(3600);

    let (result, _) =
        scoped_review_budget_millis_for_test(150, fixture.accept(&config, &ai_runner)).await;

    assert!(result.is_execution_hold());
    assert!(!result.is_runtime_limit(), "the outer limit must not fire");
    assert!(!result.is_canonical_verdict());
    assert!(
        !result.permits_acceptance_retry(),
        "a bounded hold must not be re-dispatched automatically"
    );
    assert!(
        crate::orchestration::acceptance::classify_invalid_acceptance_result(&result).is_none(),
        "a boundary decision is not an invalid reviewer response"
    );
}

/// A reviewer that emits its verdict before the deadline keeps it, even though
/// the runtime terminates the still-open process afterwards. Terminating a
/// process that already answered is finalization, not failure.
#[tokio::test]
async fn acceptance_execution_boundary_verdict_before_the_deadline_is_kept() {
    let fixture = Fixture::new("verdict-before-deadline", PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(format!(
        "sh -c 'touch {REVIEWER_MARKER}; printf \"ACCEPTANCE: PASS\\n\"; while true; do sleep 1; done'"
    ));
    let ai_runner = runner(3600);

    let (result, iteration) = scoped_review_budget_millis_for_test(
        2_000,
        crate::parallel::executor::scoped_verdict_grace_secs_for_test(
            1,
            fixture.accept(&config, &ai_runner),
        ),
    )
    .await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a verdict emitted before the deadline must stand, got {result:?}"
    );
    assert!(iteration > 0);
}

// ============================================================================
// Unchanged-input refusal: live while this owner runs, gone after a restart
// ============================================================================

/// A real deadline hold, then an unchanged retry in the *same* owner process.
///
/// The refusal holds: no gate, no reviewer, no attempt number. And it holds
/// without anything being written into the target worktree, which is what makes
/// it a live-owner fact rather than a durable one.
#[tokio::test]
async fn acceptance_execution_boundary_live_owner_refuses_unchanged_input() {
    let fixture = Fixture::new("live-owner-refusal", PROPOSAL_WITHOUT_GATES).await;
    let ai_runner = runner(3600);

    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        fixture.accept(
            &config_with_reviewer(non_terminating_reviewer()),
            &ai_runner,
        ),
    )
    .await;
    let first_hold = hold_of(first);
    assert_eq!(
        first_hold.category,
        AcceptanceHoldCategory::ReviewDeadlineExhausted
    );
    fixture.clear_reviewer_marker();

    // Same owner, same worktree. A canonical reviewer would pass instantly, so a
    // PASS here would mean the refusal was lost rather than that the change is
    // good.
    let (second, iteration) = fixture
        .accept(&config_with_reviewer(canonical_reviewer()), &ai_runner)
        .await;

    let hold = hold_of(second);
    assert_eq!(hold.fingerprint, first_hold.fingerprint);
    assert_eq!(
        hold.category,
        AcceptanceHoldCategory::ReviewDeadlineExhausted
    );
    assert!(hold
        .summary(fixture.change())
        .contains("unchanged_acceptance_input"));
    assert_eq!(iteration, 0);
    assert!(
        !fixture.reviewer_started(),
        "no reviewer may be dispatched for an unchanged input in the same owner"
    );
}

/// Restart: a real deadline hold, then an entirely fresh owner — a new live-hold
/// registry, new agent, new history, new runner — admits one fresh bounded
/// attempt from the same untouched worktree, with no repository mutation.
///
/// This replaces `acceptance_execution_boundary_restart_reaches_the_same_refusal`.
/// The old contract re-derived the refusal from a manifest the previous process
/// had written into the worktree. That was durable out-of-worktree-proof state
/// that changed the next action, which constitutional law 1 forbids once the
/// record moved outside the tree — so the refusal is now process-local, and a
/// restart deliberately permits one new bounded attempt while still reusing any
/// cached gate result that is still valid.
#[tokio::test]
async fn acceptance_execution_boundary_restart_admits_one_fresh_attempt() {
    let fixture = Fixture::new("restart-admits", PROPOSAL_WITHOUT_GATES).await;
    let ai_runner = runner(3600);

    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        fixture.accept(
            &config_with_reviewer(non_terminating_reviewer()),
            &ai_runner,
        ),
    )
    .await;
    assert!(first.is_execution_hold(), "{first:?}");
    fixture.clear_reviewer_marker();

    // "Restart": the owner's typed state is gone. The external cache and the
    // worktree are exactly as the previous owner left them.
    crate::orchestration::acceptance::execution_manifest::live_acceptance_holds()
        .clear(fixture.change());

    let (second, iteration) = fixture
        .accept(&config_with_reviewer(canonical_reviewer()), &ai_runner)
        .await;

    assert!(
        matches!(second, AcceptanceResult::Pass),
        "a restarted owner must admit one fresh bounded attempt, got {second:?}"
    );
    assert!(
        fixture.reviewer_started(),
        "the fresh attempt really dispatches a reviewer"
    );
    assert!(iteration > 0, "the fresh attempt records a real verdict");
    assert!(
        fixture
            .porcelain_status()
            .await
            .lines()
            .all(|line| line.ends_with(REVIEWER_MARKER)),
        "admission after restart must require no repository mutation"
    );
}

/// The shared admission port — the one classifier behind `retry_change`,
/// `retry_errors`, Start/F5, terminal-error queue intent, and automatic mark
/// settlement — refuses from live state and admits after a restart.
///
/// Both the explicit and the automatic edge go through this one `classify`, so
/// proving it here proves it for every caller rather than for one of them.
#[tokio::test]
async fn acceptance_execution_boundary_shared_admission_is_live_owner_scoped() {
    use crate::orchestration::acceptance::manifest_builder::build_manifest_for_workspace;
    use crate::orchestration::acceptance::verification_evidence::GitRepositoryFacts;
    use crate::orchestration::operator_command::AcceptanceAdmissionPort;

    let fixture = Fixture::new(
        "shared-admission",
        &proposal_with_gate("shared-admission", "sh -c true"),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());
    let workspace = fixture.path().to_path_buf();
    let resolve = move |_change: &str| Some(workspace.clone());

    let manifest = build_manifest_for_workspace(
        &GitRepositoryFacts,
        fixture.path(),
        &fixture.evidence().await,
        fixture.change(),
        config.get_accept_skill(),
        Some("main"),
        0,
    )
    .await
    .expect("the manifest must be derivable");

    // A live owner that recorded a deadline hold for this exact input.
    let live = Arc::new(LiveAcceptanceHolds::new());
    live.record(
        fixture.change(),
        LiveAcceptanceHold {
            category: AcceptanceHoldCategory::ReviewDeadlineExhausted,
            fingerprint: manifest.fingerprint(),
            review_base_ref: Some("main".to_string()),
        },
    );
    let admission = WorkspaceAcceptanceAdmission::with_holds(
        resolve.clone(),
        config.get_accept_skill(),
        None,
        live.clone(),
    );
    let refusal = scoped_acceptance_root_for_test(
        fixture.external.path().to_path_buf(),
        admission.classify(fixture.change()),
    )
    .await;
    match &refusal {
        AcceptanceAdmission::Refuse {
            outcome,
            category,
            components,
            ..
        } => {
            assert_eq!(*outcome, "unchanged_acceptance_input");
            assert_eq!(*category, AcceptanceHoldCategory::ReviewDeadlineExhausted);
            assert!(!components.is_empty());
        }
        other => panic!("a live owner must refuse the unchanged input, got {other:?}"),
    }

    // Restart: a new registry is what a restarted owner has, and it admits.
    let restarted = WorkspaceAcceptanceAdmission::with_holds(
        resolve.clone(),
        config.get_accept_skill(),
        None,
        Arc::new(LiveAcceptanceHolds::new()),
    );
    assert_eq!(
        scoped_acceptance_root_for_test(
            fixture.external.path().to_path_buf(),
            restarted.classify(fixture.change()),
        )
        .await,
        AcceptanceAdmission::Admit,
        "external cache must not recreate retry refusal after owner restart"
    );

    // And within the live owner, a repository change still restores eligibility.
    std::fs::write(fixture.path().join("repair.rs"), "fn repaired() {}\n")
        .or_fail("unexpected error");
    commit_all(fixture.path(), "Apply: repair").await;
    assert_eq!(
        scoped_acceptance_root_for_test(
            fixture.external.path().to_path_buf(),
            admission.classify(fixture.change()),
        )
        .await,
        AcceptanceAdmission::Admit,
        "a repository-visible change must restore live-owner retry eligibility"
    );
}

/// A repository-visible change to the candidate produces a new fingerprint and
/// permits exactly one fresh bounded attempt, even while the owner still
/// remembers the previous hold.
#[tokio::test]
async fn acceptance_execution_boundary_changed_input_permits_a_new_attempt() {
    let fixture = Fixture::new("changed-input", PROPOSAL_WITHOUT_GATES).await;
    let ai_runner = runner(3600);

    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        fixture.accept(
            &config_with_reviewer(non_terminating_reviewer()),
            &ai_runner,
        ),
    )
    .await;
    let refused_fingerprint = hold_of(first).fingerprint;
    fixture.clear_reviewer_marker();

    // The sanctioned recovery: a real repository-visible change.
    std::fs::write(fixture.path().join("repair.rs"), "fn repaired() {}\n")
        .or_fail("unexpected error");
    commit_all(fixture.path(), "Apply: repair").await;

    let (result, _) = fixture
        .accept(&config_with_reviewer(canonical_reviewer()), &ai_runner)
        .await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "a changed candidate must permit a new bounded attempt, got {result:?}"
    );
    assert!(fixture.reviewer_started());
    let stored = fixture
        .manifests()
        .await
        .load()
        .expect("the new attempt rewrites the manifest");
    assert_ne!(stored.fingerprint(), refused_fingerprint);
}

/// Settlement discards both halves of a change's Acceptance state: the external
/// cache and this owner's live refusal. Neither is authority, so dropping them
/// is always safe — and neither removal touches the target worktree, which is
/// the clause constitutional law 4 adds to "deleting out-of-worktree state may
/// discard caches".
#[tokio::test]
async fn acceptance_execution_boundary_settlement_clears_only_external_state() {
    let fixture = Fixture::new(
        "settlement-cleanup",
        &proposal_with_gate("settlement-cleanup", "sh -c true"),
    )
    .await;
    let ai_runner = runner(3600);

    // A real hold, so there is both a cache and a live refusal to clear.
    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        fixture.accept(
            &config_with_reviewer(non_terminating_reviewer()),
            &ai_runner,
        ),
    )
    .await;
    assert!(first.is_execution_hold(), "{first:?}");
    let store_root = fixture.store_root().await;
    assert!(store_root.join(format!("gates/{GATE}.json")).is_file());
    assert!(
        crate::orchestration::acceptance::execution_manifest::live_acceptance_holds()
            .get(fixture.change())
            .is_some(),
        "the live owner must be holding this change before settlement"
    );

    let config = config_with_reviewer(canonical_reviewer());
    scoped_acceptance_root_for_test(fixture.external.path().to_path_buf(), async {
        crate::parallel::executor::clear_acceptance_state(fixture.change(), fixture.path(), &config)
            .await
    })
    .await;

    assert!(
        !store_root.exists(),
        "settlement must discard the external Acceptance cache"
    );
    assert!(
        crate::orchestration::acceptance::execution_manifest::live_acceptance_holds()
            .get(fixture.change())
            .is_none(),
        "settlement must discard the live refusal too"
    );
    let status = fixture.porcelain_status().await;
    let unexpected: Vec<&str> = status
        .lines()
        .filter(|line| !line.ends_with(REVIEWER_MARKER))
        .collect();
    assert!(
        unexpected.is_empty(),
        "clearing out-of-worktree state must not modify the target worktree: {unexpected:?}"
    );
}

/// Editing the change's own artifacts is equally sufficient recovery: the
/// refusal names `change_inputs`, so that has to work.
#[tokio::test]
async fn acceptance_execution_boundary_changed_change_inputs_permit_a_new_attempt() {
    let fixture = Fixture::new("changed-inputs", PROPOSAL_WITHOUT_GATES).await;
    let ai_runner = runner(3600);

    let (first, _) = scoped_review_budget_millis_for_test(
        150,
        fixture.accept(
            &config_with_reviewer(non_terminating_reviewer()),
            &ai_runner,
        ),
    )
    .await;
    assert!(first.is_execution_hold(), "{first:?}");
    fixture.clear_reviewer_marker();

    std::fs::write(
        fixture
            .path()
            .join("openspec/changes")
            .join(fixture.change())
            .join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n- [x] 2. also done\n",
    )
    .or_fail("unexpected error");

    let (result, _) = fixture
        .accept(&config_with_reviewer(canonical_reviewer()), &ai_runner)
        .await;

    assert!(
        matches!(result, AcceptanceResult::Pass),
        "changed change inputs must restore eligibility, got {result:?}"
    );
}

// ============================================================================
// Invalid required declarations fail closed as a verdict, not a hold
// ============================================================================

/// One verification declaration entry for a hand-assembled proposal.
fn declaration_yaml(change: &str, id: &str, command: &str) -> String {
    format!(
        "\x20 - id: {id}\n\
\x20   requirement: the gate passes\n\
\x20   phase: pre-integration\n\
\x20   owner: conflux-acceptance\n\
\x20   trigger: pull-request-validation\n\
\x20   automation: openspec/changes/{change}/proposal.md\n\
\x20   evidence: \"{command}\"\n\
\x20   rerun: \"{command}\"\n\
\x20   prerequisites: []\n\
\x20   execution_class: repository-local\n\
\x20   completion_role: change-blocking\n"
    )
}

fn proposal_with_declarations(entries: &[String]) -> String {
    format!("---\nverifications:\n{}---\n# Change\n", entries.concat())
}

/// A gate script outside the target that appends one line per execution, so
/// "ran exactly once" and "never ran" are counts rather than inferences.
fn counting_gate(external: &Path) -> (String, PathBuf) {
    let counter = external.join("gate-runs.txt");
    let script = external.join("gate.sh");
    std::fs::write(&script, format!("echo run >> '{}'\n", counter.display()))
        .or_fail("unexpected error");
    (format!("sh {}", script.display()), counter)
}

fn gate_runs(counter: &Path) -> usize {
    std::fs::read_to_string(counter)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

/// A required declaration the runtime cannot supervise settles as a `fail`
/// verdict about the candidate: a real attempt with `[DECLARATION_INVALID]`
/// findings, no reviewer, no gate — not even the valid one beside it — no
/// evidence, no hold, and a persisted manifest that names the defect.
#[tokio::test]
async fn required_verification_fail_closed_invalid_manifest_fails_without_running_anything() {
    let change = "required-verification-fail-closed-invalid";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let (valid_command, counter) = counting_gate(gate_root.path());
    let proposal = proposal_with_declarations(&[
        declaration_yaml(change, "valid-gate", &valid_command),
        declaration_yaml(change, "shell-gate", &format!("{valid_command} && true")),
    ]);
    let fixture = Fixture::new(change, &proposal).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, iteration) = fixture.accept(&config, &runner(3600)).await;

    let findings = match result {
        AcceptanceResult::Fail { findings } => findings,
        other => panic!("an invalid required manifest must FAIL the candidate, got {other:?}"),
    };
    let finding = findings
        .iter()
        .map(|finding| finding.text().to_string())
        .find(|text| text.starts_with("[DECLARATION_INVALID]"))
        .unwrap_or_else(|| panic!("a [DECLARATION_INVALID] finding is required: {findings:?}"));
    assert!(finding.contains("'shell-gate'"), "{finding}");
    assert!(finding.contains("rerun"), "{finding}");
    assert!(finding.contains("declaration_shell_syntax"), "{finding}");
    assert!(
        finding.contains(&format!("openspec/changes/{change}/proposal.md")),
        "{finding}"
    );
    assert!(!finding.contains("tool_unavailable"), "{finding}");
    assert!(
        iteration > 0,
        "a declaration verdict records a real attempt"
    );

    assert!(
        !fixture.reviewer_started(),
        "the AI runner must not be invoked"
    );
    assert_eq!(gate_runs(&counter), 0, "no declared gate may be launched");
    assert!(
        !fixture.store_root().await.join("gates").exists(),
        "no evidence may be written"
    );
    assert!(
        crate::orchestration::acceptance::execution_manifest::live_acceptance_holds()
            .get(change)
            .is_none(),
        "a verdict records no execution hold"
    );
    let manifest = fixture
        .manifests()
        .await
        .load()
        .expect("the invalid manifest is persisted for diagnosis");
    assert!(manifest.gates.is_empty());
    assert_eq!(manifest.declaration_defects.len(), 1);
    assert_eq!(
        manifest.declaration_defects[0].verification_id,
        "shell-gate"
    );
    assert_eq!(manifest.declaration_defects[0].field, "rerun");
    assert!(fixture.porcelain_status().await.is_empty());
}

/// One valid required declaration executes its gate exactly once and proceeds
/// to the reviewer.
#[tokio::test]
async fn required_verification_fail_closed_valid_gate_runs_exactly_once() {
    let change = "required-verification-fail-closed-valid";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let (command, counter) = counting_gate(gate_root.path());
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(change, "valid-gate", &command)]),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
    assert_eq!(
        gate_runs(&counter),
        1,
        "the declared gate runs exactly once"
    );
    assert!(fixture.reviewer_started());
    let manifest = fixture.manifests().await.load().expect("manifest");
    assert!(!manifest.is_declaration_invalid());
    assert_eq!(manifest.gates.len(), 1);
}

/// A legitimately empty allowlist keeps today's behavior: the reviewer runs
/// with a closed, empty boundary, and the manifest is not an invalid one.
#[tokio::test]
async fn required_verification_fail_closed_empty_allowlist_still_proceeds() {
    let fixture = Fixture::new(
        "required-verification-fail-closed-empty",
        PROPOSAL_WITHOUT_GATES,
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
    assert!(fixture.reviewer_started());
    let manifest = fixture.manifests().await.load().expect("manifest");
    assert!(!manifest.has_gates());
    assert!(!manifest.is_declaration_invalid());
}

// ============================================================================
// Acceptance provenance: who decided, about which candidate
// ============================================================================

use crate::events::ExecutionEvent;
use crate::orchestration::acceptance::provenance::{
    AcceptanceProvenance, DecisionSource, DivergenceStatus, DivergenceUnknownReason, ReasonCode,
    Remediation,
};

const ARG_SENTINEL: &str = "ARG-SENTINEL-7f3a91";
const ENV_SENTINEL: &str = "ENV-SENTINEL-c4e2b8";

/// Run one invocation with a live event channel and return the provenance
/// records it published, in order.
async fn accept_with_provenance(
    fixture: &Fixture,
    config: &OrchestratorConfig,
) -> (AcceptanceResult, Vec<AcceptanceProvenance>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let ai_runner = runner(3600);
    let mut agent = AgentRunner::new(config.clone());
    let tail = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));
    let (result, _) = scoped_acceptance_root_for_test(
        fixture.external.path().to_path_buf(),
        execute_acceptance_in_workspace(
            fixture.change(),
            fixture.path(),
            &mut agent,
            Some(tx),
            None,
            &ai_runner,
            config,
            &tail,
            &history,
            Some("main"),
            None,
            crate::orchestration::acceptance::AcceptanceCommandMode::Normal,
        ),
    )
    .await
    .or_fail("acceptance must return a typed result");
    let mut records = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::AcceptanceSettled {
            change_id,
            provenance,
        } = event
        {
            assert_eq!(change_id, fixture.change());
            records.push(*provenance);
        }
    }
    (result, records)
}

fn only(records: Vec<AcceptanceProvenance>) -> AcceptanceProvenance {
    assert_eq!(
        records.len(),
        1,
        "exactly one record per settlement: {records:?}"
    );
    records.into_iter().next().expect("one record")
}

async fn git_out(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await
        .or_fail("git runs");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

async fn candidate_proposal_blob(fixture: &Fixture) -> String {
    git_out(
        fixture.path(),
        &[
            "rev-parse",
            &format!("HEAD:openspec/changes/{}/proposal.md", fixture.change()),
        ],
    )
    .await
}

/// A reviewer that emits a canonical FAIL with one finding.
fn failing_reviewer() -> String {
    format!(
        "sh -c 'touch {REVIEWER_MARKER}; echo ACCEPTANCE: FAIL; echo; echo FINDINGS:; echo - \
         missing regression coverage'"
    )
}

/// A defective required declaration settles natively, before any reviewer,
/// naming the defect, the declaration, and the exact candidate content judged.
#[tokio::test]
async fn acceptance_provenance_declaration_invalid_is_a_native_stop() {
    let change = "acceptance-provenance-declaration";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let (command, counter) = counting_gate(gate_root.path());
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(
            change,
            "shell-gate",
            &format!("{command} && true"),
        )]),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, records) = accept_with_provenance(&fixture, &config).await;

    assert!(
        matches!(result, AcceptanceResult::Fail { .. }),
        "{result:?}"
    );
    assert!(!fixture.reviewer_started());
    assert_eq!(gate_runs(&counter), 0);
    let record = only(records);
    let manifest = fixture.manifests().await.load().expect("manifest");
    assert_eq!(record.decision_source, DecisionSource::NativeVerification);
    assert!(!record.reviewer_started);
    assert_eq!(record.reason_code, ReasonCode::DeclarationInvalid);
    assert_eq!(
        record.defect_code.as_deref(),
        Some(manifest.declaration_defects[0].code.as_str())
    );
    assert_eq!(record.verification_id.as_deref(), Some("shell-gate"));
    assert_eq!(
        record.declaration_path,
        Some(format!("openspec/changes/{change}/proposal.md"))
    );
    assert_eq!(
        record.candidate_commit.as_deref(),
        Some(manifest.candidate_commit_oid.as_str())
    );
    assert_eq!(
        record.change_input_digest.as_deref(),
        Some(manifest.change_input_digest.as_str())
    );
    assert_eq!(
        record.proposal_content_id,
        Some(candidate_proposal_blob(&fixture).await)
    );
    // The candidate *is* `main` here, so the bound base carries the same blob.
    assert_eq!(
        record.candidate_divergence.status,
        DivergenceStatus::Identical
    );
    assert_eq!(record.remediation, Remediation::FixDeclaration);
    assert!(fixture.porcelain_status().await.is_empty());
}

/// A missing executable settles as `tool_unavailable`, names the unresolved
/// token, and recommends satisfying the environment.
#[tokio::test]
async fn acceptance_provenance_tool_unavailable_names_the_unresolved_program() {
    let change = "acceptance-provenance-tool";
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(
            change,
            "missing-tool",
            "cflx-definitely-missing-tool check --all",
        )]),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, records) = accept_with_provenance(&fixture, &config).await;

    assert!(
        matches!(result, AcceptanceResult::Stalled { .. }),
        "{result:?}"
    );
    assert!(!fixture.reviewer_started());
    let record = only(records);
    assert_eq!(record.decision_source, DecisionSource::NativeVerification);
    assert_eq!(record.reason_code, ReasonCode::ToolUnavailable);
    assert_eq!(
        record.program.as_deref(),
        Some("cflx-definitely-missing-tool")
    );
    assert_eq!(record.argument_count, Some(2));
    assert_eq!(record.verification_id.as_deref(), Some("missing-tool"));
    assert_eq!(record.remediation, Remediation::SatisfyEnvironment);
}

/// A failing gate whose declaration carries a sentinel argument and a sentinel
/// assignment value settles as `gate_failed` with counts and names — and
/// neither sentinel reaches the record, the API row, the popup lines, the copy
/// text, the settlement log line, or `diagnostics.json`.
#[tokio::test]
async fn acceptance_provenance_failed_gate_is_redacted_on_every_surface() {
    let change = "acceptance-provenance-redacted";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let script = gate_root.path().join("fail.sh");
    std::fs::write(&script, "exit 1\n").or_fail("unexpected error");
    let command = format!(
        "SECRET_TOKEN={ENV_SENTINEL} sh {} --token={ARG_SENTINEL}",
        script.display()
    );
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(change, "failing-gate", &command)]),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, records) = accept_with_provenance(&fixture, &config).await;

    assert!(
        matches!(result, AcceptanceResult::Fail { .. }),
        "{result:?}"
    );
    assert!(!fixture.reviewer_started());
    let record = only(records);
    assert_eq!(record.reason_code, ReasonCode::GateFailed);
    assert_eq!(record.remediation, Remediation::RepairCandidate);
    assert_eq!(record.verification_id.as_deref(), Some("failing-gate"));
    assert_eq!(record.argument_count, Some(2));
    assert_eq!(record.env_names, vec!["SECRET_TOKEN".to_string()]);
    assert!(
        record
            .program
            .as_deref()
            .is_some_and(|program| program.ends_with("/sh")),
        "the resolved executable is named: {:?}",
        record.program
    );

    let mut surfaces = vec![
        serde_json::to_string(&record).or_fail("serializes"),
        record.log_line(change),
    ];

    // The API row, through the reducer and the production projection.
    #[cfg(feature = "web-monitoring")]
    {
        let reducer = Arc::new(tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::new(vec![change.to_string()], 0),
        ));
        let web_state = crate::web::state::WebState::new(&[crate::openspec::Change {
            id: change.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: Default::default(),
        }]);
        web_state.set_shared_state(reducer.clone()).await;
        let event = ExecutionEvent::AcceptanceSettled {
            change_id: change.to_string(),
            provenance: Box::new(record.clone()),
        };
        reducer.write().await.apply_execution_event(&event);
        web_state.apply_execution_event(&event).await;
        web_state.sync_remote_control_projection().await;
        let (snapshot, _, _) = web_state.remote_control().projection().snapshot();
        let row = snapshot
            .changes
            .iter()
            .find(|row| row.id == change)
            .or_fail("the change is projected");
        assert_eq!(row.acceptance_provenance.as_ref(), Some(&record));
        surfaces.push(serde_json::to_string(row).or_fail("serializes"));
    }

    // The popup lines and the copy text.
    let popup = crate::tui::ErrorDetailsPopup {
        change_id: change.to_string(),
        error: "Acceptance failed".to_string(),
        provenance: Some(record.clone()),
        scroll: 0,
        copy_feedback: None,
    };
    let copy = popup.clipboard_text();
    assert!(copy.contains("Command: "), "{copy}");
    assert!(copy.contains("env: SECRET_TOKEN"), "{copy}");
    surfaces.push(copy);

    if let Ok(diagnostics) = std::fs::read_to_string(fixture.manifests().await.diagnostics_path()) {
        surfaces.push(diagnostics);
    }

    for surface in &surfaces {
        assert!(
            !surface.contains(ARG_SENTINEL),
            "argument value leaked: {surface}"
        );
        assert!(
            !surface.contains(ENV_SENTINEL),
            "environment value leaked: {surface}"
        );
    }
}

/// Reviewer verdicts are labeled as such: the reviewer really started, no gate
/// is named, and the remediation follows the verdict.
#[tokio::test]
async fn acceptance_provenance_reviewer_verdicts_are_distinctly_labeled() {
    for (change, reviewer, reason, remediation) in [
        (
            "acceptance-provenance-reviewer-fail",
            failing_reviewer(),
            ReasonCode::ReviewerFail,
            Remediation::RepairCandidate,
        ),
        (
            "acceptance-provenance-reviewer-pass",
            canonical_reviewer(),
            ReasonCode::ReviewerPass,
            Remediation::None,
        ),
    ] {
        let fixture = Fixture::new(change, PROPOSAL_WITHOUT_GATES).await;
        let config = config_with_reviewer(reviewer);

        let (_, records) = accept_with_provenance(&fixture, &config).await;

        assert!(fixture.reviewer_started());
        let record = only(records);
        assert_eq!(record.decision_source, DecisionSource::Reviewer, "{change}");
        assert!(record.reviewer_started);
        assert_eq!(record.reason_code, reason);
        assert!(record.verification_id.is_none());
        assert!(record.declaration_path.is_none());
        assert!(record.program.is_none());
        assert_eq!(record.remediation, remediation);
        assert_eq!(
            record.proposal_content_id,
            Some(candidate_proposal_blob(&fixture).await)
        );
    }
}

/// A candidate on its own branch, reviewed against `main`.
async fn branched_fixture(
    change: &str,
    base_proposal: Option<&str>,
    candidate_proposal: &str,
) -> Fixture {
    let repo = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo.path()).await;
    let change_dir = repo.path().join("openspec/changes").join(change);
    if let Some(base) = base_proposal {
        std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
        std::fs::write(change_dir.join("proposal.md"), base).or_fail("unexpected error");
        commit_all(repo.path(), "Base proposal").await;
    }
    git_out(repo.path(), &["checkout", "-b", "candidate"]).await;
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(change_dir.join("proposal.md"), candidate_proposal).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");
    commit_all(repo.path(), &format!("Apply: {change}")).await;
    Fixture {
        repo,
        external: TempDir::new().or_fail("unexpected error"),
        change: change.to_string(),
    }
}

/// Divergence compares the bound review base only: a base whose `proposal.md`
/// differs is `diverged` with both identities, and turns a declaration defect
/// into `refresh_candidate`; a base without the proposal is `unknown`.
#[tokio::test]
async fn acceptance_provenance_divergence_uses_the_bound_review_base() {
    let change = "acceptance-provenance-diverged";
    let invalid =
        proposal_with_declarations(&[declaration_yaml(change, "shell-gate", "sh -c true && true")]);
    let fixture = branched_fixture(change, Some(PROPOSAL_WITHOUT_GATES), &invalid).await;
    let config = config_with_reviewer(canonical_reviewer());

    let (_, records) = accept_with_provenance(&fixture, &config).await;

    let record = only(records);
    let main = git_out(fixture.path(), &["rev-parse", "main"]).await;
    let base_blob = git_out(
        fixture.path(),
        &[
            "rev-parse",
            &format!("main:openspec/changes/{change}/proposal.md"),
        ],
    )
    .await;
    let divergence = &record.candidate_divergence;
    assert_eq!(record.reason_code, ReasonCode::DeclarationInvalid);
    assert_eq!(divergence.status, DivergenceStatus::Diverged);
    assert_eq!(divergence.source_ref.as_deref(), Some("main"));
    assert_eq!(divergence.source_commit.as_deref(), Some(main.as_str()));
    assert_eq!(divergence.source_blob.as_deref(), Some(base_blob.as_str()));
    assert_eq!(
        divergence.candidate_blob,
        Some(candidate_proposal_blob(&fixture).await)
    );
    assert_eq!(record.remediation, Remediation::RefreshCandidate);

    let change = "acceptance-provenance-source-missing";
    let fixture = branched_fixture(change, None, PROPOSAL_WITHOUT_GATES).await;
    let (_, records) = accept_with_provenance(&fixture, &config).await;
    let record = only(records);
    assert_eq!(
        record.candidate_divergence.status,
        DivergenceStatus::Unknown
    );
    assert_eq!(
        record.candidate_divergence.reason,
        Some(DivergenceUnknownReason::SourceProposalMissing)
    );
}

/// The record is not authority: admission for the same workspace is identical
/// with the external state root present and deleted after a settlement.
#[tokio::test]
async fn acceptance_provenance_state_root_loss_changes_no_admission() {
    use crate::orchestration::operator_command::AcceptanceAdmissionPort;

    let change = "acceptance-provenance-state-root";
    let fixture = Fixture::new(change, PROPOSAL_WITHOUT_GATES).await;
    let config = config_with_reviewer(canonical_reviewer());
    let (_, records) = accept_with_provenance(&fixture, &config).await;
    assert_eq!(only(records).reason_code, ReasonCode::ReviewerPass);

    let workspace = fixture.path().to_path_buf();
    let admission = WorkspaceAcceptanceAdmission::with_holds(
        move |_change: &str| Some(workspace.clone()),
        config.get_accept_skill(),
        None,
        Arc::new(LiveAcceptanceHolds::new()),
    );
    let classify = || {
        scoped_acceptance_root_for_test(
            fixture.external.path().to_path_buf(),
            admission.classify(fixture.change()),
        )
    };
    let with_state = classify().await;
    std::fs::remove_dir_all(fixture.external.path()).or_fail("state root removable");
    let without_state = classify().await;
    assert_eq!(with_state, without_state);
    assert_eq!(without_state, AcceptanceAdmission::Admit);
}

// ============================================================================
// verification_declaration_integrity: the candidate's own metadata, read through
// the fallible source reader
// ============================================================================

/// A fixture whose base commit carries a valid required gate and whose Apply
/// commit then rewrote the candidate's `proposal.md` to `candidate`.
///
/// The base proposal would pass strict validation; nothing about it may be
/// consulted as evidence about the candidate.
async fn apply_mutated_fixture(change: &str, command: &str, candidate: &str) -> Fixture {
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(change, "valid-gate", command)]),
    )
    .await;
    std::fs::write(
        fixture
            .path()
            .join("openspec/changes")
            .join(change)
            .join("proposal.md"),
        candidate,
    )
    .or_fail("unexpected error");
    commit_all(fixture.path(), &format!("Apply: mutate {change}")).await;
    fixture
}

async fn head_commit(fixture: &Fixture) -> String {
    git_out(fixture.path(), &["rev-parse", "HEAD"]).await
}

#[tokio::test]
async fn verification_declaration_integrity_mutated_candidate_metadata_fails_natively() {
    for (suffix, candidate, field) in [
        (
            "wrong-typed",
            "---\nverifications:\n  - id: valid-gate\n    rerun: [cargo, test]\n    completion_role: change-blocking\n---\n# Change\n",
            "rerun",
        ),
        (
            "unclosed",
            "---\nverifications:\n  - id: valid-gate\n    completion_role: change-blocking\n# Change without a closing delimiter\n",
            "frontmatter",
        ),
    ] {
        let change = format!("verification-declaration-integrity-{suffix}");
        let gate_root = TempDir::new().or_fail("unexpected error");
        let (command, counter) = counting_gate(gate_root.path());
        let fixture = apply_mutated_fixture(&change, &command, candidate).await;
        let candidate_commit = head_commit(&fixture).await;
        let config = config_with_reviewer(canonical_reviewer());

        let (result, iteration) = fixture.accept(&config, &runner(3600)).await;

        let findings = match result {
            AcceptanceResult::Fail { findings } => findings,
            other => panic!("{suffix}: defective candidate metadata must FAIL, got {other:?}"),
        };
        assert_eq!(findings.len(), 1, "{suffix}: {findings:?}");
        let finding = &findings[0];
        assert!(
            finding.text().starts_with(&format!(
                "[DECLARATION_INVALID] openspec/changes/{change}/proposal.md: "
            )),
            "{finding}"
        );
        assert!(
            finding.text().contains("declaration_metadata_invalid"),
            "{finding}"
        );
        assert!(!finding.text().contains("cargo"), "{finding}");
        assert!(finding.native_identity().is_some());
        assert!(!finding.declares_paths());
        assert!(iteration > 0, "a declaration verdict records a real attempt");

        assert!(!fixture.reviewer_started(), "{suffix}: no reviewer");
        assert_eq!(gate_runs(&counter), 0, "{suffix}: no gate");
        assert!(
            !fixture.store_root().await.join("gates").exists(),
            "{suffix}: no evidence written"
        );
        assert!(
            crate::orchestration::acceptance::execution_manifest::live_acceptance_holds()
                .get(&change)
                .is_none(),
            "{suffix}: a verdict records no execution hold"
        );
        let manifest = fixture.manifests().await.load().expect("manifest");
        assert_eq!(manifest.candidate_commit_oid, candidate_commit);
        assert!(manifest.gates.is_empty());
        assert_eq!(manifest.declaration_defects.len(), 1);
        assert_eq!(
            manifest.declaration_defects[0].code,
            "declaration_metadata_invalid"
        );
        assert_eq!(manifest.declaration_defects[0].field, field);
        assert!(fixture.porcelain_status().await.is_empty());
    }
}

#[tokio::test]
async fn verification_declaration_integrity_missing_candidate_proposal_is_a_manifest_hold() {
    let change = "verification-declaration-integrity-missing";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let (command, counter) = counting_gate(gate_root.path());
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(change, "valid-gate", &command)]),
    )
    .await;
    std::fs::remove_file(
        fixture
            .path()
            .join("openspec/changes")
            .join(change)
            .join("proposal.md"),
    )
    .or_fail("unexpected error");
    commit_all(fixture.path(), "Apply: drop the proposal").await;
    let config = config_with_reviewer(canonical_reviewer());

    let (result, _) = fixture.accept(&config, &runner(3600)).await;

    let hold = hold_of(result);
    assert_eq!(hold.category, AcceptanceHoldCategory::ManifestMalformed);
    assert_eq!(hold.fingerprint, "0".repeat(64));
    assert!(
        hold.evidence.iter().any(|line| line
            .contains(&format!("openspec/changes/{change}/proposal.md"))
            && line.contains("NotFound")),
        "{:?}",
        hold.evidence
    );
    assert!(!fixture.reviewer_started(), "no reviewer");
    assert_eq!(gate_runs(&counter), 0, "no gate");
    assert!(
        !fixture.store_root().await.join("gates").exists(),
        "no evidence written"
    );
    assert!(
        fixture.manifests().await.load().is_err(),
        "no manifest was derived"
    );
}

#[tokio::test]
async fn verification_declaration_integrity_valid_and_empty_candidates_still_run() {
    let change = "verification-declaration-integrity-valid";
    let gate_root = TempDir::new().or_fail("unexpected error");
    let (command, counter) = counting_gate(gate_root.path());
    let fixture = Fixture::new(
        change,
        &proposal_with_declarations(&[declaration_yaml(change, "valid-gate", &command)]),
    )
    .await;
    let config = config_with_reviewer(canonical_reviewer());
    let (result, _) = fixture.accept(&config, &runner(3600)).await;
    assert!(matches!(result, AcceptanceResult::Pass), "{result:?}");
    assert_eq!(gate_runs(&counter), 1);
    assert!(fixture.reviewer_started());

    for (suffix, candidate) in [
        ("legacy", "# Legacy proposal without frontmatter\n"),
        ("null", "---\nverifications:\n---\n# Change\n"),
        ("empty-list", "---\nverifications: []\n---\n# Change\n"),
    ] {
        let change = format!("verification-declaration-integrity-empty-{suffix}");
        let fixture = Fixture::new(&change, candidate).await;
        let (result, _) = fixture.accept(&config, &runner(3600)).await;
        assert!(
            matches!(result, AcceptanceResult::Pass),
            "{suffix}: {result:?}"
        );
        assert!(fixture.reviewer_started(), "{suffix}: reviewer runs");
        let manifest = fixture.manifests().await.load().expect("manifest");
        assert!(!manifest.has_gates());
        assert!(!manifest.is_declaration_invalid());
    }
}
