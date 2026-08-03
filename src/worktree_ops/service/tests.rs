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
    Teardown,
    RemoveWorktree,
    BranchRef {
        branch: String,
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
    teardown: Mutex<WorktreeOpResult<()>>,
    remove: Mutex<WorktreeOpResult<()>>,
    eligible: Mutex<WorktreeOpResult<()>>,
    /// Ref each branch resolves to, or the failure reading it.
    ///
    /// `None` for a branch means "no entry recorded"; the default answer is the
    /// observed HEAD, which is the ordinary post-removal case.
    branch_refs: Mutex<std::collections::HashMap<String, WorktreeOpResult<Option<String>>>>,
    delete_branch: Mutex<WorktreeOpResult<()>>,
}

impl FakeBackend {
    /// Every `observe()` returns the same set.
    fn stable(facts: Vec<WorktreeFacts>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            observations: Mutex::new(vec![facts]),
            merge: Mutex::new(Ok(MergeAttempt::Merged)),
            on_merged: Mutex::new(Ok(())),
            teardown: Mutex::new(Ok(())),
            remove: Mutex::new(Ok(())),
            eligible: Mutex::new(Ok(())),
            branch_refs: Mutex::new(std::collections::HashMap::new()),
            delete_branch: Mutex::new(Ok(())),
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

    fn set_branch_ref(&self, branch: &str, answer: WorktreeOpResult<Option<String>>) {
        self.branch_refs
            .lock()
            .unwrap()
            .insert(branch.to_string(), answer);
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

    async fn teardown(&self, _path: &Path) -> WorktreeOpResult<()> {
        self.record(Call::Teardown);
        self.teardown.lock().unwrap().clone()
    }

    async fn remove_worktree(&self, _path: &Path) -> WorktreeOpResult<()> {
        self.record(Call::RemoveWorktree);
        self.remove.lock().unwrap().clone()
    }

    async fn branch_ref(&self, branch: &str) -> WorktreeOpResult<Option<String>> {
        self.record(Call::BranchRef {
            branch: branch.to_string(),
        });
        match self.branch_refs.lock().unwrap().get(branch) {
            Some(answer) => answer.clone(),
            None => Ok(Some("cafebabe".to_string())),
        }
    }

    async fn delete_branch_if_merged(&self, branch: &str) -> WorktreeOpResult<()> {
        self.record(Call::DeleteBranch {
            branch: branch.to_string(),
        });
        self.delete_branch.lock().unwrap().clone()
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
fn dirty_discard_never_waives_an_unobservable_dirty_state() {
    let mut facts = managed("/w/a", "a");
    facts.dirty = DirtyState::Unknown;

    // "I could not look" is not "there is nothing there". No policy — remote,
    // ordinary local, or explicitly destructive local — turns it into one.
    for options in [
        DeleteOptions::fail_closed(),
        DeleteOptions::local(false),
        DeleteOptions::local(true),
        DeleteOptions::local_discarding_dirty(false),
        DeleteOptions::local_discarding_dirty(true),
    ] {
        assert!(
            matches!(
                classify_delete_eligibility(&facts, options),
                Err(WorktreeOpError::DirtyUnknown(_))
            ),
            "{options:?} must refuse an unobservable dirty state"
        );
    }
}

#[test]
fn dirty_discard_is_the_only_policy_that_deletes_known_dirty_content() {
    let mut facts = managed("/w/a", "a");
    facts.dirty = DirtyState::Dirty;

    for options in [
        DeleteOptions::fail_closed(),
        DeleteOptions::local(false),
        DeleteOptions::local(true),
    ] {
        match classify_delete_eligibility(&facts, options) {
            Err(WorktreeOpError::Dirty { target, .. }) => {
                // The refusal carries the observation a local escalation is
                // allowed to confirm against, so the destructive modal never has
                // to trust a projection it was already holding.
                assert_eq!(target.path, PathBuf::from("/w/a"));
                assert_eq!(target.branch, "a");
                assert_eq!(target.head, "cafebabe");
                assert_eq!(target.identity, "gitdir: /w/a/.git");
            }
            other => panic!("{options:?} must refuse a known dirty worktree, got {other:?}"),
        }
    }

    assert!(
        classify_delete_eligibility(&facts, DeleteOptions::local_discarding_dirty(false)).is_ok()
    );
    assert!(
        classify_delete_eligibility(&facts, DeleteOptions::local_discarding_dirty(true)).is_ok()
    );
}

#[test]
fn dirty_discard_leaves_ignored_only_content_classified_as_clean() {
    // Status is observed without ignored-file enumeration, so a worktree whose
    // only extra content is generated stays `Clean` and deletes through the
    // ordinary path — no destructive confirmation, no escalation.
    let facts = managed("/w/a", "a");
    assert_eq!(facts.dirty, DirtyState::Clean);
    assert!(classify_delete_eligibility(&facts, DeleteOptions::local(false)).is_ok());
    assert!(classify_delete_eligibility(&facts, DeleteOptions::fail_closed()).is_ok());
}

#[test]
fn dirty_discard_never_waives_main_status_or_commits_ahead() {
    let mut main = managed("/repo", "main");
    main.is_main = true;
    main.dirty = DirtyState::Dirty;
    assert!(matches!(
        classify_delete_eligibility(&main, DeleteOptions::local_discarding_dirty(false)),
        Err(WorktreeOpError::Ineligible(_))
    ));

    let mut ahead = managed("/w/a", "a");
    ahead.has_commits_ahead = SafetyFact::Yes;
    ahead.dirty = DirtyState::Dirty;
    for options in [
        DeleteOptions::fail_closed(),
        DeleteOptions::local(false),
        DeleteOptions::local_discarding_dirty(false),
    ] {
        // Ahead is reported *instead of* dirty on purpose: `Dirty` is the one
        // refusal a frontend escalates, so a worktree carrying unmerged commits
        // must never produce it and must never reach the destructive modal.
        assert!(
            matches!(
                classify_delete_eligibility(&ahead, options),
                Err(WorktreeOpError::Ineligible(_))
            ),
            "{options:?} must refuse commits ahead without offering a dirty escalation"
        );
    }
}

#[test]
fn dirty_discard_never_waives_an_unobservable_ahead_or_merge_state() {
    let mut ahead_unknown = managed("/w/a", "a");
    ahead_unknown.has_commits_ahead = SafetyFact::Unknown;
    ahead_unknown.dirty = DirtyState::Dirty;

    let mut merge_unknown = managed("/w/a", "a");
    merge_unknown.base_merge_in_progress = SafetyFact::Unknown;
    merge_unknown.dirty = DirtyState::Dirty;

    for (name, facts) in [
        ("commits-ahead", ahead_unknown),
        ("base-merge", merge_unknown),
    ] {
        for options in [
            DeleteOptions::fail_closed(),
            DeleteOptions::local(false),
            DeleteOptions::local_discarding_dirty(false),
        ] {
            let refusal = classify_delete_eligibility(&facts, options)
                .expect_err(&format!("{name} must refuse under {options:?}"));
            assert!(
                matches!(refusal, WorktreeOpError::Ineligible(_)),
                "{name}: an unobservable safety fact must refuse, not escalate: {refusal:?}"
            );
            assert!(
                refusal.to_string().contains("could not be determined"),
                "{name}: the refusal must say why: {refusal}"
            );
        }
    }
}

#[test]
fn dirty_discard_unresolved_base_merge_makes_the_root_busy() {
    let mut facts = managed("/w/a", "a");
    facts.base_merge_in_progress = SafetyFact::Yes;

    assert!(matches!(
        classify_delete_eligibility(&facts, DeleteOptions::fail_closed()),
        Err(WorktreeOpError::RootBusy(_))
    ));
    assert!(matches!(
        classify_delete_eligibility(&facts, DeleteOptions::local_discarding_dirty(false)),
        Err(WorktreeOpError::RootBusy(_))
    ));
    assert!(matches!(
        classify_merge_eligibility(&facts, ConflictPolicy::PreserveConflict),
        Err(WorktreeOpError::RootBusy(_))
    ));
}

#[test]
fn dirty_discard_options_keep_teardown_and_discard_independent() {
    assert_eq!(
        DeleteOptions::fail_closed(),
        DeleteOptions {
            skip_teardown: false,
            allow_known_dirty: false
        }
    );
    // `S` chooses teardown only. It is not a force flag and never was.
    assert_eq!(
        DeleteOptions::local(true),
        DeleteOptions {
            skip_teardown: true,
            allow_known_dirty: false
        }
    );
    assert_eq!(
        DeleteOptions::local_discarding_dirty(false),
        DeleteOptions {
            skip_teardown: false,
            allow_known_dirty: true
        }
    );
}

#[test]
fn dirty_discard_safety_facts_fold_observation_failure_into_unknown() {
    assert_eq!(SafetyFact::observed::<()>(Ok(true)), SafetyFact::Yes);
    assert_eq!(SafetyFact::observed::<()>(Ok(false)), SafetyFact::No);
    assert_eq!(SafetyFact::observed(Err("boom")), SafetyFact::Unknown);
    assert!(SafetyFact::Yes.is_known_yes());
    assert!(!SafetyFact::Unknown.is_known_yes());
    assert!(!SafetyFact::No.is_known_yes());
}

// ── Merge eligibility ────────────────────────────────────────────────────────

#[test]
fn remote_worktree_preserving_policy_attempts_a_predicted_conflict() {
    let mut facts = managed("/w/a", "a");
    facts.has_commits_ahead = SafetyFact::Yes;
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
    detached.has_commits_ahead = SafetyFact::Yes;
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

/// A branch-cleanup case: name, ref answer, safe-delete answer, expected reason.
type BranchCleanupCase = (
    &'static str,
    WorktreeOpResult<Option<String>>,
    WorktreeOpResult<()>,
    &'static str,
);

fn dirty(path: &str, branch: &str) -> WorktreeFacts {
    let mut facts = managed(path, branch);
    facts.dirty = DirtyState::Dirty;
    facts
}

#[tokio::test]
async fn dirty_discard_clean_delete_tears_down_then_removes_then_deletes_the_branch() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, sink) = service(backend.clone());

    let outcome = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect("deleted");
    assert_eq!(outcome.branch, "c1");

    // Teardown, a *second* observation, and only then the irreversible step.
    let calls = backend.calls();
    let teardown = calls.iter().position(|c| c == &Call::Teardown).unwrap();
    let remove = calls
        .iter()
        .position(|c| c == &Call::RemoveWorktree)
        .unwrap();
    let reobserve = calls
        .iter()
        .enumerate()
        .position(|(idx, call)| idx > teardown && call == &Call::Observe)
        .expect("safety facts must be re-observed after teardown");
    assert!(
        teardown < reobserve && reobserve < remove,
        "expected teardown -> re-observe -> remove, got {calls:?}"
    );
    assert!(calls.contains(&Call::DeleteBranch {
        branch: "c1".to_string()
    }));
    assert!(sink.events().contains(&WorktreeOperationEvent::Deleted {
        branch: "c1".to_string()
    }));
}

#[tokio::test]
async fn dirty_discard_skip_teardown_runs_no_teardown_but_still_re_observes() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, _sink) = service(backend.clone());

    service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::local(true),
        )
        .await
        .expect("deleted");

