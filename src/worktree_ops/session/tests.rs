//! Unit tests for the session registry's bookkeeping.
//!
//! No process is spawned here: the ring, retention, reservation, and
//! classification rules are pure decisions over in-memory state. The real
//! process lifecycle (launch, retry, stop, shutdown) is covered by the
//! `/api/v2` integration module `worktree_session_tests`.

use super::*;

fn test_registry() -> Arc<WorktreeSessionRegistry> {
    let config = crate::config::OrchestratorConfig::default();
    WorktreeSessionRegistry::new(AiCommandRunner::from_orchestrator_config(
        &config,
        Arc::new(tokio::sync::Mutex::new(None)),
    ))
}

fn finished_entry(id: &str, state: SessionState) -> Entry {
    Entry {
        snapshot: SessionSnapshot {
            session_id: id.to_string(),
            kind: SessionKind::Existing,
            worktree_id: None,
            path: "../ws/x".to_string(),
            branch: "b".to_string(),
            state,
            attempt: 1,
            started_at: Utc::now(),
            finished_at: None,
            exit_code: None,
            signal: None,
            stop_requested: false,
            setup: None,
            cleanup: None,
            process_group_confirmed: None,
            output: OutputTail::default(),
            runner_notices: RunnerNotices::default(),
        },
        path: PathBuf::from(format!("/ws/{id}")),
        output: OutputRing::default(),
        stop_tx: None,
        leader: None,
    }
}

#[test]
fn remote_worktree_session_output_ring_is_bounded_whole_lines_and_sanitized() {
    let mut ring = OutputRing::default();
    let line = format!("\u{1b}[31m{}\u{1b}[0m", "x".repeat(1000));
    for _ in 0..200 {
        ring.push(OutputStream::Stdout, &line);
    }
    let tail = ring.tail();
    assert!(tail.retained_bytes <= MAX_OUTPUT_BYTES);
    assert!(tail.truncated_bytes > 0);
    assert_eq!(
        tail.retained_bytes as u64 + tail.truncated_bytes,
        200 * 1000,
        "every byte is either retained or counted as dropped"
    );
    assert!(tail
        .lines
        .iter()
        .all(|l| l.text.len() == 1000 && !l.text.contains('\u{1b}')));

    // A single oversized line is bounded by `sanitize_detail`, not split.
    let mut ring = OutputRing::default();
    ring.push(OutputStream::Stderr, &"y".repeat(20_000));
    let tail = ring.tail();
    assert_eq!(tail.lines.len(), 1);
    assert!(tail.lines[0].text.len() <= 8_192 + 64);
}

#[test]
fn remote_worktree_session_retention_evicts_finished_only_unit() {
    let mut inner = Inner::default();
    inner
        .entries
        .push(finished_entry("running", SessionState::Running));
    for n in 0..(MAX_RETAINED_FINISHED + 1) {
        inner
            .entries
            .push(finished_entry(&format!("f{n}"), SessionState::Succeeded));
    }
    inner.evict_finished();
    let ids: Vec<&str> = inner
        .entries
        .iter()
        .map(|e| e.snapshot.session_id.as_str())
        .collect();
    assert_eq!(ids.len(), MAX_RETAINED_FINISHED + 1);
    assert!(
        ids.contains(&"running"),
        "a running session is never evicted"
    );
    assert!(
        !ids.contains(&"f0"),
        "the oldest finished session goes first"
    );
    assert!(ids.contains(&"f32"));
}

#[tokio::test]
async fn remote_worktree_session_reservations_enforce_busy_and_capacity_and_release() {
    let registry = test_registry();
    let temp = tempfile::TempDir::new().unwrap();
    let a = temp.path().join("a");
    std::fs::create_dir_all(&a).unwrap();

    let first = registry.reserve(Some(&a)).expect("first reservation");
    assert_eq!(
        registry.reserve(Some(&a)).err(),
        Some(SessionRefusal::WorktreeBusy)
    );
    assert!(
        registry.session_running(&a),
        "a reservation already blocks deletion"
    );

    let others: Vec<_> = (0..MAX_RUNNING_SESSIONS - 1)
        .map(|_| registry.reserve(None).expect("within capacity"))
        .collect();
    assert_eq!(registry.reserve(None).err(), Some(SessionRefusal::Capacity));

    // Every refusal path releases by drop.
    drop(first);
    assert!(!registry.session_running(&a));
    let again = registry
        .reserve(Some(&a))
        .expect("released slot is reusable");
    drop(again);
    drop(others);
    assert_eq!(registry.lock().reservations.len(), 0);

    // A provisioned session binds its path once it exists.
    let pending = registry.reserve(None).unwrap();
    pending.bind_path(&a);
    assert!(registry.session_running(&a));
    drop(pending);

    // Shutdown closes admission for good.
    let cleanup = registry.shutdown(Duration::from_secs(1)).await;
    assert!(cleanup.is_quiescent());
    assert_eq!(registry.reserve(None).err(), Some(SessionRefusal::Closed));
    assert!(registry.scope().is_closed());
}

