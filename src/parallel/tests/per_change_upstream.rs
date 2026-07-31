//! Base-lane coverage for per-change upstream publication.
//!
//! These tests exercise the repository-evidence decisions the base lane makes:
//! whether a completed result may enter cumulative base, whether a retry must
//! resume publication instead of merging again, and whether a disabled run
//! observes anything at all. They use a real temporary Git repository because
//! the evidence *is* Git state — that makes them integration-scoped, not unit
//! tests, and they are kept small enough to stay in the default suite.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::OrchestratorConfig;
use crate::parallel::ParallelExecutor;
use crate::upstream::publication::{format_publication_marker_message, parse_publication_trailers};
use crate::upstream::{UpstreamIntegrationConfig, UpstreamRuntime};

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// A minimal repository on branch `main` with one commit, or `None` when git is
/// unavailable in this environment.
fn repo() -> Option<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir().ok()?;
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-b", "main"])?;
    git(&root, &["config", "user.email", "test@example.com"])?;
    git(&root, &["config", "user.name", "Test User"])?;
    git(&root, &["config", "commit.gpgsign", "false"])?;
    std::fs::write(root.join("README.md"), "# base\n").ok()?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-m", "Initial commit"])?;
    Some((dir, root))
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    }
}

fn runtime() -> UpstreamRuntime {
    UpstreamRuntime {
        config: UpstreamIntegrationConfig::new("origin", "cargo test"),
        branch: "main".to_string(),
    }
}

fn stagger() -> crate::ai_command_runner::SharedStaggerState {
    std::sync::Arc::new(tokio::sync::Mutex::new(None))
}

fn executor(root: &Path, enabled: bool) -> ParallelExecutor {
    let mut executor = ParallelExecutor::new(root.to_path_buf(), test_config(), None);
    if enabled {
        executor.set_upstream_integration(runtime(), stagger());
    }
    executor
}

/// Record a marker exactly as the base lane does after local integration.
fn mark_publication_required(root: &Path, change_id: &str) -> String {
    let message = format_publication_marker_message(change_id, "origin", "main");
    git(root, &["commit", "--allow-empty", "-m", &message]).expect("marker commit");
    git(root, &["rev-parse", "HEAD"]).expect("head")
}

#[tokio::test]
async fn per_change_upstream_marker_binds_change_remote_and_branch() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    let executor = executor(&root, true);

    executor
        .record_publication_intent("alpha")
        .await
        .expect("marker recorded");

    let message = git(&root, &["log", "-1", "--format=%B"]).expect("message");
    let trailers = parse_publication_trailers(&message).expect("publication trailers");
    assert_eq!(trailers.change_id, "alpha");
    assert_eq!(trailers.remote, "origin");
    assert_eq!(trailers.branch, "main");

    // The marker is durable, repository-visible evidence: a fresh scan finds it
    // without any process memory.
    let pending = executor.pending_publications().await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].trailers.change_id, "alpha");
}

#[tokio::test]
async fn per_change_upstream_marker_is_recorded_even_without_an_upstream_merge() {
    // A run whose remote never advanced creates no upstream merge commit, so the
    // publication marker is the *only* thing that distinguishes an unpublished
    // opted-in integration from ordinary terminal `merged` history.
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    let executor = executor(&root, true);

    executor
        .record_publication_intent("alpha")
        .await
        .expect("marker recorded");

    let log = git(&root, &["log", "--format=%s"]).expect("log");
    assert!(
        !log.contains("Merge upstream:"),
        "no upstream merge was created: {}",
        log
    );
    assert!(!executor.pending_publications().await.is_empty());
}

#[tokio::test]
async fn per_change_upstream_pending_publication_blocks_a_later_result() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    let executor = executor(&root, true);
    mark_publication_required(&root, "alpha");

    // A later completed result must wait before cumulative-base integration
    // while `alpha` is unpublished.
    assert_eq!(
        executor.blocking_publication_change("beta").await,
        Some("alpha".to_string())
    );
    // `alpha` itself is not blocked by its own marker; it resumes publication.
    assert_eq!(executor.blocking_publication_change("alpha").await, None);
    assert!(executor.has_pending_publication_for("alpha").await);
    assert!(!executor.has_pending_publication_for("beta").await);
}

