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
    DeleteBranchAt {
        branch: String,
        expected_oid: String,
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
    /// Why each `observe()` said it was being taken.
    ///
    /// Recorded separately from [`Call`] so the existing call-ordering
    /// assertions keep comparing the sequence of operations, while the tests
    /// that care about *cost* can assert the intent each observation carried.
    observe_requests: Mutex<Vec<crate::worktree_ops::ObservationRequest>>,
    merge: Mutex<WorktreeOpResult<MergeAttempt>>,
    on_merged: Mutex<WorktreeOpResult<()>>,
    teardown: Mutex<WorktreeOpResult<()>>,
    remove: Mutex<WorktreeOpResult<()>>,
    eligible: Mutex<WorktreeOpResult<()>>,
    /// Successive answers each branch's ref reads produce, or the failure reading it.
    ///
    /// A missing entry means "no entry recorded"; the default answer is the
    /// observed HEAD, which is the ordinary case. Deletion reads the ref twice —
    /// once to authorize removal and once during cleanup — so an entry is a
    /// script whose last answer repeats, not a single value.
    branch_refs: Mutex<std::collections::HashMap<String, Vec<WorktreeOpResult<Option<String>>>>>,
    delete_branch: Mutex<WorktreeOpResult<()>>,
    /// What the atomic compare-and-delete replays.
    ///
    /// The real operation compares inside the ref transaction, so the fake has
    /// no ref to consult: a moved, missing, or unreadable ref all reach the
    /// service as the same failure, and that is exactly the distinction the
    /// service is not allowed to make.
    delete_branch_at: Mutex<WorktreeOpResult<()>>,
}

impl FakeBackend {
    /// Every `observe()` returns the same set.
    fn stable(facts: Vec<WorktreeFacts>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            observations: Mutex::new(vec![facts]),
            observe_requests: Mutex::new(Vec::new()),
            merge: Mutex::new(Ok(MergeAttempt::Merged)),
            on_merged: Mutex::new(Ok(())),
            teardown: Mutex::new(Ok(())),
            remove: Mutex::new(Ok(())),
            eligible: Mutex::new(Ok(())),
            branch_refs: Mutex::new(std::collections::HashMap::new()),
            delete_branch: Mutex::new(Ok(())),
            delete_branch_at: Mutex::new(Ok(())),
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

    fn observe_requests(&self) -> Vec<crate::worktree_ops::ObservationRequest> {
        self.observe_requests.lock().unwrap().clone()
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }

    /// Every `branch_ref()` for `branch` answers this.
    fn set_branch_ref(&self, branch: &str, answer: WorktreeOpResult<Option<String>>) {
        self.set_branch_refs(branch, vec![answer]);
    }

    /// Successive `branch_ref()` calls for `branch` walk the script, then repeat the last.
    fn set_branch_refs(&self, branch: &str, answers: Vec<WorktreeOpResult<Option<String>>>) {
        self.branch_refs
            .lock()
            .unwrap()
            .insert(branch.to_string(), answers);
    }
}

#[async_trait]
impl WorktreeBackend for FakeBackend {
    async fn observe(
        &self,
        request: crate::worktree_ops::ObservationRequest,
    ) -> WorktreeOpResult<Vec<WorktreeFacts>> {
        self.record(Call::Observe);
        self.observe_requests.lock().unwrap().push(request);
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
        let mut scripts = self.branch_refs.lock().unwrap();
        match scripts.get_mut(branch) {
            Some(script) if script.len() > 1 => script.remove(0),
            Some(script) => script
                .first()
                .cloned()
                .unwrap_or_else(|| Ok(Some("cafebabe".to_string()))),
            None => Ok(Some("cafebabe".to_string())),
        }
    }

    async fn delete_branch_if_merged(&self, branch: &str) -> WorktreeOpResult<()> {
        self.record(Call::DeleteBranch {
            branch: branch.to_string(),
        });
        self.delete_branch.lock().unwrap().clone()
    }

