//! Real-filesystem tests for the post-quiescence reclamation boundary.
//!
//! The decision tests in the parent module prove every branch against in-memory
//! doubles. These prove the *observations* those branches consume are what a
//! real Unix filesystem actually reports: that `observe_link` refuses to follow
//! a symlink, that `observe_open_nofollow` binds evidence to a file object
//! rather than a pathname, and that device/inode replacement is visible at all.
//!
//! Evidence class: integration-scoped (real temporary filesystem), kept beside
//! the module because it is this boundary's platform contract it verifies. Each
//! case touches a handful of files in a temporary directory and never sleeps —
//! the dwell is injected — so they stay in the default fast tier.

#![cfg(unix)]

use std::path::Path;

use tempfile::TempDir;

use super::test_support::InstantDwellEnvironment;
use super::{
    reclaim_orphaned_index_lock, IndexLockReclaimOutcome, LockCandidate, LockCandidateState,
    PreDispatchLockObservation, RECLAIM_DWELL,
};
use crate::process_manager::ProcessGroupQuiescence;

fn absent_before(path: &Path) -> PreDispatchLockObservation {
    PreDispatchLockObservation::Observed {
        workspace: path.parent().unwrap().to_path_buf(),
        candidates: vec![LockCandidate {
            path: path.to_path_buf(),
            state: LockCandidateState::Absent,
        }],
    }
}

async fn reclaim(path: &Path, environment: &InstantDwellEnvironment) -> IndexLockReclaimOutcome {
    reclaim_orphaned_index_lock(
        absent_before(path),
        ProcessGroupQuiescence::Confirmed,
        environment,
    )
    .await
}

/// The happy path against a real file: an empty lock that nothing touches is
/// unlinked, and the pathname is genuinely gone afterwards.
#[tokio::test]
async fn a_real_empty_lock_is_observed_twice_and_unlinked() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"").unwrap();

    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(
        outcome,
        IndexLockReclaimOutcome::Reclaimed { path: lock.clone() }
    );
    assert_eq!(environment.dwells(), vec![RECLAIM_DWELL]);
    assert!(!lock.exists(), "the reclaimed lock must actually be gone");
}

/// A symlink is refused from the *first* observation, so the second one never
/// opens it. This is the case that would be catastrophic if metadata were read
/// through the link: the target could be any file in the repository.
#[tokio::test]
async fn a_symlinked_lock_is_refused_without_following_it() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real-empty-file");
    std::fs::write(&target, b"").unwrap();
    let lock = temp.path().join("index.lock");
    std::os::unix::fs::symlink(&target, &lock).unwrap();

    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(outcome.as_str(), "not_regular_file");
    assert!(
        outcome.diagnostics().contains("symlink"),
        "{}",
        outcome.diagnostics()
    );
    assert!(environment.dwells().is_empty(), "no dwell is paid");
    assert!(
        target.exists() && lock.symlink_metadata().is_ok(),
        "neither the link nor its target may be removed"
    );
}

/// `O_NOFOLLOW` is what makes the second observation refuse a pathname that
/// became a symlink during the dwell. Without it, `fstat` would describe the
/// target and the identity check could pass on the wrong file.
#[tokio::test]
async fn a_lock_replaced_by_a_symlink_during_the_dwell_is_refused() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"").unwrap();
    let target = temp.path().join("swapped-target");
    std::fs::write(&target, b"").unwrap();

    let swap = {
        let lock = lock.clone();
        let target = target.clone();
        move || {
            std::fs::remove_file(&lock).unwrap();
            std::os::unix::fs::symlink(&target, &lock).unwrap();
        }
    };
    let environment = InstantDwellEnvironment::mutating_during_dwell(swap);

    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(
        outcome.as_str(),
        "observation_failed",
        "O_NOFOLLOW must fail the open rather than describe the target: {}",
        outcome.diagnostics()
    );
    assert!(outcome.diagnostics().contains("second"));
    assert!(target.exists(), "the symlink target must be untouched");
    assert!(
        lock.symlink_metadata().unwrap().file_type().is_symlink(),
        "the swapped-in link must remain"
    );
}

/// Recreating the lock during the dwell yields a different inode. That is the
/// signal that another actor is live on this pathname, and it must lose.
#[tokio::test]
async fn an_inode_replacement_during_the_dwell_is_refused_against_a_real_file() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"").unwrap();

    let recreate = {
        let lock = lock.clone();
        move || {
            std::fs::remove_file(&lock).unwrap();
            std::fs::write(&lock, b"").unwrap();
        }
    };
    let environment = InstantDwellEnvironment::mutating_during_dwell(recreate);

    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(
        outcome.as_str(),
        "identity_changed",
        "{}",
        outcome.diagnostics()
    );
    assert!(lock.exists(), "the replacement lock must survive");
}