#[tokio::test]
async fn per_change_upstream_published_marker_stops_blocking() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    let executor = executor(&root, true);
    let marker = mark_publication_required(&root, "alpha");

    // Remote reachability is what ends the wait. Simulating the remote-tracking
    // ref is exactly what a real fetch after a confirmed push leaves behind.
    git(
        &root,
        &["update-ref", "refs/remotes/origin/main", marker.as_str()],
    )
    .expect("tracking ref");

    assert!(executor.pending_publications().await.is_empty());
    assert_eq!(executor.blocking_publication_change("beta").await, None);
    assert!(!executor.has_pending_publication_for("alpha").await);
}

#[tokio::test]
async fn per_change_upstream_disabled_run_observes_no_publication_evidence() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    // Even with a marker present in history, a disabled executor performs no
    // publication observation at all: default-off is a hard boundary.
    mark_publication_required(&root, "alpha");
    let executor = executor(&root, false);

    assert!(!executor.has_upstream_integration());
    assert!(executor.pending_publications().await.is_empty());
    assert_eq!(executor.blocking_publication_change("beta").await, None);
}

#[tokio::test]
async fn per_change_upstream_ordinary_merge_history_is_never_publication_work() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    let executor = executor(&root, true);

    // A disabled-mode cumulative integration commit carries no marker.
    git(&root, &["checkout", "-b", "wt-alpha"]).expect("branch");
    std::fs::write(root.join("work.txt"), "work\n").expect("write");
    git(&root, &["add", "."]).expect("add");
    git(&root, &["commit", "-m", "Apply: alpha"]).expect("commit");
    git(&root, &["checkout", "main"]).expect("checkout");
    git(
        &root,
        &["merge", "--no-ff", "-m", "Merge change: alpha", "wt-alpha"],
    )
    .expect("merge");

    assert!(
        executor.pending_publications().await.is_empty(),
        "terminal `merged` history must never be promoted to publication work"
    );
}

/// A repository on `main` with a real local bare remote `origin`.
fn repo_with_remote() -> Option<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir().ok()?;
    let root = dir.path().join("repo");
    let remote = dir.path().join("remote.git");
    std::fs::create_dir_all(&root).ok()?;
    git(dir.path(), &["init", "--bare", "-b", "main", "remote.git"])?;
    git(&root, &["init", "-b", "main"])?;
    git(&root, &["config", "user.email", "test@example.com"])?;
    git(&root, &["config", "user.name", "Test User"])?;
    git(&root, &["config", "commit.gpgsign", "false"])?;
    std::fs::write(root.join("README.md"), "# base\n").ok()?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-m", "Initial commit"])?;
    git(&root, &["remote", "add", "origin", remote.to_str()?])?;
    git(&root, &["push", "-u", "origin", "main"])?;
    Some((dir, root))
}

/// Runtime whose verification command is a real, trivially passing process.
fn passing_runtime() -> UpstreamRuntime {
    UpstreamRuntime {
        config: UpstreamIntegrationConfig::new("origin", "exit 0"),
        branch: "main".to_string(),
    }
}

#[tokio::test]
async fn per_change_upstream_retry_resumes_publication_without_apply_dispatch() {
    let Some((_dir, root)) = repo_with_remote() else {
        println!("Skipping test: git not available");
        return;
    };

    // Resumption takes the project base lane, so this test must not run beside
    // another test that owns it.
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    // A prior attempt integrated the change and recorded its marker, then died
    // before publishing. Only repository evidence survives.
    mark_publication_required(&root, "alpha");
    let head = git(&root, &["rev-parse", "HEAD"]).expect("head");

    let mut executor = ParallelExecutor::new(root.clone(), test_config(), None);
    executor.set_upstream_integration(passing_runtime(), stagger());
    assert!(executor.has_pending_publication_for("alpha").await);

    // This is the explicit-retry / restart entry point. It performs publication
    // work only: no apply or acceptance dispatch is created for `alpha`.
    executor.resume_pending_publications().await;

    let remote_head = git(&root, &["ls-remote", "origin", "refs/heads/main"]).expect("ls-remote");
    assert!(
        remote_head.starts_with(&head),
        "resumed publication must reach the remote: {}",
        remote_head
    );
    assert!(
        !executor.has_pending_publication_for("alpha").await,
        "confirmed publication releases the base lane for waiting results"
    );
    assert_eq!(executor.blocking_publication_change("beta").await, None);
}