    let calls = backend.calls();
    assert!(
        !calls.contains(&Call::Teardown),
        "skip-teardown must not run teardown: {calls:?}"
    );
    // Skipping teardown does not skip the fresh look: the guard excludes
    // Conflux's own mutations, not the rest of the machine.
    assert_eq!(
        calls.iter().filter(|c| **c == Call::Observe).count(),
        2,
        "safety facts must still be re-observed immediately before removal: {calls:?}"
    );
    assert!(calls.contains(&Call::RemoveWorktree));
}

#[tokio::test]
async fn dirty_discard_failed_teardown_retains_the_resource() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    *backend.teardown.lock().unwrap() =
        Err(WorktreeOpError::Internal("teardown exited 1".to_string()));
    let (service, sink) = service(backend.clone());

    let failure = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect_err("must fail");
    assert!(matches!(failure, WorktreeOpError::Internal(_)));
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::RemoveWorktree | Call::DeleteBranch { .. })),
        "a failed teardown must not proceed to removal or branch deletion"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn dirty_discard_teardown_induced_drift_refuses_before_removal() {
    // Each case is one fact the teardown script moved out from under the
    // authorization. All of them are observable, and all of them must stop the
    // irreversible step rather than be carried across it.
    let cases: Vec<(&str, WorktreeFacts)> = vec![
        ("identity", {
            let mut after = managed("/workspaces/c1", "c1");
            after.identity = "gitdir: /workspaces/c1/.git-replaced".to_string();
            after
        }),
        ("branch", managed("/workspaces/c1", "c1-renamed")),
        ("head", {
            let mut after = managed("/workspaces/c1", "c1");
            after.head = "deadbeef".to_string();
            after
        }),
        ("commits-ahead", {
            let mut after = managed("/workspaces/c1", "c1");
            after.has_commits_ahead = SafetyFact::Yes;
            after
        }),
        ("unobservable-dirty", {
            let mut after = managed("/workspaces/c1", "c1");
            after.dirty = DirtyState::Unknown;
            after
        }),
        ("base-merge", {
            let mut after = managed("/workspaces/c1", "c1");
            after.base_merge_in_progress = SafetyFact::Yes;
            after
        }),
    ];

    for (name, after) in cases {
        let backend =
            FakeBackend::scripted(vec![vec![managed("/workspaces/c1", "c1")], vec![after]]);
        let (service, sink) = service(backend.clone());

        let refusal = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local(false),
            )
            .await
            .expect_err(&format!("{name} drift must refuse"));
        assert!(
            !backend.calls().contains(&Call::RemoveWorktree),
            "{name}: forced removal must not run on drifted facts ({refusal})"
        );
        assert!(
            !backend
                .calls()
                .iter()
                .any(|call| matches!(call, Call::DeleteBranch { .. })),
            "{name}: a refused removal must not delete the branch"
        );
        assert!(
            !sink
                .events()
                .iter()
                .any(|event| matches!(event, WorktreeOperationEvent::Deleted { .. })),
            "{name}: nothing was deleted, so nothing may be announced as deleted"
        );
    }
}

