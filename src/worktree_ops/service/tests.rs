//! Unit tests for the shared worktree operation service.
//!
//! Every boundary is a fake: no repository, no process, no filesystem state.
//! What is under test is the decision logic and the ordering guarantees the two
//! frontends both depend on.

use std::sync::{Arc, Mutex};

use super::*;

/// Recorded backend calls, in the order the service made them.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Observe,
    BaseHead,
    Create {
        branch: String,
    },
    Remove {
        skip_teardown: bool,
    },
    DeleteBranch {
        branch: String,
    },
    Merge {
        branch: String,
        policy: ConflictPolicy,
    },
    OnMerged {
        change_id: String,
    },
    Eligibility {
        change_id: String,
    },
}

struct FakeBackend {
    calls: Mutex<Vec<Call>>,
    observations: Mutex<Vec<Vec<WorktreeFacts>>>,
    merge: Mutex<WorktreeOpResult<MergeAttempt>>,
    on_merged: Mutex<WorktreeOpResult<()>>,
    remove: Mutex<WorktreeOpResult<()>>,
    eligible: Mutex<WorktreeOpResult<()>>,
}

impl FakeBackend {
    /// Every `observe()` returns the same set.
    fn stable(facts: Vec<WorktreeFacts>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            observations: Mutex::new(vec![facts]),
            merge: Mutex::new(Ok(MergeAttempt::Merged)),
            on_merged: Mutex::new(Ok(())),
            remove: Mutex::new(Ok(())),
            eligible: Mutex::new(Ok(())),
        })
    }

    /// Successive `observe()` calls walk the given script, then repeat the last.
    fn scripted(script: Vec<Vec<WorktreeFacts>>) -> Arc<Self> {
        let backend = Self::stable(Vec::new());
        *backend.observations.lock().unwrap() = script;
        backend
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

#[async_trait]
impl WorktreeBackend for FakeBackend {
    async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
        self.record(Call::Observe);
        let mut script = self.observations.lock().unwrap();
        if script.len() > 1 {
            Ok(script.remove(0))
        } else {
            Ok(script.first().cloned().unwrap_or_default())
        }
    }

    async fn base_head(&self) -> WorktreeOpResult<String> {
        self.record(Call::BaseHead);
        Ok("basehead0000".to_string())
    }

    async fn create(&self, _path: &Path, branch: &str, _base_commit: &str) -> WorktreeOpResult<()> {
        self.record(Call::Create {
            branch: branch.to_string(),
        });
        Ok(())
    }

    async fn remove(&self, _path: &Path, skip_teardown: bool) -> WorktreeOpResult<()> {
        self.record(Call::Remove { skip_teardown });
        self.remove.lock().unwrap().clone()
    }

    async fn delete_branch(&self, branch: &str) -> WorktreeOpResult<()> {
        self.record(Call::DeleteBranch {
            branch: branch.to_string(),
        });
        Ok(())
    }

    async fn merge_into_base(
        &self,
        branch: &str,
        policy: ConflictPolicy,
    ) -> WorktreeOpResult<MergeAttempt> {
        self.record(Call::Merge {
            branch: branch.to_string(),
            policy,
        });
        self.merge.lock().unwrap().clone()
    }

    async fn run_on_merged(&self, change_id: &str, _worktree_path: &Path) -> WorktreeOpResult<()> {
        self.record(Call::OnMerged {
            change_id: change_id.to_string(),
        });
        self.on_merged.lock().unwrap().clone()
    }

    async fn change_is_eligible(&self, change_id: &str) -> WorktreeOpResult<()> {
        self.record(Call::Eligibility {
            change_id: change_id.to_string(),
        });
        self.eligible.lock().unwrap().clone()
    }
}

#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<WorktreeOperationEvent>>,
}