#[tokio::test]
async fn per_change_upstream_disabled_executor_resumes_nothing() {
    let Some((_dir, root)) = repo_with_remote() else {
        println!("Skipping test: git not available");
        return;
    };
    mark_publication_required(&root, "alpha");
    let remote_before = git(&root, &["ls-remote", "origin", "refs/heads/main"]).expect("ls-remote");

    let mut executor = ParallelExecutor::new(root.clone(), test_config(), None);
    executor.resume_pending_publications().await;

    assert_eq!(
        git(&root, &["ls-remote", "origin", "refs/heads/main"]).expect("ls-remote"),
        remote_before,
        "a disabled run must never push"
    );
}

#[tokio::test]
async fn per_change_upstream_later_result_is_deferred_at_the_merge_boundary() {
    let Some((_dir, root)) = repo() else {
        println!("Skipping test: git not available");
        return;
    };
    // `alpha` owns the base lane with an unconfirmed publication.
    mark_publication_required(&root, "alpha");
    let head_before = git(&root, &["rev-parse", "HEAD"]).expect("head");

    let executor = executor(&root, true);
    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;

    // `beta` is ready for cumulative-base integration and must wait, not merge.
    let outcome = executor
        .attempt_merge(
            &["wt-beta".to_string()],
            &["beta".to_string()],
            &[root.join("nonexistent-workspace")],
        )
        .await
        .expect("attempt_merge");

    match outcome {
        crate::parallel::merge::MergeAttempt::Deferred(deferred) => {
            assert!(
                deferred.reason.contains("alpha"),
                "the wait must name the unpublished change: {}",
                deferred.reason
            );
            assert!(
                deferred.auto_resumable,
                "the wait resumes automatically once publication is confirmed"
            );
        }
        other => panic!("expected a deferral, got {:?}", other),
    }

    assert_eq!(
        git(&root, &["rev-parse", "HEAD"]).expect("head"),
        head_before,
        "a waiting result must not enter cumulative base"
    );
}

/// A shell fragment that appends `<label> <base-head> <remote-head>` to `log`.
///
/// Recording the remote head alongside the base head at each step is what makes
/// ordering observable without a Git hook: a step that ran before publication
/// still sees the pre-publication remote tip.
fn ordering_probe(label: &str, root: &Path, log: &Path) -> String {
    let root = root.display();
    let log = log.display();
    format!(
        "printf '{label} %s %s\\n' \
         \"$(git -C '{root}' rev-parse HEAD)\" \
         \"$(git -C '{root}' ls-remote origin refs/heads/main | cut -f1)\" >> '{log}'"
    )
}

/// Parsed `<label> <base-head> <remote-head>` observation.
struct OrderingStep {
    label: String,
    base_head: String,
    remote_head: String,
}

fn read_ordering_log(log: &Path) -> Vec<OrderingStep> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split_whitespace();
            OrderingStep {
                label: fields.next().unwrap_or_default().to_string(),
                base_head: fields.next().unwrap_or_default().to_string(),
                remote_head: fields.next().unwrap_or_default().to_string(),
            }
        })
        .collect()
}