#[tokio::test]
async fn dirty_discard_removes_a_known_dirty_worktree_after_explicit_permission() {
    let backend = FakeBackend::stable(vec![dirty("/workspaces/c1", "c1")]);
    let (service, sink) = service(backend.clone());

    // Ordinary local policy refuses and hands back the escalation target.
    let refusal = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::local(false),
        )
        .await
        .expect_err("an ordinary confirmation must not delete dirty content");
    let WorktreeOpError::Dirty { target, .. } = refusal else {
        panic!("expected a dirty refusal, got {refusal:?}");
    };
    assert!(
        !backend.calls().contains(&Call::Teardown)
            && !backend.calls().contains(&Call::RemoveWorktree),
        "a refused dirty delete must not tear down or remove anything"
    );

    // The explicit destructive confirmation replays the same identity.
    service
        .delete_worktree(
            &target.path,
            &ExpectedTarget::on_branch(target.branch.clone())
                .with_identity(target.identity.clone())
                .with_head(target.head.clone()),
            DeleteOptions::local_discarding_dirty(false),
        )
        .await
        .expect("explicit discard must delete");

    assert!(backend.calls().contains(&Call::RemoveWorktree));
    assert!(sink.events().contains(&WorktreeOperationEvent::Deleted {
        branch: "c1".to_string()
    }));
}