/// A lock that gains content during the dwell belongs to a live writer.
#[tokio::test]
async fn content_written_during_the_dwell_is_refused() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"").unwrap();

    let write = {
        let lock = lock.clone();
        move || {
            std::fs::write(&lock, b"DIRC live index write").unwrap();
        }
    };
    let environment = InstantDwellEnvironment::mutating_during_dwell(write);

    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(
        outcome.as_str(),
        "identity_changed",
        "{}",
        outcome.diagnostics()
    );
    assert_eq!(
        std::fs::read(&lock).unwrap(),
        b"DIRC live index write",
        "the live writer's content must survive untouched"
    );
}

/// A lock that already carries content when the boundary first looks at it is
/// refused before the dwell, so a partially written index is never removed.
#[tokio::test]
async fn a_non_empty_real_lock_is_refused_before_the_dwell() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"DIRC").unwrap();

    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(outcome.as_str(), "non_zero_length");
    assert!(outcome.diagnostics().contains("4 bytes"));
    assert!(environment.dwells().is_empty());
    assert!(lock.exists());
}

/// A directory at the lock pathname is not a lock file, and `remove_file` on it
/// would fail anyway — refusing early keeps the diagnostic honest.
#[tokio::test]
async fn a_directory_at_the_lock_path_is_refused() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::create_dir(&lock).unwrap();

    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(outcome.as_str(), "not_regular_file");
    assert!(
        !outcome.diagnostics().contains("symlink"),
        "a directory is not a symlink: {}",
        outcome.diagnostics()
    );
    assert!(lock.is_dir(), "the directory must be left in place");
}

/// Another actor removing the same residue reaches the state this boundary
/// exists to produce, so it is convergence rather than a failed deletion.
#[tokio::test]
async fn a_lock_that_disappears_during_the_dwell_converges_naturally() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");
    std::fs::write(&lock, b"").unwrap();

    let remove = {
        let lock = lock.clone();
        move || {
            std::fs::remove_file(&lock).unwrap();
        }
    };
    let environment = InstantDwellEnvironment::mutating_during_dwell(remove);

    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(
        outcome,
        IndexLockReclaimOutcome::NaturallyConverged { path: lock.clone() }
    );
    assert!(!lock.exists());
}

/// No residue at all is the ordinary case, and it must cost neither a dwell nor
/// an unlink.
#[tokio::test]
async fn an_absent_lock_is_not_present_rather_than_reclaimed() {
    let temp = TempDir::new().unwrap();
    let lock = temp.path().join("index.lock");

    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;

    assert_eq!(outcome, IndexLockReclaimOutcome::NotPresent);
    assert!(environment.dwells().is_empty());
}

/// An unlink the filesystem refuses is a refusal, not a silent success: a
/// read-only parent directory is the realistic form of that failure.
#[tokio::test]
async fn an_unlink_the_filesystem_refuses_is_reported_as_a_refusal() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let git_dir = temp.path().join("git-dir");
    std::fs::create_dir(&git_dir).unwrap();
    let lock = git_dir.join("index.lock");
    std::fs::write(&lock, b"").unwrap();

    // Running as root defeats directory permissions entirely, so the case has
    // nothing to prove there.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }

    std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let environment = InstantDwellEnvironment::new();
    let outcome = reclaim(&lock, &environment).await;
    // Restore write permission before any assertion so the temporary directory
    // can always be cleaned up.
    std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert_eq!(
        outcome.as_str(),
        "unlink_failed",
        "{}",
        outcome.diagnostics()
    );
    assert!(lock.exists(), "the lock must survive a failed unlink");
}

/// A pre-dispatch capture against a real Git worktree resolves the real
/// `index.lock` pathname and reports it absent, which is the only state that
/// can later authorize reclamation.
#[tokio::test]
async fn pre_dispatch_capture_reads_a_real_git_worktree() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test"],
    ] {
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(repo)
            .output()
            .expect("git should run");
        assert!(output.status.success(), "git {args:?} failed");
    }

    let environment = InstantDwellEnvironment::new();
    let observation = PreDispatchLockObservation::capture(&environment, repo, true).await;

    let PreDispatchLockObservation::Observed { candidates, .. } = &observation else {
        panic!("a real worktree must resolve candidates, got {observation:?}");
    };
    assert!(!candidates.is_empty());
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.state == LockCandidateState::Absent),
        "a fresh repository holds no lock: {candidates:?}"
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.path.ends_with("index.lock")),
        "every candidate names an index.lock: {candidates:?}"
    );

    // The same capture, run against the same worktree while a lock exists,
    // reports pre-existence — which is what removes reclamation authority.
    let lock = candidates[0].path.clone();
    std::fs::write(&lock, b"").unwrap();
    let observation = PreDispatchLockObservation::capture(&environment, repo, true).await;
    let PreDispatchLockObservation::Observed { candidates, .. } = &observation else {
        panic!("expected observed candidates");
    };
    assert!(
        candidates
            .iter()
            .any(|candidate| matches!(candidate.state, LockCandidateState::Present { .. })),
        "an existing lock must be seen before the dispatch: {candidates:?}"
    );

    let outcome =
        reclaim_orphaned_index_lock(observation, ProcessGroupQuiescence::Confirmed, &environment)
            .await;
    assert_eq!(outcome.as_str(), "pre_existing_lock");
    assert!(lock.exists(), "a pre-existing lock is never removed");
}