impl RecordingSink {
    fn events(&self) -> Vec<WorktreeOperationEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl WorktreeEventSink for RecordingSink {
    async fn emit(&self, event: WorktreeOperationEvent) {
        self.events.lock().unwrap().push(event);
    }
}

fn managed(path: &str, branch: &str) -> WorktreeFacts {
    let mut facts = WorktreeFacts::new(path, branch);
    facts.identity = format!("gitdir: {path}/.git");
    facts.head = "cafebabe".to_string();
    facts
}

fn service(backend: Arc<FakeBackend>) -> (WorktreeService, Arc<RecordingSink>) {
    let sink = Arc::new(RecordingSink::default());
    (
        WorktreeService::new(backend, sink.clone(), PathBuf::from("/workspaces")),
        sink,
    )
}

// ── Delete eligibility ───────────────────────────────────────────────────────

#[test]
fn remote_worktree_unknown_dirty_blocks_fail_closed_delete() {
    let mut facts = managed("/w/a", "a");
    facts.dirty = DirtyState::Unknown;

    let refusal =
        classify_delete_eligibility(&facts, DeleteOptions::fail_closed()).expect_err("must refuse");
    assert!(matches!(refusal, WorktreeOpError::DirtyUnknown(_)));
}

#[test]
fn remote_worktree_unknown_dirty_is_allowed_only_by_explicit_local_policy() {
    let mut facts = managed("/w/a", "a");
    facts.dirty = DirtyState::Unknown;

    assert!(classify_delete_eligibility(&facts, DeleteOptions::local(false)).is_ok());
}

#[test]
fn remote_worktree_dirty_and_main_and_ahead_all_block_delete() {
    let mut dirty = managed("/w/a", "a");
    dirty.dirty = DirtyState::Dirty;
    assert!(matches!(
        classify_delete_eligibility(&dirty, DeleteOptions::fail_closed()),
        Err(WorktreeOpError::Dirty(_))
    ));

    let mut main = managed("/repo", "main");
    main.is_main = true;
    assert!(matches!(
        classify_delete_eligibility(&main, DeleteOptions::fail_closed()),
        Err(WorktreeOpError::Ineligible(_))
    ));

    let mut ahead = managed("/w/a", "a");
    ahead.has_commits_ahead = true;
    assert!(matches!(
        classify_delete_eligibility(&ahead, DeleteOptions::fail_closed()),
        Err(WorktreeOpError::Ineligible(_))
    ));
}

#[test]
fn remote_worktree_unresolved_base_merge_makes_the_root_busy() {
    let mut facts = managed("/w/a", "a");
    facts.base_merge_in_progress = true;

    assert!(matches!(
        classify_delete_eligibility(&facts, DeleteOptions::fail_closed()),
        Err(WorktreeOpError::RootBusy(_))
    ));
    assert!(matches!(
        classify_merge_eligibility(&facts, ConflictPolicy::PreserveConflict),
        Err(WorktreeOpError::RootBusy(_))
    ));
}

// ── Merge eligibility ────────────────────────────────────────────────────────

#[test]
fn remote_worktree_preserving_policy_attempts_a_predicted_conflict() {
    let mut facts = managed("/w/a", "a");
    facts.has_commits_ahead = true;
    facts.conflict_files = vec!["src/main.rs".to_string()];

    // The aborting frontend refuses up front; the preserving one must run the
    // merge so the conflict evidence it promises actually exists.
    assert!(matches!(
        classify_merge_eligibility(&facts, ConflictPolicy::AbortOnConflict),
        Err(WorktreeOpError::Ineligible(_))
    ));
    assert!(classify_merge_eligibility(&facts, ConflictPolicy::PreserveConflict).is_ok());
}

#[test]
fn remote_worktree_merge_requires_a_branch_and_commits_ahead() {
    let mut detached = managed("/w/a", "");
    detached.is_detached = true;
    detached.has_commits_ahead = true;
    assert!(matches!(
        classify_merge_eligibility(&detached, ConflictPolicy::PreserveConflict),
        Err(WorktreeOpError::Ineligible(_))
    ));

    let behind = managed("/w/a", "a");
    assert!(matches!(
        classify_merge_eligibility(&behind, ConflictPolicy::PreserveConflict),
        Err(WorktreeOpError::Ineligible(_))
    ));
}

// ── Server-derived naming ────────────────────────────────────────────────────

#[test]
fn remote_worktree_branch_and_path_are_derived_from_the_change_id_alone() {
    assert_eq!(branch_name_for_change("feat/one two"), "feat-one-two");
    assert_eq!(
        worktree_path_for_change(Path::new("/workspaces"), "feat/one two"),
        PathBuf::from("/workspaces/feat-one-two")
    );
}

// ── Create ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn remote_worktree_create_uses_current_base_head_and_returns_fresh_facts() {
    let created = managed("/workspaces/c1", "c1");
    let backend = FakeBackend::scripted(vec![vec![], vec![created.clone()]]);
    let (service, sink) = service(backend.clone());

