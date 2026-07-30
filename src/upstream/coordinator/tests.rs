//! Unit coverage for the upstream integration workflow.
//!
//! These tests are unit-scoped by construction: every external boundary (Git,
//! the verification process, the repair agent, the observer) is an in-memory
//! double. No real repository, process, network, clock, or filesystem state is
//! touched, so the whole module runs far below the one-second budget. Real
//! Git/worktree/bare-remote behavior is covered by the heavy E2E suite.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use async_trait::async_trait;

use super::*;
use crate::upstream::classify::MergeRepositoryState;
use crate::upstream::ports::{
    MergeCommandResult, PushCommandResult, RepairAttemptResult, VerificationOutcome,
};
use crate::upstream::spine::{CommitTreeEvidence, SpineCommit};
use crate::upstream::trailers::format_upstream_merge_message;

// ── In-memory doubles ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FakeCommit {
    message: String,
    parents: Vec<String>,
    tree_evidence: CommitTreeEvidence,
}

/// What the next `merge_no_ff` call should do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MergeBehavior {
    /// Create the merge commit and report success.
    Succeed,
    /// Leave `MERGE_HEAD` plus unmerged entries, reporting a non-zero exit.
    Conflict,
    /// Non-zero exit with no repairable merge state.
    CommandFailure,
}

/// What the next `push_porcelain` call should do.
#[derive(Debug, Clone)]
enum PushBehavior {
    Succeed,
    Reject { porcelain: String },
    FailWithoutRefStatus,
}

#[derive(Debug, Default)]
struct FakeGitInner {
    commits: HashMap<String, FakeCommit>,
    head: String,
    /// Authoritative remote state, keyed by `remote/branch`.
    remote_refs: HashMap<String, String>,
    /// Local remote-tracking refs, refreshed by `fetch`.
    tracking_refs: HashMap<String, String>,
    configured_remotes: Vec<String>,
    current_branch: Option<String>,
    merge_state: MergeRepositoryState,
    working_tree_clean: bool,
    porcelain_v2: String,
    merge_behaviors: VecDeque<MergeBehavior>,
    push_behaviors: VecDeque<PushBehavior>,
    fetch_error: Option<String>,
    // Observed call counts / arguments.
    fetch_calls: usize,
    merge_calls: Vec<(String, String)>,
    push_calls: usize,
    successful_pushes: usize,
    next_sha: usize,
}

#[derive(Default)]
struct FakeGit {
    inner: Mutex<FakeGitInner>,
}

/// Deterministic 40-hex object name for a readable seed.
///
/// Identity trailers are only valid for full hex SHAs, so the doubles must
/// produce real-shaped object names rather than readable placeholders.
fn sha(seed: &str) -> String {
    let mut s: String = seed.bytes().map(|b| format!("{:02x}", b)).collect();
    while s.len() < 40 {
        s.push('0');
    }
    s.truncate(40);
    s
}