#[tokio::test]
async fn dirty_discard_retains_the_branch_when_its_ref_moved_or_cannot_be_reconfirmed() {
    let cases: [BranchCleanupCase; 3] = [
        (
            "ref-moved",
            Ok(Some("0ddba11".to_string())),
            Ok(()),
            "its ref moved",
        ),
        (
            "ref-unreadable",
            Err(WorktreeOpError::Internal("show-ref failed".to_string())),
            Ok(()),
            "could not be reconfirmed",
        ),
        (
            "unreachable-commits",
            Ok(Some("cafebabe".to_string())),
            Err(WorktreeOpError::Internal("not fully merged".to_string())),
            "not reachable from elsewhere",
        ),
    ];

    for (name, branch_ref, delete, expected_reason) in cases {
        let backend = FakeBackend::stable(vec![dirty("/workspaces/c1", "c1")]);
        backend.set_branch_ref("c1", branch_ref);
        *backend.delete_branch.lock().unwrap() = delete;
        let (service, _sink) = service(backend.clone());

        let outcome = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local_discarding_dirty(false),
            )
            .await
            .expect("the worktree itself is still removed");

        // Removal and branch deletion are distinct outcomes: losing the branch
        // would make its commits unreachable on the strength of a stale fact.
        assert!(
            backend.calls().contains(&Call::RemoveWorktree),
            "{name}: the worktree removal itself must still happen"
        );
        assert!(
            outcome.detail.contains("was retained") && outcome.detail.contains(expected_reason),
            "{name}: the outcome must say why the branch survived: {}",
            outcome.detail
        );
    }
}