    let facts = service.create_change_worktree("c1").await.expect("created");
    assert_eq!(facts, created);

    let calls = backend.calls();
    assert_eq!(
        calls,
        vec![
            Call::Eligibility {
                change_id: "c1".to_string()
            },
            Call::Observe,
            Call::BaseHead,
            Call::Create {
                branch: "c1".to_string()
            },
            Call::Observe,
        ],
        "eligibility and existence must be checked before any base read or mutation"
    );
    assert!(sink.events().contains(&WorktreeOperationEvent::Created {
        branch: "c1".to_string()
    }));
}

#[tokio::test]
async fn remote_worktree_create_conflicts_instead_of_succeeding_as_a_no_op() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, sink) = service(backend.clone());

    let refusal = service
        .create_change_worktree("c1")
        .await
        .expect_err("existing worktree must conflict");
    assert!(matches!(refusal, WorktreeOpError::Exists(_)));
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Create { .. } | Call::BaseHead)),
        "a conflicting create must not read base or mutate Git"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn remote_worktree_create_refuses_an_unmanaged_change() {
    let backend = FakeBackend::stable(vec![]);
    *backend.eligible.lock().unwrap() = Err(WorktreeOpError::NotFound("archived".to_string()));
    let (service, _sink) = service(backend.clone());

    let refusal = service
        .create_change_worktree("gone")
        .await
        .expect_err("must refuse");
    assert!(matches!(refusal, WorktreeOpError::NotFound(_)));
    assert_eq!(
        backend.calls(),
        vec![Call::Eligibility {
            change_id: "gone".to_string()
        }]
    );
}

// ── Delete ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn remote_worktree_delete_runs_mandatory_teardown_and_deletes_the_branch() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, sink) = service(backend.clone());

    let outcome = service
        .delete_worktree(Path::new("/workspaces/c1"), DeleteOptions::fail_closed())
        .await
        .expect("deleted");
    assert_eq!(outcome.branch, "c1");

    assert!(backend.calls().contains(&Call::Remove {
        skip_teardown: false
    }));
    assert!(backend.calls().contains(&Call::DeleteBranch {
        branch: "c1".to_string()
    }));
    assert!(sink.events().contains(&WorktreeOperationEvent::Deleted {
        branch: "c1".to_string()
    }));
}

#[tokio::test]
async fn remote_worktree_failed_teardown_retains_the_resource() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    *backend.remove.lock().unwrap() =
        Err(WorktreeOpError::Internal("teardown exited 1".to_string()));
    let (service, sink) = service(backend.clone());

    let failure = service
        .delete_worktree(Path::new("/workspaces/c1"), DeleteOptions::fail_closed())
        .await
        .expect_err("must fail");
    assert!(matches!(failure, WorktreeOpError::Internal(_)));
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::DeleteBranch { .. })),
        "a failed teardown must not proceed to branch deletion"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn remote_worktree_delete_of_an_unobserved_path_is_not_found() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, _sink) = service(backend);

    let failure = service
        .delete_worktree(Path::new("/workspaces/gone"), DeleteOptions::fail_closed())
        .await
        .expect_err("must fail");
    assert!(matches!(failure, WorktreeOpError::NotFound(_)));
}

// ── Merge ────────────────────────────────────────────────────────────────────

fn mergeable() -> WorktreeFacts {
    let mut facts = managed("/workspaces/c1", "c1");
    facts.has_commits_ahead = true;
    facts
}

