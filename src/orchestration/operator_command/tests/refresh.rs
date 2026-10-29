//! Candidate proposal refresh through the shared operator service.
//!
//! Evidence classes:
//! - unit: lifecycle admission over in-memory reducer state and doubles
//! - integration: the apply path against a disposable real-Git candidate

use super::*;
use crate::orchestration::proposal_refresh::tests::{Fixture as GitFixture, CHANGE};
use crate::orchestration::proposal_refresh::{RefreshOutcome, RefreshReason, RefreshRequest};

/// A port that names a worktree directory that is not a repository.
///
/// If a lifecycle refusal ever let Git run, the answer would be a command-time
/// Git failure rather than the published lifecycle reason.
struct NotARepository(tempfile::TempDir);

impl CandidateWorktreePort for NotARepository {
    fn project_root(&self) -> Option<std::path::PathBuf> {
        Some(self.0.path().to_path_buf())
    }

    fn worktree_path(&self, _change_id: &str) -> Option<std::path::PathBuf> {
        Some(self.0.path().to_path_buf())
    }
}

fn request(change_id: &str) -> RefreshRequest {
    RefreshRequest {
        change_id: change_id.to_string(),
        source_ref: "develop".to_string(),
        expected_source: "a".repeat(40),
        expected_head: "b".repeat(40),
    }
}

#[tokio::test]
async fn candidate_proposal_refresh_service_refuses_lifecycle_rows_before_any_git_read() {
    let ids = ["queued-row", "applying-row", "merge-row", "unknown-row"];
    let mut fixture = fixture(&ids[..3]);
    fixture.service = fixture
        .service
        .with_candidate_worktrees(Arc::new(NotARepository(tempfile::TempDir::new().unwrap())));
    reach_status(&fixture, "queued-row", "queued").await;
    reach_status(&fixture, "applying-row", "applying").await;
    reach_status(&fixture, "merge-row", "merge wait").await;

    for (change_id, expected) in [
        ("queued-row", RefreshReason::TargetAdmitted),
        ("applying-row", RefreshReason::TargetActive),
        ("merge-row", RefreshReason::MergeWait),
        ("unknown-row", RefreshReason::UnknownTarget),
    ] {
        let error = fixture
            .service
            .refresh_proposal(&request(change_id))
            .await
            .expect_err("an ineligible row must be refused");
        match error {
            OperatorCommandError::RefreshIneligible { reason, .. } => {
                assert_eq!(reason, expected, "{change_id}")
            }
            other => panic!("{change_id}: expected a lifecycle refusal, got {other:?}"),
        }
        let preview = fixture
            .service
            .preview_proposal_refresh(change_id, "develop")
            .await;
        assert_eq!(preview.refusal, Some(expected), "{change_id}");
        assert!(!preview.eligible);
        assert_eq!(preview.source_commit, None, "no Git read happened");
    }
}

#[tokio::test]
async fn candidate_proposal_refresh_service_refuses_a_row_without_a_worktree() {
    let fixture = fixture(&["change-a"]);
    reach_status(&fixture, "change-a", "error").await;

    let error = fixture
        .service
        .refresh_proposal(&request("change-a"))
        .await
        .expect_err("no candidate worktree is bound");
    assert!(matches!(
        error,
        OperatorCommandError::RefreshIneligible {
            reason: RefreshReason::NoCandidateWorktree,
            ..
        }
    ));
    assert!(error.to_string().contains("no_candidate_worktree"));
}

/// The real worktree layout a production assembly binds.
fn git_service(git: &GitFixture) -> Fixture {
    let mut fixture = fixture(&[CHANGE]);
    fixture.service =
        fixture
            .service
            .with_candidate_worktrees(Arc::new(WorktreeBaseDirCandidates::new(
                git.root.clone(),
                git.worktree.parent().unwrap().to_path_buf(),
            )));
    fixture
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_service_commits_without_any_lifecycle_effect() {
    let git = GitFixture::new();
    let fixture = git_service(&git);
    reach_status(&fixture, CHANGE, "error").await;
    let before = git.head();

    let preview = fixture
        .service
        .preview_proposal_refresh(CHANGE, "develop")
        .await;
    assert!(preview.eligible, "{preview:?}");

    let outcome = fixture
        .service
        .execute(
            OperatorMode::Select,
            OperatorCommand::RefreshProposal {
                change_id: CHANGE.to_string(),
                source_ref: "develop".to_string(),
                expected_source: preview.source_commit.clone().unwrap(),
                expected_head: preview.candidate_head.clone().unwrap(),
            },
        )
        .await
        .expect("an eligible clean candidate refreshes");
    let OperatorOutcome::ProposalRefreshed(report) = outcome else {
        panic!("expected a refresh report, got {outcome:?}");
    };
    assert_eq!(report.outcome, Some(RefreshOutcome::Refreshed));
    assert_eq!(git.rev(&git.worktree, "HEAD^"), before);

    // The row is exactly as it was: still `error`, unmarked, not queued, and no
    // queue, hook, scheduler, or retry port was touched.
    assert_eq!(fixture.state.read().await.display_status(CHANGE), "error");
    assert!(!fixture.marks.is_marked(CHANGE));
    assert!(fixture.queue.added.lock().unwrap().is_empty());
    assert!(fixture.queue.removed.lock().unwrap().is_empty());
    assert!(fixture.queue.cancellations.lock().unwrap().is_empty());
    assert!(fixture.queue.explicit_retries.lock().unwrap().is_empty());
    assert_eq!(*fixture.queue.notified.lock().unwrap(), 0);
    assert!(fixture.hooks.adds().is_empty() && fixture.hooks.removes().is_empty());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_service_reports_a_hook_refusal_with_effects() {
    let git = GitFixture::new();
    let fixture = git_service(&git);
    reach_status(&fixture, CHANGE, "error").await;
    git.hook("exit 1");

    let error = fixture
        .service
        .refresh_proposal(&git.request())
        .await
        .expect_err("a rejecting hook refuses");
    let OperatorCommandError::RefreshRefused { report } = error else {
        panic!("expected a command-time refusal");
    };
    assert_eq!(report.refusal, Some(RefreshReason::HookRejected));
    assert!(report.effects.file_written && report.effects.staged);
    assert!(!report.effects.committed);
    assert_eq!(fixture.state.read().await.display_status(CHANGE), "error");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_ordinary_retry_never_refreshes() {
    let git = GitFixture::new();
    let fixture = git_service(&git);
    reach_status(&fixture, CHANGE, "error").await;
    let before = git.head();

    fixture
        .service
        .retry_change(CHANGE)
        .await
        .expect("an error row is retryable");

    assert_eq!(git.head(), before);
    assert_eq!(git.status(), "");
    assert_eq!(
        git.candidate_text(),
        crate::orchestration::proposal_refresh::tests::STALE_PROPOSAL
    );
}