#[tokio::test]
async fn dirty_discard_deletes_the_branch_only_when_its_ref_still_matches() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    backend.set_branch_ref("c1", Ok(Some("cafebabe".to_string())));
    let (service, _sink) = service(backend.clone());

    let outcome = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect("deleted");

    let calls = backend.calls();
    let checked = calls
        .iter()
        .position(|call| {
            call == &Call::BranchRef {
                branch: "c1".to_string(),
            }
        })
        .expect("the branch ref must be reconfirmed");
    let removed = calls
        .iter()
        .position(|c| c == &Call::RemoveWorktree)
        .unwrap();
    assert!(
        removed < checked,
        "the ref is reconfirmed after removal, against the OID removal was authorized from: {calls:?}"
    );
    assert!(outcome.detail.contains("was deleted"));
}

#[tokio::test]
async fn dirty_discard_delete_of_an_unobserved_path_is_not_found() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, _sink) = service(backend);

    let failure = service
        .delete_worktree(
            Path::new("/workspaces/gone"),
            &ExpectedTarget::unchecked(),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect_err("must fail");
    assert!(matches!(failure, WorktreeOpError::NotFound(_)));
}

#[tokio::test]
async fn dirty_discard_delete_refuses_when_another_branch_now_occupies_the_path() {
    // The caller confirmed `c1`, but by the time the mutation guard is taken the
    // path is occupied by a replacement worktree on a different branch.
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "replacement")]);
    let (service, sink) = service(backend.clone());

    let refusal = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect_err("must refuse");
    assert!(matches!(refusal, WorktreeOpError::NotFound(_)));
    assert!(
        refusal.to_string().contains("c1") && refusal.to_string().contains("replacement"),
        "the refusal must name both the confirmed and the observed identity: {refusal}"
    );

    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Teardown | Call::RemoveWorktree)),
        "a stale identity must refuse before the backend is asked to remove anything"
    );
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::DeleteBranch { .. })),
        "a stale identity must not delete a branch either"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn dirty_discard_delete_proceeds_when_the_confirmed_identity_still_occupies_the_path() {
    let backend = FakeBackend::stable(vec![managed("/workspaces/c1", "c1")]);
    let (service, _sink) = service(backend.clone());

    service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1")
                .with_identity("gitdir: /workspaces/c1/.git")
                .with_head("cafebabe"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect("a matching identity must delete");

    assert!(backend.calls().contains(&Call::RemoveWorktree));
}

#[test]
fn dirty_discard_expected_identity_checks_only_what_the_caller_can_name() {
    let facts = managed("/workspaces/c1", "c1");

    assert!(classify_delete_identity(&facts, &ExpectedTarget::unchecked()).is_ok());
    assert!(classify_delete_identity(&facts, &ExpectedTarget::on_branch("c1")).is_ok());
    assert!(classify_delete_identity(
        &facts,
        &ExpectedTarget::on_branch("c1")
            .with_identity("gitdir: /workspaces/c1/.git")
            .with_head("cafebabe")
    )
    .is_ok());

    for expected in [
        ExpectedTarget::on_branch("other"),
        ExpectedTarget::unchecked().with_identity("gitdir: /elsewhere/.git"),
        ExpectedTarget::unchecked().with_head("deadbeef"),
    ] {
        assert!(
            matches!(
                classify_delete_identity(&facts, &expected),
                Err(WorktreeOpError::NotFound(_))
            ),
            "{expected:?} must refuse a mismatched observation"
        );
    }

    assert!(matches!(
        classify_delete_identity(
            &managed("/workspaces/c1", ""),
            &ExpectedTarget::on_branch("c1")
        ),
        Err(WorktreeOpError::NotFound(_))
    ));
}

// ── Merge ────────────────────────────────────────────────────────────────────

fn mergeable() -> WorktreeFacts {
    let mut facts = managed("/workspaces/c1", "c1");
    facts.has_commits_ahead = SafetyFact::Yes;
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
async fn dirty_discard_concurrent_mutation_reports_root_busy() {
    let backend = FakeBackend::stable(vec![mergeable()]);
    let (service, _sink) = service(backend);

    // Hold the guard exactly as an in-flight operation would.
    let held = service
        .acquire_root()
        .expect("first caller takes the guard");
    let refusal = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::fail_closed(),
        )
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