impl FakeGit {
    /// Linear history `root -> local`, remote `origin/main` at `root`.
    fn new_linear() -> Self {
        let mut inner = FakeGitInner {
            working_tree_clean: true,
            current_branch: Some("main".to_string()),
            configured_remotes: vec!["origin".to_string()],
            next_sha: 100,
            ..Default::default()
        };
        inner.commits.insert(
            sha("root"),
            FakeCommit {
                message: "root\n".into(),
                parents: vec![],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.commits.insert(
            sha("local"),
            FakeCommit {
                message: "Merge change: a\n".into(),
                parents: vec![sha("root"), sha("wt")],
                tree_evidence: CommitTreeEvidence::new(["a".to_string()], []),
            },
        );
        inner.commits.insert(
            sha("wt"),
            FakeCommit {
                message: "work\n".into(),
                parents: vec![sha("root")],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("local");
        inner.remote_refs.insert("origin/main".into(), sha("root"));
        Self {
            inner: Mutex::new(inner),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeGitInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Advance the authoritative remote by one commit descending from `parent`.
    fn advance_remote(&self, seed: &str, parent: &str) -> String {
        let mut inner = self.lock();
        let new = sha(seed);
        inner.commits.insert(
            new.clone(),
            FakeCommit {
                message: format!("{}\n", seed),
                parents: vec![parent.to_string()],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.remote_refs.insert("origin/main".into(), new.clone());
        new
    }

    fn set_remote_ref(&self, value: &str) {
        self.lock()
            .remote_refs
            .insert("origin/main".into(), value.to_string());
    }

    fn head(&self) -> String {
        self.lock().head.clone()
    }

    fn merge_calls(&self) -> Vec<(String, String)> {
        self.lock().merge_calls.clone()
    }

    fn fetch_calls(&self) -> usize {
        self.lock().fetch_calls
    }

    fn successful_pushes(&self) -> usize {
        self.lock().successful_pushes
    }

    fn push_calls(&self) -> usize {
        self.lock().push_calls
    }

    fn is_ancestor_locked(inner: &FakeGitInner, ancestor: &str, descendant: &str) -> bool {
        if ancestor == descendant {
            return true;
        }
        let mut stack = vec![descendant.to_string()];
        let mut seen = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let Some(commit) = inner.commits.get(&current) else {
                continue;
            };
            for parent in &commit.parents {
                if parent == ancestor {
                    return true;
                }
                stack.push(parent.clone());
            }
        }
        false
    }
}

#[async_trait]
impl UpstreamGit for FakeGit {
    async fn remote_configured(&self, remote: &str) -> PortResult<bool> {
        Ok(self.lock().configured_remotes.iter().any(|r| r == remote))
    }

    async fn current_branch(&self) -> PortResult<Option<String>> {
        Ok(self.lock().current_branch.clone())
    }

    async fn fetch(&self, remote: &str, branch: &str) -> PortResult<()> {
        let mut inner = self.lock();
        inner.fetch_calls += 1;
        if let Some(err) = inner.fetch_error.clone() {
            return Err(UpstreamPortError::new("git fetch", err));
        }
        let key = format!("{}/{}", remote, branch);
        if let Some(value) = inner.remote_refs.get(&key).cloned() {
            inner.tracking_refs.insert(key, value);
        } else {
            inner.tracking_refs.remove(&key);
        }
        Ok(())
    }

    async fn fetched_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>> {
        Ok(self
            .lock()
            .tracking_refs
            .get(&format!("{}/{}", remote, branch))
            .cloned())
    }

    async fn head_sha(&self) -> PortResult<String> {
        Ok(self.lock().head.clone())
    }

    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> PortResult<bool> {
        let inner = self.lock();
        Ok(FakeGit::is_ancestor_locked(&inner, ancestor, descendant))
    }

    async fn merge_base(&self, a: &str, b: &str) -> PortResult<String> {
        let inner = self.lock();
        if FakeGit::is_ancestor_locked(&inner, a, b) {
            return Ok(a.to_string());
        }
        if FakeGit::is_ancestor_locked(&inner, b, a) {
            return Ok(b.to_string());
        }
        Ok(sha("root"))
    }

    async fn merge_no_ff(&self, target: &str, message: &str) -> PortResult<MergeCommandResult> {
        let mut inner = self.lock();
        inner
            .merge_calls
            .push((target.to_string(), message.to_string()));
        let behavior = inner
            .merge_behaviors
            .pop_front()
            .unwrap_or(MergeBehavior::Succeed);
        match behavior {
            MergeBehavior::Succeed => {
                inner.next_sha += 1;
                let new = sha(&format!("merge{}", inner.next_sha));
                let head = inner.head.clone();
                inner.commits.insert(
                    new.clone(),
                    FakeCommit {
                        message: message.to_string(),
                        parents: vec![head, target.to_string()],
                        tree_evidence: CommitTreeEvidence::default(),
                    },
                );
                inner.head = new;
                inner.merge_state = MergeRepositoryState {
                    merge_head_present: false,
                    has_unmerged_entries: false,
                };
                Ok(MergeCommandResult {
                    exit_success: true,
                    state: inner.merge_state,
                })
            }
            MergeBehavior::Conflict => {
                inner.merge_state = MergeRepositoryState {
                    merge_head_present: true,
                    has_unmerged_entries: true,
                };
                inner.working_tree_clean = false;
                Ok(MergeCommandResult {
                    exit_success: false,
                    state: inner.merge_state,
                })
            }
            MergeBehavior::CommandFailure => {
                inner.merge_state = MergeRepositoryState {
                    merge_head_present: false,
                    has_unmerged_entries: false,
                };
                Ok(MergeCommandResult {
                    exit_success: false,
                    state: inner.merge_state,
                })
            }
        }
    }

    async fn merge_repository_state(&self) -> PortResult<MergeRepositoryState> {
        Ok(self.lock().merge_state)
    }

    async fn is_working_tree_clean(&self) -> PortResult<bool> {
        Ok(self.lock().working_tree_clean)
    }

    async fn status_porcelain_v2(&self) -> PortResult<String> {
        Ok(self.lock().porcelain_v2.clone())
    }

    async fn commit_message(&self, commit: &str) -> PortResult<String> {
        self.lock()
            .commits
            .get(commit)
            .map(|c| c.message.clone())
            .ok_or_else(|| UpstreamPortError::new("git log", format!("unknown commit {}", commit)))
    }

    async fn commit_parents(&self, commit: &str) -> PortResult<Vec<String>> {
        self.lock()
            .commits
            .get(commit)
            .map(|c| c.parents.clone())
            .ok_or_else(|| {
                UpstreamPortError::new("git rev-list", format!("unknown commit {}", commit))
            })
    }

    async fn first_parent_commits(
        &self,
        from_exclusive: Option<&str>,
        to: &str,
        limit: Option<usize>,
    ) -> PortResult<Vec<SpineCommit>> {
        let inner = self.lock();
        let mut collected = Vec::new();
        let mut current = Some(to.to_string());
        while let Some(sha_value) = current {
            if Some(sha_value.as_str()) == from_exclusive {
                break;
            }
            if let Some(max) = limit {
                if collected.len() >= max {
                    break;
                }
            }
            let Some(commit) = inner.commits.get(&sha_value) else {
                break;
            };
            collected.push(SpineCommit {
                sha: sha_value.clone(),
                message: commit.message.clone(),
                parents: commit.parents.clone(),
                tree_evidence: commit.tree_evidence.clone(),
            });
            current = commit.parents.first().cloned();
        }
        collected.reverse();
        Ok(collected)
    }

    async fn local_ref_sha(&self, reference: &str) -> PortResult<Option<String>> {
        let key = reference
            .strip_prefix("refs/remotes/")
            .unwrap_or(reference)
            .to_string();
        Ok(self.lock().tracking_refs.get(&key).cloned())
    }

    async fn push_porcelain(&self, remote: &str, branch: &str) -> PortResult<PushCommandResult> {
        let mut inner = self.lock();
        inner.push_calls += 1;
        let behavior = inner
            .push_behaviors
            .pop_front()
            .unwrap_or(PushBehavior::Succeed);
        match behavior {
            PushBehavior::Succeed => {
                inner.successful_pushes += 1;
                let head = inner.head.clone();
                inner
                    .remote_refs
                    .insert(format!("{}/{}", remote, branch), head);
                Ok(PushCommandResult {
                    exit_success: true,
                    porcelain_stdout:
                        "To fake\n \trefs/heads/main:refs/heads/main\tabc..def\nDone\n".to_string(),
                })
            }
            PushBehavior::Reject { porcelain } => Ok(PushCommandResult {
                exit_success: false,
                porcelain_stdout: porcelain,
            }),
            PushBehavior::FailWithoutRefStatus => Ok(PushCommandResult {
                exit_success: false,
                porcelain_stdout: String::new(),
            }),
        }
    }

    async fn ls_remote_sha(&self, remote: &str, branch: &str) -> PortResult<Option<String>> {
        Ok(self
            .lock()
            .remote_refs
            .get(&format!("{}/{}", remote, branch))
            .cloned())
    }
}

#[derive(Default)]
struct FakeVerifier {
    results: Mutex<VecDeque<bool>>,
    calls: Mutex<usize>,
}

impl FakeVerifier {
    fn always_pass() -> Self {
        Self::default()
    }

    fn scripted(results: impl IntoIterator<Item = bool>) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            calls: Mutex::new(0),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl UpstreamVerifier for FakeVerifier {
    async fn verify(&self) -> PortResult<VerificationOutcome> {
        *self.calls.lock().unwrap() += 1;
        let pass = self.results.lock().unwrap().pop_front().unwrap_or(true);
        Ok(if pass {
            VerificationOutcome::passed()
        } else {
            VerificationOutcome::failed("assertion failed")
        })
    }
}

/// Callback applied to the fake repository after each repair attempt.
type RepairAction = Box<dyn Fn(&FakeGit) + Send>;

#[derive(Default)]
struct FakeRepairAgent {
    max_attempts: u32,
    calls: Mutex<Vec<RepairRequest>>,
    on_repair: Mutex<Option<RepairAction>>,
    git: Mutex<Option<std::sync::Weak<FakeGit>>>,
}

impl FakeRepairAgent {
    fn with_attempts(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    fn calls(&self) -> Vec<RepairRequest> {
        self.calls.lock().unwrap().clone()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    fn bind(&self, git: &std::sync::Arc<FakeGit>, action: impl Fn(&FakeGit) + Send + 'static) {
        *self.git.lock().unwrap() = Some(std::sync::Arc::downgrade(git));
        *self.on_repair.lock().unwrap() = Some(Box::new(action));
    }
}

#[async_trait]
impl UpstreamRepairAgent for FakeRepairAgent {
    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    async fn repair(&self, request: &RepairRequest) -> PortResult<RepairAttemptResult> {
        self.calls.lock().unwrap().push(request.clone());
        let git = self.git.lock().unwrap().clone();
        let action = self.on_repair.lock().unwrap();
        if let (Some(weak), Some(action)) = (git, action.as_ref()) {
            if let Some(git) = weak.upgrade() {
                action(&git);
            }
        }
        Ok(RepairAttemptResult {
            command_success: true,
        })
    }
}

#[derive(Default)]
struct RecordingObserver {
    events: Mutex<Vec<UpstreamEvent>>,
}

impl RecordingObserver {
    fn events(&self) -> Vec<UpstreamEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl UpstreamObserver for RecordingObserver {
    async fn observe(&self, event: UpstreamEvent) {
        self.events.lock().unwrap().push(event);
    }
}

struct Harness {
    git: Arc<FakeGit>,
    verifier: Arc<FakeVerifier>,
    repair: Arc<FakeRepairAgent>,
    observer: Arc<RecordingObserver>,
    coordinator: UpstreamCoordinator,
}

fn harness_with(verifier: FakeVerifier, repair: FakeRepairAgent) -> Harness {
    let git = Arc::new(FakeGit::new_linear());
    let verifier = Arc::new(verifier);
    let repair = Arc::new(repair);
    let observer = Arc::new(RecordingObserver::default());
    let coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );
    Harness {
        git,
        verifier,
        repair,
        observer,
        coordinator,
    }
}

fn harness() -> Harness {
    harness_with(
        FakeVerifier::always_pass(),
        FakeRepairAgent::with_attempts(2),
    )
}

// ── Startup validation ─────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_initial_fetch_rejects_missing_remote_branch() {
    let h = harness();
    h.git.lock().remote_refs.clear();
    let result = h.coordinator.validate_initial_fetch().await.unwrap();
    assert_eq!(
        result,
        Err(UpstreamOptionError::RemoteBranchMissing {
            remote: "origin".into(),
            branch: "main".into()
        })
    );
}

#[tokio::test]
async fn upstream_integration_initial_fetch_rejects_unconfigured_remote_and_detached_head() {
    let h = harness();
    h.git.lock().configured_remotes.clear();
    assert_eq!(
        h.coordinator.validate_initial_fetch().await.unwrap(),
        Err(UpstreamOptionError::RemoteNotConfigured("origin".into()))
    );

    let h = harness();
    h.git.lock().current_branch = None;
    assert_eq!(
        h.coordinator.validate_initial_fetch().await.unwrap(),
        Err(UpstreamOptionError::DetachedHead)
    );
}

#[tokio::test]
async fn upstream_integration_initial_fetch_accepts_cumulative_change_merge_history() {
    let h = harness();
    let validation = h
        .coordinator
        .validate_initial_fetch()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(validation.branch, "main");
    assert!(validation.spine.is_publishable());
    assert_eq!(
        validation.spine.integrated_change_ids,
        vec!["a".to_string()]
    );
}

#[tokio::test]
async fn upstream_integration_initial_fetch_rejects_unrelated_local_history() {
    let h = harness();
    {
        let mut inner = h.git.lock();
        let head = inner.head.clone();
        inner.commits.insert(
            sha("hack"),
            FakeCommit {
                message: "hotfix: local only\n".into(),
                parents: vec![head],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("hack");
    }
    let result = h.coordinator.validate_initial_fetch().await.unwrap();
    assert!(matches!(
        result,
        Err(UpstreamOptionError::UnrelatedLocalHistory { .. })
    ));
}

#[tokio::test]
async fn upstream_integration_initial_fetch_accepts_upstream_recovery_commit() {
    let h = harness();
    {
        let mut inner = h.git.lock();
        let head = inner.head.clone();
        let fetched = sha("root");
        inner.commits.insert(
            sha("upmerge"),
            FakeCommit {
                message: format_upstream_merge_message("origin", "main", &fetched),
                parents: vec![head, fetched],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("upmerge");
    }
    let validation = h
        .coordinator
        .validate_initial_fetch()
        .await
        .unwrap()
        .unwrap();
    assert!(validation.spine.is_publishable());
    assert_eq!(validation.spine.upstream_merges.len(), 1);
}

#[tokio::test]
async fn upstream_integration_fetch_failure_leaves_worktree_untouched() {
    let mut h = harness();
    h.git.lock().fetch_error = Some("network unreachable".into());
    let err = h
        .coordinator
        .checkpoint(
            CheckpointTrigger::BeforeFirstDispatch,
            &BaseLaneState::clean(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.operation, "git fetch");
    assert!(h.git.merge_calls().is_empty());
    assert_eq!(h.verifier.calls(), 0);
}

// ── Checkpoint behavior ────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_already_integrated_revision_is_a_noop() {
    let mut h = harness();
    // Remote is at `root`, which is already an ancestor of local HEAD.
    let outcome = h
        .coordinator
        .checkpoint(
            CheckpointTrigger::BeforeFirstDispatch,
            &BaseLaneState::clean(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        outcome,
        UpstreamStepOutcome::NoOp {
            fetched_sha: sha("root")
        }
    );
    assert!(h.git.merge_calls().is_empty());
    assert_eq!(h.verifier.calls(), 0, "no-op must not reverify");
    assert_eq!(h.repair.call_count(), 0, "no-op must not invoke an agent");
}

#[tokio::test]
async fn upstream_integration_remote_ahead_uses_no_ff_merge_with_trailers() {
    let mut h = harness();
    let advanced = h.git.advance_remote("remoteahead", &h.git.head());

    let outcome = h
        .coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    let calls = h.git.merge_calls();
    assert_eq!(calls.len(), 1, "strictly remote-ahead history still merges");
    assert_eq!(calls[0].0, advanced);
    assert_eq!(
        calls[0].1,
        format_upstream_merge_message("origin", "main", &advanced)
    );
    assert!(matches!(outcome, UpstreamStepOutcome::Integrated { .. }));
    assert_eq!(
        h.repair.call_count(),
        0,
        "conflict-free merge starts no agent"
    );
    assert_eq!(h.verifier.calls(), 1, "changed tree runs full verification");
}

#[tokio::test]
async fn upstream_integration_diverged_revision_is_integrated() {
    let mut h = harness();
    let diverged = h.git.advance_remote("diverged", &sha("root"));
    h.coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();
    assert_eq!(h.git.merge_calls()[0].0, diverged);
}

#[tokio::test]
async fn upstream_integration_command_failure_does_not_start_an_agent() {
    let mut h = harness();
    h.git.advance_remote("remoteahead", &h.git.head());
    h.git
        .lock()
        .merge_behaviors
        .push_back(MergeBehavior::CommandFailure);

    let outcome = h
        .coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    assert!(matches!(outcome, UpstreamStepOutcome::Stalled { .. }));
    assert_eq!(h.repair.call_count(), 0);
    assert_eq!(h.verifier.calls(), 0);
}

#[tokio::test]
async fn upstream_integration_dirty_base_defers_before_fetch() {
    let mut h = harness();
    let lane = BaseLaneState {
        base_dirty_reason: Some("uncommitted changes".into()),
        lane_busy_reason: None,
    };
    let outcome = h
        .coordinator
        .checkpoint(CheckpointTrigger::BeforeBaseIntegration, &lane, Some("c1"))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        UpstreamStepOutcome::Deferred {
            reason: "uncommitted changes".into()
        }
    );
    assert_eq!(
        h.git.fetch_calls(),
        0,
        "deferred checkpoint performs no fetch"
    );
    // The completed result is queued, not discarded.
    assert_eq!(h.coordinator.queued_results(), vec!["c1".to_string()]);
}

// ── Textual repair ─────────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_textual_conflict_converges_through_repair() {
    let git = Arc::new(FakeGit::new_linear());
    let advanced = git.advance_remote("remoteahead", &git.head());
    git.lock()
        .merge_behaviors
        .push_back(MergeBehavior::Conflict);

    let verifier = Arc::new(FakeVerifier::always_pass());
    let repair = Arc::new(FakeRepairAgent::with_attempts(2));
    let observer = Arc::new(RecordingObserver::default());

    // The agent finishes the merge; Conflux independently revalidates it.
    let advanced_for_repair = advanced.clone();
    repair.bind(&git, move |git| {
        let mut inner = git.lock();
        let head = inner.head.clone();
        let message = format_upstream_merge_message("origin", "main", &advanced_for_repair);
        inner.commits.insert(
            sha("repaired"),
            FakeCommit {
                message,
                parents: vec![head, advanced_for_repair.clone()],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("repaired");
        inner.merge_state = MergeRepositoryState {
            merge_head_present: false,
            has_unmerged_entries: false,
        };
        inner.working_tree_clean = true;
    });

    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );

    let outcome = coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        UpstreamStepOutcome::Integrated {
            merge_sha: sha("repaired")
        }
    );
    assert_eq!(repair.call_count(), 1);
    assert_eq!(repair.calls()[0].cause, RepairCause::TextualConflict);
    assert_eq!(repair.calls()[0].fetched_sha, advanced);
    assert_eq!(verifier.calls(), 1);
}

#[tokio::test]
async fn upstream_integration_textual_repair_exhaustion_stalls() {
    let mut h = harness();
    h.git.advance_remote("remoteahead", &h.git.head());
    h.git
        .lock()
        .merge_behaviors
        .push_back(MergeBehavior::Conflict);

    let outcome = h
        .coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    assert!(matches!(outcome, UpstreamStepOutcome::Stalled { .. }));
    assert_eq!(h.repair.call_count(), 2, "retry budget is Conflux-owned");
    assert_eq!(h.verifier.calls(), 0, "unconverged merge never reverifies");
}

// ── Semantic repair and verification routing ───────────────────────────────

#[tokio::test]
async fn upstream_integration_conflict_free_merge_with_semantic_failure_repairs_then_reruns() {
    let git = Arc::new(FakeGit::new_linear());
    git.advance_remote("remoteahead", &git.head());
    // Fail once, then pass after repair.
    let verifier = Arc::new(FakeVerifier::scripted([false, true]));
    let repair = Arc::new(FakeRepairAgent::with_attempts(2));
    let observer = Arc::new(RecordingObserver::default());
    repair.bind(&git, |git| {
        // Forward-only repair commit on top of the merge.
        let mut inner = git.lock();
        let head = inner.head.clone();
        inner.next_sha += 1;
        let new = sha(&format!("fix{}", inner.next_sha));
        inner.commits.insert(
            new.clone(),
            FakeCommit {
                message: "fix: repair semantic breakage\n".into(),
                parents: vec![head],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = new;
        inner.working_tree_clean = true;
    });

    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );

    let outcome = coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    assert!(matches!(outcome, UpstreamStepOutcome::Integrated { .. }));
    assert_eq!(repair.call_count(), 1);
    assert_eq!(repair.calls()[0].cause, RepairCause::SemanticVerification);
    assert_eq!(repair.calls()[0].verify_command, "cargo test");
    assert!(!repair.calls()[0].verify_output_tail.is_empty());
    assert_eq!(
        verifier.calls(),
        2,
        "every repair attempt is followed by a mandatory rerun"
    );
}

#[tokio::test]
async fn upstream_integration_semantic_repair_rejects_history_rewrite() {
    let git = Arc::new(FakeGit::new_linear());
    git.advance_remote("remoteahead", &git.head());
    let verifier = Arc::new(FakeVerifier::scripted([false, true]));
    let repair = Arc::new(FakeRepairAgent::with_attempts(2));
    let observer = Arc::new(RecordingObserver::default());
    repair.bind(&git, |git| {
        // Simulate a reset: HEAD moves backwards off the merge.
        let mut inner = git.lock();
        inner.head = sha("root");
    });

    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );

    let outcome = coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    match outcome {
        UpstreamStepOutcome::Stalled { reason } => {
            assert!(reason.contains("forward-only"), "reason: {}", reason)
        }
        other => panic!("expected stall, got {:?}", other),
    }
    assert_eq!(
        verifier.calls(),
        1,
        "a rewritten history is never granted a verification rerun"
    );
}

#[tokio::test]
async fn upstream_integration_semantic_repair_exhaustion_blocks_base_lane() {
    let git = Arc::new(FakeGit::new_linear());
    git.advance_remote("remoteahead", &git.head());
    let verifier = Arc::new(FakeVerifier::scripted([false, false, false, false]));
    let repair = Arc::new(FakeRepairAgent::with_attempts(2));
    let observer = Arc::new(RecordingObserver::default());
    repair.bind(&git, |git| {
        let mut inner = git.lock();
        let head = inner.head.clone();
        inner.next_sha += 1;
        let new = sha(&format!("fix{}", inner.next_sha));
        inner.commits.insert(
            new.clone(),
            FakeCommit {
                message: "fix attempt\n".into(),
                parents: vec![head],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = new;
    });

    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );

    let outcome = coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();
    assert!(matches!(outcome, UpstreamStepOutcome::Stalled { .. }));
    assert_eq!(repair.call_count(), 2);
    assert_eq!(verifier.calls(), 3);
}

#[tokio::test]
async fn upstream_integration_completed_result_runs_full_verification() {
    let mut h = harness();
    let outcome = h.coordinator.verify_base_result("change-a").await.unwrap();
    assert!(matches!(outcome, UpstreamStepOutcome::Integrated { .. }));
    assert_eq!(h.verifier.calls(), 1);
    assert!(h.git.merge_calls().is_empty());
}

#[tokio::test]
async fn upstream_integration_completed_result_verification_failure_blocks_dispatch() {
    let mut h = harness_with(
        FakeVerifier::scripted([false]),
        FakeRepairAgent::with_attempts(0),
    );
    let outcome = h.coordinator.verify_base_result("change-a").await.unwrap();
    match outcome {
        UpstreamStepOutcome::Stalled { reason } => assert!(reason.contains("change-a")),
        other => panic!("expected stall, got {:?}", other),
    }
    assert_eq!(h.repair.call_count(), 0);
}

// ── Recovery ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_scan_finds_unpushed_trailer_identified_merge() {
    let git = FakeGit::new_linear();
    {
        let mut inner = git.lock();
        let head = inner.head.clone();
        let fetched = sha("root");
        inner.commits.insert(
            sha("upmerge"),
            FakeCommit {
                message: format_upstream_merge_message("origin", "main", &fetched),
                parents: vec![head, fetched],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("upmerge");
        // Remote-tracking ref still points at the old base: the merge is unpushed.
        inner
            .tracking_refs
            .insert("origin/main".into(), sha("root"));
    }

    let evidence = scan_unpushed_upstream_merges(&git).await.unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].merge_sha, sha("upmerge"));
    assert_eq!(evidence[0].trailers.remote, "origin");

    let refusal = upstream_recovery_refusal(&evidence).unwrap();
    let message = refusal.to_string();
    assert!(message.contains("--integrate-upstream=origin"));
    assert!(message.contains("--upstream-verify-command"));
}

#[tokio::test]
async fn upstream_integration_scan_ignores_published_and_untrailered_merges() {
    let git = FakeGit::new_linear();
    {
        let mut inner = git.lock();
        let head = inner.head.clone();
        let fetched = sha("root");
        inner.commits.insert(
            sha("upmerge"),
            FakeCommit {
                message: format_upstream_merge_message("origin", "main", &fetched),
                parents: vec![head, fetched],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("upmerge");
        // Remote already contains the merge.
        inner
            .tracking_refs
            .insert("origin/main".into(), sha("upmerge"));
    }
    assert!(scan_unpushed_upstream_merges(&git)
        .await
        .unwrap()
        .is_empty());

    // A `Merge change:` commit is not upstream recovery evidence.
    let plain = FakeGit::new_linear();
    plain
        .lock()
        .tracking_refs
        .insert("origin/main".into(), sha("root"));
    assert!(scan_unpushed_upstream_merges(&plain)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn upstream_integration_restart_reruns_verification_for_unpushed_merge() {
    // Restart with the same repository state: the trailer-identified merge is
    // reachable and unpushed, so finalization must run the newly supplied
    // verification command before pushing. Nothing durable claims completion.
    let git = Arc::new(FakeGit::new_linear());
    {
        let mut inner = git.lock();
        let head = inner.head.clone();
        let fetched = sha("root");
        inner.commits.insert(
            sha("upmerge"),
            FakeCommit {
                message: format_upstream_merge_message("origin", "main", &fetched),
                parents: vec![head, fetched],
                tree_evidence: CommitTreeEvidence::default(),
            },
        );
        inner.head = sha("upmerge");
    }
    let verifier = Arc::new(FakeVerifier::always_pass());
    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        Arc::new(FakeRepairAgent::with_attempts(1)),
        Arc::new(RecordingObserver::default()),
    );

    let outcome = coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        FinalizeOutcome::Completed {
            pushed_head: sha("upmerge")
        }
    );
    assert!(verifier.calls() >= 1);
    assert_eq!(git.successful_pushes(), 1);
}

// ── Finalization ───────────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_blocked_and_cancelled_outcomes_never_push() {
    for outcome in [
        SchedulerOutcome::BlockedOrStalled,
        SchedulerOutcome::Cancelled,
    ] {
        let mut h = harness();
        let result = h.coordinator.finalize(outcome).await.unwrap();
        assert!(matches!(result, FinalizeOutcome::Skipped { .. }));
        assert_eq!(h.git.push_calls(), 0);
        assert_eq!(h.verifier.calls(), 0);
        assert_eq!(h.git.fetch_calls(), 0);
    }
}

#[tokio::test]
async fn upstream_integration_successful_drain_pushes_once_and_confirms() {
    let mut h = harness();
    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        FinalizeOutcome::Completed {
            pushed_head: sha("local")
        }
    );
    assert_eq!(h.git.successful_pushes(), 1);
    assert_eq!(h.coordinator.pushed_head(), Some(sha("local").as_str()));
    assert!(h
        .observer
        .events()
        .iter()
        .any(|e| matches!(e, UpstreamEvent::PushConfirmed { .. })));

    // A second finalization never retries after confirmed success.
    let repeat = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(repeat, FinalizeOutcome::Completed { .. }));
    assert_eq!(h.git.successful_pushes(), 1);
}

#[tokio::test]
async fn upstream_integration_zero_change_fresh_run_manufactures_no_history() {
    let mut h = harness();
    // Remote already contains local HEAD: nothing recognized is unpushed.
    h.git.set_remote_ref(&sha("local"));

    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert_eq!(outcome, FinalizeOutcome::NoWork);
    assert!(h.git.merge_calls().is_empty());
    assert_eq!(h.verifier.calls(), 0);
    assert_eq!(h.git.push_calls(), 0);
}

#[tokio::test]
async fn upstream_integration_remote_only_advance_creates_no_synthetic_merge() {
    let mut h = harness();
    // Remote advanced past local HEAD and contains it: only observation changes.
    let advanced = h.git.advance_remote("ahead", &sha("local"));
    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert_eq!(outcome, FinalizeOutcome::NoWork);
    assert!(h.git.merge_calls().is_empty());
    assert_eq!(h.git.push_calls(), 0);
    assert_ne!(advanced, sha("local"));
}

#[tokio::test]
async fn upstream_integration_noop_run_still_verifies_before_push() {
    let mut h = harness();
    // Remote at `root` is already integrated, but local work must be published.
    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Completed { .. }));
    assert!(h.git.merge_calls().is_empty(), "no-op performs no merge");
    assert_eq!(
        h.verifier.calls(),
        1,
        "final cumulative HEAD is always verified before push"
    );
}

#[tokio::test]
async fn upstream_integration_remote_advance_before_push_returns_to_integration() {
    let mut h = harness();
    // Advance the remote off local history so the pre-push probe must integrate.
    let advanced = h.git.advance_remote("racer", &sha("root"));
    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Completed { .. }));
    let calls = h.git.merge_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, advanced);
    assert_eq!(h.git.successful_pushes(), 1);
}

#[tokio::test]
async fn upstream_integration_race_rejection_returns_to_checkpoint_then_succeeds() {
    let mut h = harness();
    h.git.lock().push_behaviors.push_back(PushBehavior::Reject {
        porcelain:
            "To fake\n!\trefs/heads/main:refs/heads/main\t[rejected]\t(non-fast-forward)\nDone\n"
                .to_string(),
    });

    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Completed { .. }));
    assert_eq!(
        h.git.push_calls(),
        2,
        "one failed race attempt, then success"
    );
    assert_eq!(h.git.successful_pushes(), 1, "at most one successful push");
    assert_eq!(h.repair.call_count(), 0, "a race never invokes an agent");
}

#[tokio::test]
async fn upstream_integration_non_repairable_push_failure_stalls_without_agent() {
    let mut h = harness();
    for _ in 0..MAX_FINALIZE_ATTEMPTS {
        h.git
            .lock()
            .push_behaviors
            .push_back(PushBehavior::FailWithoutRefStatus);
    }

    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Stalled { .. }));
    assert_eq!(h.git.successful_pushes(), 0);
    assert_eq!(h.repair.call_count(), 0);
    assert!(h
        .observer
        .events()
        .iter()
        .any(|e| matches!(e, UpstreamEvent::PushFailed { .. })));
}

#[tokio::test]
async fn upstream_integration_repairable_push_failure_repairs_then_conflux_pushes() {
    let git = Arc::new(FakeGit::new_linear());
    git.lock()
        .push_behaviors
        .push_back(PushBehavior::FailWithoutRefStatus);
    git.lock().porcelain_v2 = "1 .M N... 100644 100644 100644 aaa bbb src/lib.rs\n".to_string();

    let verifier = Arc::new(FakeVerifier::always_pass());
    let repair = Arc::new(FakeRepairAgent::with_attempts(2));
    let observer = Arc::new(RecordingObserver::default());
    repair.bind(&git, |git| {
        // The agent cleans the worktree but must never push.
        git.lock().porcelain_v2 = String::new();
    });

    let mut coordinator = UpstreamCoordinator::new(
        UpstreamIntegrationConfig::new("origin", "cargo test"),
        "main",
        git.clone(),
        verifier.clone(),
        repair.clone(),
        observer.clone(),
    );

    let outcome = coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Completed { .. }));
    assert_eq!(repair.call_count(), 1);
    assert_eq!(repair.calls()[0].cause, RepairCause::PushRepository);
    assert_eq!(
        git.successful_pushes(),
        1,
        "Conflux performs the retry itself"
    );
    assert!(
        verifier.calls() >= 2,
        "post-repair convergence reruns the complete command"
    );
}