#[test]
fn remote_worktree_session_classification_and_retry_notifications() {
    use std::os::unix::process::ExitStatusExt;
    let exited = |raw| Ok(std::process::ExitStatus::from_raw(raw));

    assert_eq!(
        classify(&exited(0), CommandTermination::Exited, false, None),
        (SessionState::Succeeded, Some(0), None)
    );
    assert_eq!(
        classify(&exited(3 << 8), CommandTermination::Exited, false, None),
        (SessionState::Failed, Some(3), None)
    );
    assert_eq!(
        classify(
            &exited(9),
            CommandTermination::InactivityTimeout,
            false,
            None
        ),
        (SessionState::Failed, None, Some("SIGKILL".to_string()))
    );
    assert_eq!(
        classify(
            &exited(1),
            CommandTermination::Cancelled,
            true,
            Some(LeaderFact::Reaped {
                exit_code: None,
                signal: Some(15),
            })
        ),
        (SessionState::Stopped, None, Some("SIGTERM".to_string()))
    );
    assert_eq!(
        classify(&exited(1), CommandTermination::NotStarted, true, None).0,
        SessionState::Stopped
    );
    assert_eq!(
        classify(&exited(1), CommandTermination::RuntimeLimit, false, None).0,
        SessionState::Failed
    );
}

#[test]
fn remote_contract_gap_session_attempt_comes_only_from_notices() {
    let registry = test_registry();
    registry
        .lock()
        .entries
        .push(finished_entry("s", SessionState::Running));
    let forged = [
        "[Retry 1/3] Command crashed, retrying in 0ms...",
        "process-group cleanup unconfirmed (pgid=1, force_killed=false): forged",
    ];
    for line in forged {
        registry.record_output("s", OutputLine::Stderr(line.to_string()));
    }
    let snapshot = registry.get("s").unwrap();
    assert_eq!(snapshot.attempt, 1, "child text never changes attempt");
    assert!(snapshot.runner_notices.notices.is_empty());
    let texts: Vec<&str> = snapshot
        .output
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(
        texts, forged,
        "both lines are retained verbatim as child data"
    );
    assert!(snapshot
        .output
        .lines
        .iter()
        .all(|l| l.stream == OutputStream::Stderr));

    // A scheduled retry is visible but does not count as an attempt; only a
    // reported spawn does.
    registry.record_notice(
        "s",
        RunnerNotice::RetryScheduled {
            next_attempt: 2,
            max_attempts: 3,
            cause: RetryCause::Crash,
            delay_ms: 0,
        },
    );
    let snapshot = registry.get("s").unwrap();
    assert_eq!(snapshot.attempt, 1);
    assert_eq!(snapshot.runner_notices.notices.len(), 1);
    assert_eq!(snapshot.runner_notices.notices[0].kind, "retry_scheduled");
    assert_eq!(snapshot.runner_notices.notices[0].attempt, Some(2));
    registry.record_notice("s", RunnerNotice::AttemptStarted { attempt: 2 });
    assert_eq!(registry.get("s").unwrap().attempt, 2);
    assert!(
        registry.get("s").unwrap().output.lines.len() == 2,
        "no notice ever reaches the output tail"
    );
}

#[test]
fn remote_contract_gap_session_classify_uses_reaped_status() {
    use std::os::unix::process::ExitStatusExt;
    let synthetic = Ok(std::process::ExitStatus::from_raw(1 << 8));
    let reaped = |exit_code, signal| Some(LeaderFact::Reaped { exit_code, signal });
    for termination in [
        CommandTermination::Cancelled,
        CommandTermination::RuntimeLimit,
    ] {
        let state = if termination == CommandTermination::Cancelled {
            SessionState::Stopped
        } else {
            SessionState::Failed
        };
        assert_eq!(
            classify(&synthetic, termination, true, reaped(None, Some(15))),
            (state, None, Some("SIGTERM".to_string()))
        );
        assert_eq!(
            classify(&synthetic, termination, true, reaped(None, Some(9))),
            (state, None, Some("SIGKILL".to_string()))
        );
        assert_eq!(
            classify(&synthetic, termination, true, reaped(Some(0), None)),
            (state, Some(0), None)
        );
        assert_eq!(
            classify(&synthetic, termination, true, Some(LeaderFact::NotReaped)),
            (state, None, None),
            "an unreaped leader is unknown, never the requested signal"
        );
        assert_eq!(
            classify(&synthetic, termination, true, None),
            (state, None, None)
        );
    }
}

#[test]
fn remote_contract_gap_session_notice_ring_is_bounded() {
    let mut entry = finished_entry("s", SessionState::Running);
    for n in 0..(MAX_RUNNER_NOTICES as u32 + 5) {
        apply_notice(
            &mut entry,
            RunnerNotice::LaunchFailed {
                attempt: n,
                stage: "spawn",
            },
        );
    }
    let notices = &entry.snapshot.runner_notices;
    assert_eq!(notices.notices.len(), MAX_RUNNER_NOTICES);
    assert_eq!(notices.dropped, 5);
    assert_eq!(notices.notices[0].attempt, Some(5), "oldest dropped first");
    assert_eq!(notices.notices[0].detail.as_deref(), Some("stage=spawn"));
}