    async fn delete_branch_at(&self, branch: &str, expected_oid: &str) -> WorktreeOpResult<()> {
        self.record(Call::DeleteBranchAt {
            branch: branch.to_string(),
            expected_oid: expected_oid.to_string(),
        });
        self.delete_branch_at.lock().unwrap().clone()
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
        // Ahead is reported *instead of* dirty on purpose: `Dirty` means
        // "nothing but uncommitted work stands in the way", so a worktree
        // carrying unmerged commits must never produce it. Dirty-discard
        // permission is not ahead-discard permission and cannot reach removal
        // through the dirty confirmation.
        let refusal = classify_delete_eligibility(&ahead, options)
            .expect_err(&format!("{options:?} must refuse commits ahead"));
        let WorktreeOpError::CommitsAhead { target, .. } = refusal else {
            panic!("{options:?} must refuse with the typed ahead refusal, got {refusal:?}");
        };
        assert!(
            target.dirty,
            "{options:?}: the ahead target must carry the known dirty fact so one \
             confirmation can disclose both losses"
        );
    }
}

#[test]
fn ahead_discard_is_the_only_policy_that_deletes_known_unmerged_commits() {
    let mut ahead = managed("/w/a", "a");
    ahead.has_commits_ahead = SafetyFact::Yes;

    for options in [
        DeleteOptions::fail_closed(),
        DeleteOptions::local(false),
        DeleteOptions::local(true),
        DeleteOptions::local_discarding_dirty(false),
        DeleteOptions::local_discarding_dirty(true),
    ] {
        let refusal = classify_delete_eligibility(&ahead, options)
            .expect_err(&format!("{options:?} must refuse commits ahead"));
        let WorktreeOpError::CommitsAhead { target, .. } = refusal else {
            panic!("{options:?} must refuse with the typed ahead refusal, got {refusal:?}");
        };
        // The refusal carries the observation the escalation confirms against,
        // including the OID the branch would later be deleted at.
        assert_eq!(target.path, PathBuf::from("/w/a"));
        assert_eq!(target.identity, "gitdir: /w/a/.git");
        assert_eq!(target.branch, "a");
        assert_eq!(target.head, "cafebabe");
        assert!(!target.dirty, "a clean ahead worktree must not claim dirty");
    }

    for options in [
        DeleteOptions::local_discarding_ahead(false, false),
        DeleteOptions::local_discarding_ahead(true, false),
    ] {
        assert_eq!(
            classify_delete_eligibility(&ahead, options),
            Ok(()),
            "{options:?} is the explicit permission and must pass a clean ahead worktree"
        );
    }
}

#[test]
fn ahead_discard_does_not_escalate_an_unobservable_dirty_state() {
    // Nothing here can be disclosed truthfully: the confirmation would have to
    // state whether uncommitted work is lost, and the observation cannot say.
    let mut facts = managed("/w/a", "a");
    facts.has_commits_ahead = SafetyFact::Yes;
    facts.dirty = DirtyState::Unknown;

    for options in [
        DeleteOptions::fail_closed(),
        DeleteOptions::local(false),
        DeleteOptions::local_discarding_dirty(false),
        DeleteOptions::local_discarding_ahead(false, false),
        DeleteOptions::local_discarding_ahead(false, true),
    ] {
        assert!(
            matches!(
                classify_delete_eligibility(&facts, options),
                Err(WorktreeOpError::DirtyUnknown(_))
            ),
            "{options:?} must fail closed on an unobservable dirty state without escalating"
        );
    }
}

#[test]
fn ahead_discard_never_waives_a_permission_it_was_not_granted() {
    // Each row: what the worktree is, and the permission that is *not* enough.
    let mut dirty_only = managed("/w/a", "a");
    dirty_only.dirty = DirtyState::Dirty;

    let mut both = managed("/w/a", "a");
    both.has_commits_ahead = SafetyFact::Yes;
    both.dirty = DirtyState::Dirty;

    // Ahead permission alone does not discard uncommitted work.
    assert!(matches!(
        classify_delete_eligibility(
            &dirty_only,
            DeleteOptions::local_discarding_ahead(false, false)
        ),
        Err(WorktreeOpError::Dirty { .. })
    ));
    assert!(matches!(
        classify_delete_eligibility(&both, DeleteOptions::local_discarding_ahead(false, false)),
        Err(WorktreeOpError::Dirty { .. })
    ));
    // Only the confirmation that disclosed both losses grants both.
    assert_eq!(
        classify_delete_eligibility(&both, DeleteOptions::local_discarding_ahead(false, true)),
        Ok(())
    );

    // And main status is not a permission either frontend can hold.
    let mut main_ahead = managed("/repo", "main");
    main_ahead.is_main = true;
    main_ahead.has_commits_ahead = SafetyFact::Yes;
    assert!(matches!(
        classify_delete_eligibility(
            &main_ahead,
            DeleteOptions::local_discarding_ahead(false, true)
        ),
        Err(WorktreeOpError::Ineligible(_))
    ));

    // Nor is an unresolved base merge at the repository root.
    let mut busy_ahead = managed("/w/a", "a");
    busy_ahead.has_commits_ahead = SafetyFact::Yes;
    busy_ahead.base_merge_in_progress = SafetyFact::Yes;
    assert!(matches!(
        classify_delete_eligibility(
            &busy_ahead,
            DeleteOptions::local_discarding_ahead(false, true)
        ),
        Err(WorktreeOpError::RootBusy(_))
    ));

    // Nor an unobservable one.
    let mut unknown_merge_ahead = managed("/w/a", "a");
    unknown_merge_ahead.has_commits_ahead = SafetyFact::Yes;
    unknown_merge_ahead.base_merge_in_progress = SafetyFact::Unknown;
    assert!(matches!(
        classify_delete_eligibility(
            &unknown_merge_ahead,
            DeleteOptions::local_discarding_ahead(false, true)
        ),
        Err(WorktreeOpError::Ineligible(_))
    ));
}

#[test]
fn ahead_discard_never_waives_an_unobservable_ahead_state() {
    let mut facts = managed("/w/a", "a");
    facts.has_commits_ahead = SafetyFact::Unknown;

    for options in [
        DeleteOptions::local_discarding_ahead(false, false),
        DeleteOptions::local_discarding_ahead(true, true),
    ] {
        let refusal = classify_delete_eligibility(&facts, options).expect_err(&format!(
            "{options:?} must refuse an unobservable ahead state"
        ));
        assert!(
            matches!(refusal, WorktreeOpError::Ineligible(_))
                && refusal.to_string().contains("could not be determined"),
            "{options:?}: permission to discard known commits is not permission to guess: {refusal}"
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
            allow_known_dirty: false,
            allow_commits_ahead: false,
        }
    );
    // `S` chooses teardown only. It is not a force flag and never was.
    assert_eq!(
        DeleteOptions::local(true),
        DeleteOptions {
            skip_teardown: true,
            allow_known_dirty: false,
            allow_commits_ahead: false,
        }
    );
    assert_eq!(
        DeleteOptions::local_discarding_dirty(false),
        DeleteOptions {
            skip_teardown: false,
            allow_known_dirty: true,
            allow_commits_ahead: false,
        }
    );
    // Ahead discard is its own bit. Granting it says nothing about teardown, and
    // says nothing about uncommitted work unless that was disclosed too.
    assert_eq!(
        DeleteOptions::local_discarding_ahead(false, false),
        DeleteOptions {
            skip_teardown: false,
            allow_known_dirty: false,
            allow_commits_ahead: true,
        }
    );
    assert_eq!(
        DeleteOptions::local_discarding_ahead(true, true),
        DeleteOptions {
            skip_teardown: true,
            allow_known_dirty: true,
            allow_commits_ahead: true,
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

#[test]
fn an_uninspected_worktree_is_refused_for_inspection_not_for_being_behind() {
    // What periodic refresh skipped looks exactly like a clean, not-ahead
    // worktree: no conflict files, no commits ahead. The refusal must say which
    // of the two it is, because "there is nothing to merge" sends an operator
    // away and "nobody looked yet" does not.
    let mut skipped = managed("/w/a", "a");
    skipped.inspection = crate::worktree_ops::InspectionState::NotInspected;
    skipped.has_commits_ahead = SafetyFact::Unknown;

    let Err(WorktreeOpError::Ineligible(message)) =
        classify_merge_eligibility(&skipped, ConflictPolicy::PreserveConflict)
    else {
        panic!("an uninspected worktree must not be merge-eligible");
    };
    assert!(
        message.contains("not been inspected"),
        "the refusal must name the missing inspection, got: {message}"
    );
    assert!(
        !message.contains("no commits ahead"),
        "an uninspected worktree must not be reported as having nothing to merge: {message}"
    );

    // Once it *is* inspected, an unobservable ahead state is still not the same
    // refusal as a confident "behind".
    let mut unobservable = managed("/w/a", "a");
    unobservable.has_commits_ahead = SafetyFact::Unknown;
    let Err(WorktreeOpError::Ineligible(message)) =
        classify_merge_eligibility(&unobservable, ConflictPolicy::PreserveConflict)
    else {
        panic!("an unobservable ahead state must fail closed");
    };
    assert!(
        message.contains("could not be determined"),
        "an inspected-but-unanswerable state has its own refusal, got: {message}"
    );

    // And an inspected, ahead worktree still merges.
    let mut ready = managed("/w/a", "a");
    ready.has_commits_ahead = SafetyFact::Yes;
    assert!(classify_merge_eligibility(&ready, ConflictPolicy::PreserveConflict).is_ok());
}

#[tokio::test]
async fn operator_merge_and_delete_both_observe_their_own_target_freshly() {
    // Periodic filtering decides what a *refresh* spends, never what an
    // operator may do. Every mutation entry point therefore asks for a
    // targeted observation of the exact worktree it was pointed at, including
    // one whose branch maps to no change at all.
    let mut session = managed("/workspaces/ws-session-a1b2c3", "ws-session-a1b2c3");
    session.has_commits_ahead = SafetyFact::Yes;

    let backend = FakeBackend::stable(vec![session.clone()]);
    let (service, _sink) = service(backend.clone());
    service
        .merge_worktree(&session.path, ConflictPolicy::AbortOnConflict)
        .await
        .expect("an unmapped branch is still mergeable on operator request");

    assert_eq!(
        backend.observe_requests(),
        vec![crate::worktree_ops::ObservationRequest::Target(
            session.path.clone()
        )],
        "merge must re-derive its own evidence for the addressed worktree"
    );
}

#[tokio::test]
async fn operator_deletion_observes_its_own_target_freshly() {
    // Same contract as the merge above, across the teardown boundary: both
    // observations name the worktree being deleted, so periodic filtering can
    // never make a stale worktree undeletable.
    let backend = FakeBackend::stable(vec![managed("/workspaces/stale", "stale")]);
    let (deleting_service, _sink) = service(backend.clone());
    let target = PathBuf::from("/workspaces/stale");

    deleting_service
        .delete_worktree(
            &target,
            &ExpectedTarget::on_branch("stale"),
            DeleteOptions::fail_closed(),
        )
        .await
        .expect("a stale worktree stays deletable");

    let requests = backend.observe_requests();
    assert!(
        requests.len() >= 2
            && requests.iter().all(|request| *request
                == crate::worktree_ops::ObservationRequest::Target(target.clone())),
        "every deletion observation must target the worktree being deleted, got {requests:?}"
    );
}

#[tokio::test]
async fn a_listing_never_buys_inspection_it_does_not_need() {
    // Creation only asks whether a branch or path is already taken, which is
    // structural. Paying for a merge simulation of every registered worktree to
    // answer it is exactly the cost this change exists to stop.
    let backend = FakeBackend::scripted(vec![vec![], vec![managed("/workspaces/c1", "c1")]]);
    let (service, _sink) = service(backend.clone());

    service.create_change_worktree("c1").await.expect("created");

    assert_eq!(
        backend.observe_requests(),
        vec![
            crate::worktree_ops::ObservationRequest::Listing,
            crate::worktree_ops::ObservationRequest::Target(PathBuf::from("/workspaces/c1")),
        ],
        "existence is a structural question; only the created worktree is inspected"
    );
}

#[test]
fn an_uninspected_worktree_never_reads_as_conflict_free() {
    // The abort-on-conflict policy refuses on a non-empty conflict list. An
    // uninspected observation has an empty one for the opposite reason, so it
    // must be refused before that check can mistake it for a clean merge.
    let mut skipped = managed("/w/a", "a");
    skipped.inspection = crate::worktree_ops::InspectionState::NotInspected;
    skipped.has_commits_ahead = SafetyFact::Yes;
    assert!(skipped.conflict_files.is_empty());

    assert!(
        classify_merge_eligibility(&skipped, ConflictPolicy::AbortOnConflict).is_err(),
        "an empty conflict list nobody produced must not authorize a merge"
    );
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

/// A branch-cleanup case: name, cleanup-time ref answer, safe-delete answer, expected reason.
type BranchCleanupCase = (
    &'static str,
    WorktreeOpResult<Option<String>>,
    WorktreeOpResult<()>,
    &'static str,
);

/// A pre-removal ref case: name, ref answer, expected refusal fragment.
type BranchRefGateCase = (&'static str, WorktreeOpResult<Option<String>>, &'static str);

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
        // The ref is intact when removal is authorized and only drifts
        // afterwards: that residual window is exactly what cleanup must survive.
        backend.set_branch_refs("c1", vec![Ok(Some("cafebabe".to_string())), branch_ref]);
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
    let checks: Vec<usize> = calls
        .iter()
        .enumerate()
        .filter(|(_, call)| {
            *call
                == &Call::BranchRef {
                    branch: "c1".to_string(),
                }
        })
        .map(|(index, _)| index)
        .collect();
    let removed = calls
        .iter()
        .position(|c| c == &Call::RemoveWorktree)
        .unwrap();
    assert!(
        checks.first().is_some_and(|first| *first < removed),
        "the ref is confirmed before the irreversible removal: {calls:?}"
    );
    assert!(
        checks.last().is_some_and(|last| *last > removed),
        "and reconfirmed after it, against the OID removal was authorized from: {calls:?}"
    );
    assert!(outcome.detail.contains("was deleted"));
}

// ── Explicit ahead discard ───────────────────────────────────────────────────

fn ahead(path: &str, branch: &str) -> WorktreeFacts {
    let mut facts = managed(path, branch);
    facts.has_commits_ahead = SafetyFact::Yes;
    facts
}

#[tokio::test]
async fn ahead_discard_ordinary_deletion_escalates_instead_of_removing_anything() {
    for (name, facts) in [
        ("clean", ahead("/workspaces/c1", "c1")),
        ("dirty", {
            let mut facts = ahead("/workspaces/c1", "c1");
            facts.dirty = DirtyState::Dirty;
            facts
        }),
    ] {
        let expected_dirty = facts.dirty == DirtyState::Dirty;
        let backend = FakeBackend::stable(vec![facts]);
        let (service, sink) = service(backend.clone());

        for skip_teardown in [false, true] {
            let refusal = service
                .delete_worktree(
                    Path::new("/workspaces/c1"),
                    &ExpectedTarget::on_branch("c1"),
                    DeleteOptions::local(skip_teardown),
                )
                .await
                .expect_err(&format!(
                    "{name}/{skip_teardown}: ordinary deletion must refuse"
                ));
            let WorktreeOpError::CommitsAhead { target, .. } = refusal else {
                panic!("{name}: expected a typed ahead refusal, got {refusal:?}");
            };
            // The escalation names the service's own fresh look, including the
            // OID the branch would be deleted at.
            assert_eq!(target.path, PathBuf::from("/workspaces/c1"));
            assert_eq!(target.identity, "gitdir: /workspaces/c1/.git");
            assert_eq!(target.branch, "c1");
            assert_eq!(target.head, "cafebabe");
            assert_eq!(target.dirty, expected_dirty);
        }

        assert!(
            !backend.calls().iter().any(|call| matches!(
                call,
                Call::Teardown
                    | Call::RemoveWorktree
                    | Call::DeleteBranch { .. }
                    | Call::DeleteBranchAt { .. }
            )),
            "{name}: a refused ahead delete must not tear down, remove, or delete a branch"
        );
        assert!(
            sink.events().is_empty(),
            "{name}: nothing happened to announce"
        );
    }
}

#[tokio::test]
async fn ahead_discard_removes_the_worktree_then_compare_and_deletes_the_branch() {
    for (name, mut facts, options) in [
        (
            "clean",
            ahead("/workspaces/c1", "c1"),
            DeleteOptions::local_discarding_ahead(false, false),
        ),
        (
            "dirty",
            ahead("/workspaces/c1", "c1"),
            DeleteOptions::local_discarding_ahead(false, true),
        ),
    ] {
        if name == "dirty" {
            facts.dirty = DirtyState::Dirty;
        }
        let backend = FakeBackend::stable(vec![facts]);
        let (service, sink) = service(backend.clone());

        let outcome = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1")
                    .with_identity("gitdir: /workspaces/c1/.git")
                    .with_head("cafebabe"),
                options,
            )
            .await
            .unwrap_or_else(|error| panic!("{name}: explicit ahead discard must delete: {error}"));

        let calls = backend.calls();
        let removed = calls
            .iter()
            .position(|call| call == &Call::RemoveWorktree)
            .unwrap_or_else(|| panic!("{name}: the worktree must be removed: {calls:?}"));
        let deleted = calls
            .iter()
            .position(|call| {
                call == &Call::DeleteBranchAt {
                    branch: "c1".to_string(),
                    expected_oid: "cafebabe".to_string(),
                }
            })
            .unwrap_or_else(|| {
                panic!("{name}: the branch must be compare-and-deleted at the confirmed OID: {calls:?}")
            });
        // Git cannot delete a checked-out branch, and losing the branch before
        // the worktree is gone would be the worse half of a partial failure.
        assert!(
            removed < deleted,
            "{name}: removal must precede branch deletion: {calls:?}"
        );
        // Ordinary merged-only cleanup is not weakened into doing this: it is a
        // different call and it is not made here.
        assert!(
            !calls
                .iter()
                .any(|call| matches!(call, Call::DeleteBranch { .. })),
            "{name}: explicit ahead discard must not route through merged-only cleanup: {calls:?}"
        );
        assert!(
            !outcome.branch_retained,
            "{name}: a completed discard retains nothing"
        );
        assert!(
            outcome.detail.contains("unmerged commits were deleted"),
            "{name}: the outcome must say the commits went: {}",
            outcome.detail
        );
        assert!(sink.events().contains(&WorktreeOperationEvent::Deleted {
            branch: "c1".to_string()
        }));
    }
}

#[tokio::test]
async fn ahead_discard_retains_the_branch_when_the_compare_and_delete_fails() {
    // A moved, missing, or unreadable ref all fail the same transaction, and
    // all of them must leave the branch standing. The worktree is already gone
    // by then, so this is a partial success, not a rollback.
    for (name, failure) in [
        (
            "ref-moved",
            WorktreeOpError::Internal("update-ref: reference already exists".to_string()),
        ),
        (
            "ref-missing",
            WorktreeOpError::Internal("update-ref: unable to resolve reference".to_string()),
        ),
        (
            "ref-unreadable",
            WorktreeOpError::Internal("update-ref: could not read ref".to_string()),
        ),
    ] {
        let backend = FakeBackend::stable(vec![ahead("/workspaces/c1", "c1")]);
        *backend.delete_branch_at.lock().unwrap() = Err(failure);
        let (service, _sink) = service(backend.clone());

        let outcome = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local_discarding_ahead(false, false),
            )
            .await
            .unwrap_or_else(|error| {
                panic!("{name}: the worktree removal still succeeded: {error}")
            });

        assert!(
            backend.calls().contains(&Call::RemoveWorktree),
            "{name}: the worktree itself is still removed"
        );
        assert!(
            outcome.branch_retained,
            "{name}: a retained branch must be reported as partial success"
        );
        assert!(
            outcome.detail.contains("was retained")
                && outcome.detail.contains("confirmed commit 'cafebabe'"),
            "{name}: the outcome must say why the branch survived: {}",
            outcome.detail
        );
    }
}

#[tokio::test]
async fn ahead_discard_failed_teardown_retains_both_resources() {
    let backend = FakeBackend::stable(vec![ahead("/workspaces/c1", "c1")]);
    *backend.teardown.lock().unwrap() = Err(WorktreeOpError::Internal("teardown exited 1".into()));
    let (svc, sink) = service(backend.clone());

    svc.delete_worktree(
        Path::new("/workspaces/c1"),
        &ExpectedTarget::on_branch("c1"),
        DeleteOptions::local_discarding_ahead(false, false),
    )
    .await
    .expect_err("a failed teardown must refuse");

    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::RemoveWorktree | Call::DeleteBranchAt { .. })),
        "a failed teardown must not remove the worktree or delete the branch"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn ahead_discard_refuses_removal_when_the_branch_ref_cannot_be_confirmed() {
    // The confirmed OID is the whole basis for the later compare-and-delete, so
    // an unconfirmable ref stops the operation outright rather than being
    // discovered once the worktree is already gone.
    let cases: [BranchRefGateCase; 3] = [
        ("ref-moved", Ok(Some("0ddba11".to_string())), "moved from"),
        ("ref-missing", Ok(None), "no longer exists"),
        (
            "ref-unreadable",
            Err(WorktreeOpError::Internal("show-ref failed".to_string())),
            "could not be determined",
        ),
    ];

    for (name, branch_ref, expected_reason) in cases {
        let backend = FakeBackend::stable(vec![ahead("/workspaces/c1", "c1")]);
        backend.set_branch_ref("c1", branch_ref);
        let (svc, _sink) = service(backend.clone());

        let refusal = svc
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local_discarding_ahead(false, false),
            )
            .await
            .expect_err(&format!("{name} must refuse"));
        assert!(
            refusal.to_string().contains(expected_reason)
                && refusal.to_string().contains("nothing was removed"),
            "{name}: the refusal must say why nothing was removed: {refusal}"
        );
        assert!(
            !backend
                .calls()
                .iter()
                .any(|call| matches!(call, Call::RemoveWorktree | Call::DeleteBranchAt { .. })),
            "{name}: neither the worktree nor the branch may be touched"
        );
    }
}