#[tokio::test]
async fn per_change_upstream_base_lane_orders_merge_hook_verification_and_publication() {
    // The opted-in base lane's ordering is the contract, not an implementation
    // detail of `publish_base_integration`: local integration, then `on_merged`,
    // then complete verification, then native push and remote confirmation.
    // Driving a real post-archive merge and observing what each step could see
    // makes a reordering fail here instead of only reading wrong.
    let Some((dir, root)) = repo_with_remote() else {
        println!("Skipping test: git not available");
        return;
    };

    let remote_before =
        git(&root, &["ls-remote", "origin", "refs/heads/main"]).expect("initial ls-remote");
    let remote_before = remote_before
        .split_whitespace()
        .next()
        .expect("initial remote head")
        .to_string();

    // A completed, archived change waiting in its own worktree.
    let workspace_base = dir.path().join("workspaces");
    std::fs::create_dir_all(&workspace_base).expect("workspace base");
    let workspace_path = workspace_base.join("ws-alpha");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-b",
            "ws-alpha",
            workspace_path.to_str().expect("workspace path"),
            "HEAD",
        ],
    )
    .expect("worktree");
    let archive_dir = workspace_path.join("openspec/changes/archive/alpha");
    std::fs::create_dir_all(&archive_dir).expect("archive dir");
    std::fs::write(archive_dir.join("proposal.md"), "# archived alpha\n").expect("archive file");
    git(&workspace_path, &["add", "-A"]).expect("stage archive");
    git(&workspace_path, &["commit", "-m", "Archive: alpha"]).expect("archive commit");
    let change_revision = git(&workspace_path, &["rev-parse", "HEAD"]).expect("change revision");

    let log = dir.path().join("base-lane-order.log");
    let config = OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        // The sequential-merge step delegates the merge itself to the resolve
        // command; a deterministic shell merge stands in for the agent here.
        resolve_command: Some("git merge --no-ff -m 'Merge change: alpha' ws-alpha".to_string()),
        // The command-queue pacing exists for real agent processes; a shell merge
        // needs none of it, and paying for it would push this test out of the
        // default suite.
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..test_config()
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let mut executor = ParallelExecutor::new(root.clone(), config, Some(tx));
    executor.set_hooks(crate::hooks::HookRunner::new(
        crate::hooks::HooksConfig {
            on_merged: Some(crate::hooks::HookConfigValue::Simple(ordering_probe(
                "on_merged",
                &root,
                &log,
            ))),
            ..Default::default()
        },
        root.clone(),
    ));
    executor.set_upstream_integration(
        UpstreamRuntime {
            config: UpstreamIntegrationConfig::new("origin", ordering_probe("verify", &root, &log)),
            branch: "main".to_string(),
        },
        stagger(),
    );

    let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
    let outcome = executor
        .attempt_merge(
            &["ws-alpha".to_string()],
            &["alpha".to_string()],
            &[workspace_path.clone()],
        )
        .await
        .expect("attempt_merge");
    assert!(
        matches!(outcome, crate::parallel::merge::MergeAttempt::Merged { .. }),
        "an archived opted-in result integrates and publishes: {:?}",
        outcome
    );

    let base_head = git(&root, &["rev-parse", "HEAD"]).expect("base head");
    let remote_after =
        git(&root, &["ls-remote", "origin", "refs/heads/main"]).expect("final ls-remote");
    let remote_after = remote_after
        .split_whitespace()
        .next()
        .expect("final remote head")
        .to_string();

    let steps = read_ordering_log(&log);
    let labels: Vec<&str> = steps.iter().map(|step| step.label.as_str()).collect();
    assert_eq!(
        labels.first(),
        Some(&"on_merged"),
        "the hook runs before any verification: {:?}",
        labels
    );
    assert!(
        labels.len() >= 2 && labels[1..].iter().all(|label| *label == "verify"),
        "every step after the hook is complete verification: {:?}",
        labels
    );

    // `on_merged` observed a base that already contains the archived change, so
    // local integration precedes the hook.
    assert!(
        git(
            &root,
            &[
                "merge-base",
                "--is-ancestor",
                change_revision.as_str(),
                steps[0].base_head.as_str(),
            ],
        )
        .is_some(),
        "on_merged must run after the change is in cumulative base"
    );
    // ...and a remote that has not been published to, so no push preceded it.
    assert_eq!(
        steps[0].remote_head, remote_before,
        "nothing may be published before on_merged"
    );

    // Verification ran against the exact cumulative HEAD that was then published,
    // and still before the push.
    for step in &steps[1..] {
        assert_eq!(
            step.base_head, base_head,
            "verification must cover the published cumulative HEAD"
        );
        assert_eq!(
            step.remote_head, remote_before,
            "the native push must follow verification"
        );
    }

    // Publication and remote confirmation closed the sequence.
    assert_eq!(
        remote_after, base_head,
        "the confirmed remote branch must contain the published cumulative HEAD"
    );

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    let push_started = events
        .iter()
        .position(|event| matches!(event, crate::parallel::ParallelEvent::PushStarted { .. }))
        .expect("PushStarted");
    let push_completed = events
        .iter()
        .position(|event| matches!(event, crate::parallel::ParallelEvent::PushCompleted { .. }))
        .expect("PushCompleted");
    assert!(
        push_started < push_completed,
        "publication progress precedes confirmation"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, crate::parallel::ParallelEvent::MergeCompleted { .. })),
        "opted-in local integration is not terminal `merged`"
    );
}