#[tokio::test]
async fn remote_worktree_successful_merge_runs_on_merged_exactly_once() {
    let backend = FakeBackend::stable(vec![mergeable()]);
    let (service, sink) = service(backend.clone());

    service
        .merge_worktree(
            Path::new("/workspaces/c1"),
            ConflictPolicy::PreserveConflict,
        )
        .await
        .expect("merged");

    let hook_calls = backend
        .calls()
        .into_iter()
        .filter(|call| matches!(call, Call::OnMerged { .. }))
        .count();
    assert_eq!(hook_calls, 1);
    assert_eq!(
        sink.events(),
        vec![
            WorktreeOperationEvent::MergeStarted {
                branch: "c1".to_string()
            },
            WorktreeOperationEvent::MergeCompleted {
                branch: "c1".to_string()
            },
            WorktreeOperationEvent::Refreshed,
        ]
    );
}

#[tokio::test]
async fn remote_worktree_merge_conflict_preserves_state_and_skips_the_hook() {
    let backend = FakeBackend::stable(vec![mergeable()]);
    *backend.merge.lock().unwrap() = Ok(MergeAttempt::Conflict {
        files: vec!["src/main.rs".to_string(), "README.md".to_string()],
    });
    let (service, sink) = service(backend.clone());

    let failure = service
        .merge_worktree(
            Path::new("/workspaces/c1"),
            ConflictPolicy::PreserveConflict,
        )
        .await
        .expect_err("conflict must fail the command");

    match &failure {
        WorktreeOpError::MergeConflict { files, recovery } => {
            assert_eq!(files, &["src/main.rs".to_string(), "README.md".to_string()]);
            assert_eq!(*recovery, RECOVERY_LOCAL_OR_TUI);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(failure.to_string().contains("local_or_tui_required"));
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::OnMerged { .. })),
        "a conflicted merge must not run on_merged"
    );
    assert!(sink
        .events()
        .iter()
        .any(|event| matches!(event, WorktreeOperationEvent::MergeFailed { .. })));
}

#[tokio::test]
async fn remote_worktree_merge_passes_the_callers_conflict_policy_to_the_backend() {
    for policy in [
        ConflictPolicy::AbortOnConflict,
        ConflictPolicy::PreserveConflict,
    ] {
        let backend = FakeBackend::stable(vec![mergeable()]);
        let (service, _sink) = service(backend.clone());
        service
            .merge_worktree(Path::new("/workspaces/c1"), policy)
            .await
            .expect("merged");
        assert!(backend.calls().contains(&Call::Merge {
            branch: "c1".to_string(),
            policy
        }));
    }
}

#[tokio::test]
async fn remote_worktree_on_merged_failure_blocks_the_merged_transition() {
    let backend = FakeBackend::stable(vec![mergeable()]);
    *backend.on_merged.lock().unwrap() = Err(WorktreeOpError::Internal("hook exit 2".to_string()));
    let (service, sink) = service(backend);

    let failure = service
        .merge_worktree(
            Path::new("/workspaces/c1"),
            ConflictPolicy::PreserveConflict,
        )
        .await
        .expect_err("must fail");
    assert!(failure.to_string().contains("on_merged"));
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event, WorktreeOperationEvent::MergeCompleted { .. })),
        "completion must not be announced when the hook blocked the transition"
    );
}

// ── Root guard ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn remote_worktree_concurrent_mutation_reports_root_busy() {
    let backend = FakeBackend::stable(vec![mergeable()]);
    let (service, _sink) = service(backend);

    // Hold the guard exactly as an in-flight operation would.
    let held = service
        .acquire_root()
        .expect("first caller takes the guard");
    let refusal = service
        .delete_worktree(Path::new("/workspaces/c1"), DeleteOptions::fail_closed())
        .await
        .expect_err("second caller must be refused");
    assert!(matches!(refusal, WorktreeOpError::RootBusy(_)));
    drop(held);

    assert!(service
        .merge_worktree(
            Path::new("/workspaces/c1"),
            ConflictPolicy::PreserveConflict
        )
        .await
        .is_ok());
}