#[tokio::test]
async fn ahead_discard_worktree_removal_failure_keeps_the_branch() {
    let backend = FakeBackend::stable(vec![ahead("/workspaces/c1", "c1")]);
    *backend.remove.lock().unwrap() = Err(WorktreeOpError::Internal("worktree locked".into()));
    let (svc, sink) = service(backend.clone());

    svc.delete_worktree(
        Path::new("/workspaces/c1"),
        &ExpectedTarget::on_branch("c1"),
        DeleteOptions::local_discarding_ahead(false, false),
    )
    .await
    .expect_err("a failed removal must fail the operation");

    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| matches!(call, Call::DeleteBranchAt { .. })),
        "a worktree that is still there must keep its branch"
    );
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn ahead_discard_refuses_when_teardown_moved_any_authorized_fact() {
    // The permission was granted over one observation. Re-running eligibility
    // would not catch a *waived* fact drifting, so every fact is compared.
    let cases: Vec<(&str, WorktreeFacts)> = vec![
        ("identity", {
            let mut after = ahead("/workspaces/c1", "c1");
            after.identity = "gitdir: /workspaces/c1/.git-replaced".to_string();
            after
        }),
        ("branch", ahead("/workspaces/c1", "c1-renamed")),
        ("head", {
            let mut after = ahead("/workspaces/c1", "c1");
            after.head = "deadbeef".to_string();
            after
        }),
        // Waived by the permission in hand, and still not allowed to move.
        ("commits-ahead-gone", managed("/workspaces/c1", "c1")),
        ("became-dirty", {
            let mut after = ahead("/workspaces/c1", "c1");
            after.dirty = DirtyState::Dirty;
            after
        }),
        ("unobservable-dirty", {
            let mut after = ahead("/workspaces/c1", "c1");
            after.dirty = DirtyState::Unknown;
            after
        }),
        ("base-merge", {
            let mut after = ahead("/workspaces/c1", "c1");
            after.base_merge_in_progress = SafetyFact::Yes;
            after
        }),
    ];

    for (name, after) in cases {
        let backend = FakeBackend::scripted(vec![vec![ahead("/workspaces/c1", "c1")], vec![after]]);
        let (service, sink) = service(backend.clone());

        let refusal = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local_discarding_ahead(false, false),
            )
            .await
            .expect_err(&format!("{name} drift must refuse"));
        assert!(
            !backend
                .calls()
                .iter()
                .any(|call| matches!(call, Call::RemoveWorktree | Call::DeleteBranchAt { .. })),
            "{name}: nothing irreversible may run on drifted facts ({refusal})"
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
async fn ahead_discard_yields_to_a_concurrent_operation_on_the_same_root() {
    let backend = FakeBackend::stable(vec![ahead("/workspaces/c1", "c1")]);
    let (service, _sink) = service(backend.clone());
    let _busy = service.acquire_root_for_test().expect("first holder");

    let refusal = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::local_discarding_ahead(false, true),
        )
        .await
        .expect_err("a busy root must refuse even the explicitly authorized discard");
    assert!(matches!(refusal, WorktreeOpError::RootBusy(_)));
    assert!(
        backend.calls().is_empty(),
        "the guard is taken before anything is observed: {:?}",
        backend.calls()
    );
}

#[tokio::test]
async fn dirty_discard_refuses_removal_when_the_branch_ref_cannot_be_confirmed() {
    // The worktree's own HEAD says nothing about where the branch ref points.
    // Discarding a dirty worktree destroys its uncommitted work, so the branch
    // still naming the authorized commit is the only thing keeping the
    // committed work recoverable — an unconfirmable ref must stop the removal,
    // not be discovered by best-effort cleanup once the directory is gone.
    let cases: [BranchRefGateCase; 3] = [
        ("ref-moved", Ok(Some("0ddba11".to_string())), "moved from"),
        ("ref-missing", Ok(None), "no longer exists"),
        (
            "ref-unreadable",
            Err(WorktreeOpError::Internal("show-ref failed".to_string())),
            "could not be determined",
        ),
    ];

    for (name, branch_ref, expected_reason) in cases {
        let backend = FakeBackend::stable(vec![dirty("/workspaces/c1", "c1")]);
        backend.set_branch_ref("c1", branch_ref);
        let (service, sink) = service(backend.clone());

        let refusal = service
            .delete_worktree(
                Path::new("/workspaces/c1"),
                &ExpectedTarget::on_branch("c1"),
                DeleteOptions::local_discarding_dirty(false),
            )
            .await
            .expect_err(&format!("{name} must refuse"));
        assert!(
            refusal.to_string().contains(expected_reason)
                && refusal.to_string().contains("nothing was removed"),
            "{name}: the refusal must say why nothing was removed: {refusal}"
        );
        assert!(
            !backend.calls().contains(&Call::RemoveWorktree),
            "{name}: forced removal must not run on an unconfirmed branch ref"
        );
        assert!(
            !backend
                .calls()
                .iter()
                .any(|call| matches!(call, Call::DeleteBranch { .. })),
            "{name}: the branch must be retained too"
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
async fn dirty_discard_refuses_when_the_branch_ref_moves_during_teardown() {
    // Teardown is operator code: the ref can be intact when the deletion is
    // authorized and moved by the time removal would run.
    let backend = FakeBackend::stable(vec![dirty("/workspaces/c1", "c1")]);
    backend.set_branch_ref("c1", Ok(Some("0ddba11".to_string())));
    let (service, _sink) = service(backend.clone());

    let refusal = service
        .delete_worktree(
            Path::new("/workspaces/c1"),
            &ExpectedTarget::on_branch("c1"),
            DeleteOptions::local_discarding_dirty(false),
        )
        .await
        .expect_err("a ref that moved during teardown must refuse");
    let calls = backend.calls();
    assert!(
        calls.contains(&Call::Teardown) && !calls.contains(&Call::RemoveWorktree),
        "teardown ran, but the removal it authorized must not: {refusal}"
    );
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
