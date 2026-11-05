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
        },
        path: PathBuf::from(format!("/ws/{id}")),
        output: OutputRing::default(),
        stop_tx: None,
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
        classify(&exited(0), CommandTermination::Exited, false),
        (SessionState::Succeeded, Some(0), None)
    );
    assert_eq!(
        classify(&exited(3 << 8), CommandTermination::Exited, false),
        (SessionState::Failed, Some(3), None)
    );
    assert_eq!(
        classify(&exited(9), CommandTermination::InactivityTimeout, false),
        (SessionState::Failed, None, Some("SIGKILL".to_string()))
    );
    assert_eq!(
        classify(&exited(1), CommandTermination::Cancelled, true),
        (SessionState::Stopped, None, Some("SIGTERM".to_string()))
    );
    assert_eq!(
        classify(&exited(1), CommandTermination::NotStarted, true).0,
        SessionState::Stopped
    );
    assert_eq!(
        classify(&exited(1), CommandTermination::RuntimeLimit, false).0,
        SessionState::Failed
    );

    assert!(is_retry_notification(
        "[Retry 1/2] Command crashed, retrying in 0ms..."
    ));
    assert!(!is_retry_notification("Retry later"));
}