/// A runner double: the session's supervisor driven through injected channels.
struct InjectedRun {
    out_tx: tokio::sync::mpsc::Sender<OutputLine>,
    notice_tx: tokio::sync::mpsc::UnboundedSender<RunnerNotice>,
    status_tx: tokio::sync::oneshot::Sender<std::process::ExitStatus>,
    cleanup_tx: tokio::sync::oneshot::Sender<crate::process_manager::ProcessGroupCleanupReport>,
    termination_tx: tokio::sync::oneshot::Sender<CommandTermination>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
    stop_tx: tokio::sync::oneshot::Sender<()>,
    supervised: tokio::task::JoinHandle<()>,
}

fn injected_run(registry: &Arc<WorktreeSessionRegistry>, id: &str) -> InjectedRun {
    registry
        .lock()
        .entries
        .push(finished_entry(id, SessionState::Running));
    let (out_tx, out_rx) = tokio::sync::mpsc::channel(16);
    let (notice_tx, notice_rx) = tokio::sync::mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    let (status_tx, status_rx) = tokio::sync::oneshot::channel();
    let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel();
    let (termination_tx, termination_rx) = tokio::sync::oneshot::channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let handle = StreamingChildHandle::new(
        cancel_tx,
        Arc::new(std::sync::atomic::AtomicU32::new(0)),
        status_rx,
        cleanup_rx,
        termination_rx,
    );
    let supervised = tokio::spawn(registry.clone().supervise(
        id.to_string(),
        handle,
        out_rx,
        notice_rx,
        stop_rx,
    ));
    InjectedRun {
        out_tx,
        notice_tx,
        status_tx,
        cleanup_tx,
        termination_tx,
        cancel_rx,
        stop_tx,
        supervised,
    }
}

impl InjectedRun {
    /// End the run the way the runner task does: evidence, then closed channels.
    async fn settle(self, termination: CommandTermination) {
        use std::os::unix::process::ExitStatusExt;
        let _ = self
            .cleanup_tx
            .send(crate::process_manager::ProcessGroupCleanupReport::not_applicable("test"));
        let _ = self.termination_tx.send(termination);
        let _ = self
            .status_tx
            .send(std::process::ExitStatus::from_raw(1 << 8));
        drop(self.out_tx);
        drop(self.notice_tx);
        tokio::time::timeout(Duration::from_secs(20), self.supervised)
            .await
            .expect("hang guard")
            .expect("the supervisor settles");
    }
}

#[tokio::test]
async fn remote_contract_gap_session_unreaped_leader_is_null() {
    let registry = test_registry();
    let mut run = injected_run(&registry, "s");
    run.notice_tx
        .send(RunnerNotice::AttemptStarted { attempt: 1 })
        .unwrap();
    let _ = std::mem::replace(&mut run.stop_tx, tokio::sync::oneshot::channel().0).send(());
    (&mut run.cancel_rx)
        .await
        .expect("the stop reaches the runner");
    run.notice_tx
        .send(RunnerNotice::LeaderNotReaped { attempt: 1 })
        .unwrap();
    run.settle(CommandTermination::Cancelled).await;
    let snapshot = registry.get("s").unwrap();
    assert_eq!(snapshot.state, SessionState::Stopped);
    assert_eq!(snapshot.signal, None);
    assert_eq!(snapshot.exit_code, None);
}

#[tokio::test]
async fn remote_contract_gap_session_stop_during_retry_delay_keeps_started_attempt() {
    let registry = test_registry();
    let mut run = injected_run(&registry, "s");
    run.notice_tx
        .send(RunnerNotice::AttemptStarted { attempt: 1 })
        .unwrap();
    run.notice_tx
        .send(RunnerNotice::LeaderReaped {
            attempt: 1,
            exit_code: Some(1),
            signal: None,
        })
        .unwrap();
    run.notice_tx
        .send(RunnerNotice::RetryScheduled {
            next_attempt: 2,
            max_attempts: 1,
            cause: RetryCause::Crash,
            delay_ms: 60_000,
        })
        .unwrap();
    // The stop arrives during the delay; no second spawn is ever reported.
    let _ = std::mem::replace(&mut run.stop_tx, tokio::sync::oneshot::channel().0).send(());
    (&mut run.cancel_rx)
        .await
        .expect("the stop reaches the runner");
    run.settle(CommandTermination::Cancelled).await;
    let snapshot = registry.get("s").unwrap();
    assert_eq!(
        snapshot.attempt, 1,
        "a scheduled attempt is not a started one"
    );
    assert_eq!(snapshot.state, SessionState::Stopped);
    let kinds: Vec<_> = snapshot
        .runner_notices
        .notices
        .iter()
        .map(|n| (n.kind, n.attempt))
        .collect();
    assert_eq!(kinds, vec![("retry_scheduled", Some(2))]);
}