#[tokio::test]
async fn upstream_integration_final_verification_failure_never_pushes() {
    let mut h = harness_with(
        FakeVerifier::scripted([false]),
        FakeRepairAgent::with_attempts(0),
    );
    let outcome = h
        .coordinator
        .finalize(SchedulerOutcome::DrainedSuccessfully)
        .await
        .unwrap();
    assert!(matches!(outcome, FinalizeOutcome::Stalled { .. }));
    assert_eq!(h.git.push_calls(), 0);
}

// ── Observability ──────────────────────────────────────────────────────────

#[tokio::test]
async fn upstream_integration_reports_lifecycle_without_becoming_routing_authority() {
    let mut h = harness();
    h.git.advance_remote("remoteahead", &h.git.head());
    h.coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();

    let events = h.observer.events();
    assert!(events
        .iter()
        .any(|e| matches!(e, UpstreamEvent::CheckpointStarted { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        UpstreamEvent::FetchCompleted { remote, branch, .. } if remote == "origin" && branch == "main"
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, UpstreamEvent::IntegrationCompleted { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, UpstreamEvent::Reverifying { .. })));

    // Repeating the same checkpoint with an unchanged repository is a no-op,
    // proving the emitted events did not establish or change routing.
    let repeat = h
        .coordinator
        .checkpoint(CheckpointTrigger::AfterDrain, &BaseLaneState::clean(), None)
        .await
        .unwrap();
    assert!(matches!(repeat, UpstreamStepOutcome::NoOp { .. }));
}
