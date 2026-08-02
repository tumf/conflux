//! Projection-owner tests: revision, sequence, ordering, rings, and gaps.

use std::sync::Arc;

use serde_json::json;

use crate::events::{ExecutionEvent, LogEntry};
use crate::web::remote_control_api::dto::{EventCategory, InstanceSnapshot, MAX_LOGS};
use crate::web::remote_control_api::projection::{
    describe_event, project_snapshot, EventsSince, Projection,
};
use crate::web::state::{OrchestratorStateSnapshot, WebState};

use super::snapshot_with;

#[test]
fn instance_id_is_a_fresh_random_128_bit_hex_per_incarnation() {
    let first = Projection::new();
    let second = Projection::new();
    assert_eq!(first.instance_id().len(), 32);
    assert!(first.instance_id().chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(
        first.instance_id(),
        second.instance_id(),
        "a restart must invalidate a stored cursor"
    );
}

#[test]
fn changed_state_advances_revision_once_and_publishes_the_same_revision() {
    let projection = Projection::new();
    assert_eq!(projection.revision(), 0);

    let event = projection.apply_state(
        "processing_started",
        Some("c1".to_string()),
        json!({}),
        snapshot_with("c1", "applying"),
    );

    assert_eq!(event.state_revision, 1);
    assert_eq!(event.event_sequence, 1);
    assert_eq!(event.category, EventCategory::State);

    let (snapshot, revision, sequence) = projection.snapshot();
    assert_eq!(revision, 1, "revision advances exactly once");
    assert_eq!(sequence, 1);
    assert_eq!(
        snapshot,
        snapshot_with("c1", "applying"),
        "the stored snapshot and the published event describe the same instant"
    );
}

#[test]
fn no_op_state_input_allocates_a_sequence_without_advancing_revision() {
    let projection = Projection::new();
    projection.apply_state("a", None, json!({}), snapshot_with("c1", "applying"));

    let event = projection.apply_state(
        "b",
        None,
        json!({}),
        snapshot_with("c1", "applying"), // identical
    );

    assert_eq!(
        event.state_revision, 1,
        "an unchanged snapshot must not advance revision"
    );
    assert_eq!(event.event_sequence, 2, "the event is still ordered");
    assert_eq!(projection.revision(), 1);
}

#[test]
fn log_input_keeps_the_current_revision_and_only_advances_the_sequence() {
    let projection = Projection::new();
    projection.apply_state("a", None, json!({}), snapshot_with("c1", "applying"));

    let event = projection.apply_log(LogEntry::info("hello"));

    assert_eq!(event.category, EventCategory::Log);
    assert_eq!(event.state_revision, 1, "logs are observational");
    assert_eq!(event.event_sequence, 2);
    assert_eq!(projection.revision(), 1);
}

#[test]
fn periodic_sync_emits_nothing_when_the_snapshot_is_unchanged() {
    let projection = Projection::new();
    let snapshot = snapshot_with("c1", "queued");
    assert!(projection
        .apply_state_if_changed("state_refreshed", snapshot.clone())
        .is_some());

    let (_, revision_before, sequence_before) = projection.snapshot();
    assert!(
        projection
            .apply_state_if_changed("state_refreshed", snapshot)
            .is_none(),
        "an idle refresh must not churn the event ring"
    );
    let (_, revision_after, sequence_after) = projection.snapshot();
    assert_eq!(revision_before, revision_after);
    assert_eq!(sequence_before, sequence_after);
}

#[test]
fn logs_are_bounded_to_the_documented_ring_size() {
    let projection = Projection::new();
    for i in 0..(MAX_LOGS + 25) {
        projection.apply_log(LogEntry::info(format!("line {i}")));
    }

    let (logs, revision, sequence) = projection.logs();
    assert_eq!(logs.len(), MAX_LOGS);
    assert_eq!(
        revision, 0,
        "a chatty run must not invalidate concurrency tokens"
    );
    assert_eq!(sequence as usize, MAX_LOGS + 25);
    assert_eq!(
        logs.first().unwrap().message,
        "line 25",
        "oldest entries drop first"
    );
    assert_eq!(
        logs.last().unwrap().message,
        format!("line {}", MAX_LOGS + 24)
    );
}

#[test]
fn events_after_a_retained_cursor_replays_in_order() {
    let projection = Projection::new();
    for i in 0..5u32 {
        projection.apply_state(
            "progress_updated",
            None,
            json!({ "i": i }),
            snapshot_with("c1", &format!("status-{i}")),
        );
    }

    let EventsSince::Replay(events) = projection.events_after(2) else {
        panic!("a retained cursor must replay");
    };
    let sequences: Vec<u64> = events.iter().map(|e| e.event_sequence).collect();
    assert_eq!(sequences, vec![3, 4, 5]);
}

#[test]
fn current_cursor_replays_nothing_and_future_cursor_is_a_gap() {
    let projection = Projection::new();
    projection.apply_state("a", None, json!({}), snapshot_with("c1", "queued"));

    assert_eq!(projection.events_after(1), EventsSince::Replay(Vec::new()));
    assert_eq!(
        projection.events_after(99),
        EventsSince::Gap,
        "a cursor ahead of us cannot be from this incarnation"
    );
}

#[test]
fn cursor_older_than_the_ring_is_a_gap_recoverable_through_state() {
    // A tiny projection would need a tiny ring; instead, overflow the real one.
    let projection = Projection::new();
    for i in 0..(crate::web::remote_control_api::dto::MAX_EVENTS + 10) {
        projection.apply_log(LogEntry::info(format!("l{i}")));
    }

    assert_eq!(projection.events_after(1), EventsSince::Gap);

    let gap = projection.gap_envelope(1);
    assert_eq!(gap.category, EventCategory::Gap);
    assert_eq!(gap.event_type, "replay_gap");
    assert_eq!(gap.payload["recover_with"], "GET /api/v2/state");
    assert_eq!(gap.payload["requested_after"], 1);
}

#[tokio::test]
async fn subscribers_observe_events_in_allocation_order() {
    let projection = Arc::new(Projection::new());
    let mut rx = projection.subscribe();

    for i in 0..10u32 {
        projection.apply_state(
            "progress_updated",
            None,
            json!({}),
            snapshot_with("c1", &format!("s{i}")),
        );
    }

    let mut received = Vec::new();
    for _ in 0..10 {
        received.push(rx.recv().await.unwrap().event_sequence);
    }
    assert_eq!(received, (1..=10).collect::<Vec<u64>>());
}

#[tokio::test]
async fn concurrent_state_inputs_never_produce_a_torn_snapshot() {
    let projection = Arc::new(Projection::new());
    let mut handles = Vec::new();
    for i in 0..16u32 {
        let projection = projection.clone();
        handles.push(tokio::spawn(async move {
            projection.apply_state(
                "progress_updated",
                None,
                json!({}),
                snapshot_with("c1", &format!("status-{i}")),
            );
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }

    let (snapshot, revision, sequence) = projection.snapshot();
    assert_eq!(sequence, 16, "every input allocated exactly one sequence");
    assert!((1..=16).contains(&revision));
    assert_eq!(snapshot.changes.len(), 1);
    // Whatever won, it is a whole snapshot from one writer, not a mix.
    assert!(snapshot.changes[0].display_status.starts_with("status-"));
}

#[test]
fn snapshot_projection_preserves_reducer_derived_statuses() {
    let mut source = OrchestratorStateSnapshot::from_changes(&[]);
    source.app_mode = "running".to_string();
    source.is_resolving = true;
    source.changes = vec![
        crate::web::state::ChangeStatus {
            id: "queued-change".to_string(),
            completed_tasks: 2,
            total_tasks: 4,
            progress_percent: 50.0,
            status: "in_progress".to_string(),
            dependencies: vec!["dep".to_string()],
            queue_status: Some("blocked".to_string()),
            iteration_number: Some(3),
            ..Default::default()
        },
        crate::web::state::ChangeStatus {
            id: "idle-change".to_string(),
            status: "pending".to_string(),
            ..Default::default()
        },
    ];
    source.completed_changes = 0;
    source.in_progress_changes = 1;
    source.pending_changes = 0;

    let projected = project_snapshot(&source);

    assert_eq!(projected.app_mode, "running");
    assert!(projected.is_resolving);
    assert_eq!(projected.changes[0].display_status, "blocked");
    assert_eq!(projected.changes[0].iteration_number, Some(3));
    assert_eq!(
        projected.changes[1].display_status, "not queued",
        "absent queue status is the explicit 'not queued' value, not a hole"
    );
    assert_eq!(projected.totals.total, 2);
    assert_eq!(projected.totals.in_progress, 1);
}

#[test]
fn projected_snapshot_has_no_wall_clock_field() {
    // A timestamp inside the snapshot would make every projection look changed
    // and advance the revision forever.
    let empty = InstanceSnapshot::empty();
    let json = serde_json::to_value(&empty).unwrap();
    let object = json.as_object().unwrap();
    assert!(!object.contains_key("last_updated"));
    assert!(!object.contains_key("timestamp"));
    assert_eq!(
        object.keys().collect::<Vec<_>>().len(),
        5,
        "snapshot fields: app_mode, is_resolving, process_error, changes, totals"
    );
}

#[test]
fn execution_events_are_projected_to_stable_wire_names() {
    let cases: Vec<(ExecutionEvent, &str, Option<&str>)> = vec![
        (
            ExecutionEvent::ProcessingStarted("c1".to_string()),
            "processing_started",
            Some("c1"),
        ),
        (
            ExecutionEvent::ChangeArchived("c2".to_string()),
            "change_archived",
            Some("c2"),
        ),
        (
            ExecutionEvent::ProcessingError {
                id: "c3".to_string(),
                error: "boom".to_string(),
            },
            "processing_error",
            Some("c3"),
        ),
        (ExecutionEvent::Log(LogEntry::info("x")), "log", None),
    ];

    for (event, expected_type, expected_change) in cases {
        let (event_type, change_id, payload) = describe_event(&event);
        assert_eq!(event_type, expected_type);
        assert_eq!(change_id.as_deref(), expected_change);
        if expected_type == "processing_error" {
            assert_eq!(payload["detail"], "boom");
        }
    }
}

#[tokio::test]
async fn web_state_feeds_the_projection_with_events_and_logs() {
    let web_state = WebState::new(&[]);
    let projection = web_state.remote_control().projection();

    web_state
        .apply_execution_event(&ExecutionEvent::Log(LogEntry::info("hello")))
        .await;
    let (logs, revision_after_log, sequence_after_log) = projection.logs();
    assert_eq!(logs.len(), 1);
    assert_eq!(
        revision_after_log, 0,
        "a log alone never advances the revision"
    );
    assert_eq!(sequence_after_log, 1);

    web_state
        .apply_execution_event(&ExecutionEvent::ProgressUpdated {
            change_id: "c1".to_string(),
            completed: 1,
            total: 2,
        })
        .await;
    let (_, _, sequence) = projection.snapshot();
    assert_eq!(
        sequence, 2,
        "every observed event is ordered into the stream"
    );
}
