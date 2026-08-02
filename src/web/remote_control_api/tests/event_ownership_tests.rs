//! Ownership tests for the internal-event → `/api/v2` boundary.
//!
//! These hold the projection to the contract the dispatch owner promises: one
//! internal event produces one ordered remote event, no field is dropped on the
//! way out, a repeated delivery changes nothing, and serial and parallel modes
//! publish the same thing for the same event.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::events::{
    all_completed_may_overwrite_mode, event_ownership, event_variant_name,
    ownership_fixtures::all_execution_events, EventDispatch, EventOwnership, EventSink,
    ExecutionEvent, LogEntry,
};
use crate::orchestration::state::{ExecutionMode, OrchestratorState};
use crate::web::remote_control_api::dto::{EventCategory, MAX_LOGS};
use crate::web::remote_control_api::projection::describe_event;
use crate::web::state::{WebEventSink, WebState};

fn state_with(mode: ExecutionMode, ids: &[&str]) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
    Arc::new(tokio::sync::RwLock::new(OrchestratorState::with_mode(
        ids.iter().map(|id| id.to_string()).collect(),
        10,
        mode,
    )))
}

/// A web frontend bound to a reducer through the single dispatch owner.
async fn bound_web_state(
    mode: ExecutionMode,
    ids: &[&str],
) -> (Arc<WebState>, Arc<tokio::sync::RwLock<OrchestratorState>>) {
    let web_state = Arc::new(WebState::new(&[]));
    let reducer = state_with(mode, ids);
    web_state.set_shared_state(reducer.clone()).await;
    (web_state, reducer)
}

async fn dispatch(
    reducer: &Arc<tokio::sync::RwLock<OrchestratorState>>,
    sinks: &[Arc<dyn EventSink>],
    event: ExecutionEvent,
) {
    crate::events::dispatch_event(reducer.as_ref(), sinks, event).await;
}

// ── Field preservation ───────────────────────────────────────────────────────

/// Every variant must reach the wire with a stable, distinct type name.
///
/// The old projection collapsed unenumerated variants onto a single
/// `orchestration_event` name and dropped their change ID with them, which is
/// how blockers, hooks, and terminal events became invisible to a controller.
#[test]
fn every_variant_has_a_distinct_wire_event_type() {
    let events = all_execution_events();
    let mut types = BTreeSet::new();
    for event in &events {
        let (event_type, _, _) = describe_event(event);
        assert_ne!(
            event_type,
            "orchestration_event",
            "{} still degrades to the generic wire name",
            event_variant_name(event)
        );
        assert!(
            types.insert(event_type),
            "{} reuses wire type {event_type}",
            event_variant_name(event)
        );
    }
    assert_eq!(types.len(), events.len());
}

/// Every change-addressed variant publishes its change ID.
#[test]
fn change_addressed_variants_publish_their_change_id() {
    // Process-level events genuinely have no single target change.
    let process_level = [
        "Log",
        "Stopping",
        "Stopped",
        "AllCompleted",
        "Error",
        "ChangesRefreshed",
        "WorktreesRefreshed",
    ];

    for event in all_execution_events() {
        let name = event_variant_name(&event);
        if process_level.contains(&name)
            || matches!(event_ownership(&event), EventOwnership::Presentation)
        {
            continue;
        }
        let (_, change_id, _) = describe_event(&event);
        assert_eq!(
            change_id.as_deref(),
            Some("change-a"),
            "{name} dropped its change ID at the wire boundary"
        );
    }
}

/// Golden payloads for the fields a controller cannot rediscover from the
/// snapshot: blocker evidence, hook identity, push target, retry budget.
#[test]
fn structured_payload_fields_survive_the_projection() {
    let cases: Vec<(ExecutionEvent, Vec<(&str, serde_json::Value)>)> = vec![
        (
            ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: crate::events::ownership_fixtures::blocker(),
            },
            vec![
                ("category", serde_json::json!("external_service")),
                ("phase", serde_json::json!("acceptance")),
                ("detail", serde_json::json!("registry returned 503")),
                (
                    "unblock_condition",
                    serde_json::json!("the registry answers 200"),
                ),
                ("prerequisite_owner", serde_json::json!("platform")),
                ("resumable", serde_json::json!(true)),
            ],
        ),
        (
            ExecutionEvent::HookFailed {
                change_id: "change-a".to_string(),
                hook_type: "pre_archive".to_string(),
                error: "hook boom".to_string(),
            },
            vec![
                ("hook_type", serde_json::json!("pre_archive")),
                ("detail", serde_json::json!("hook boom")),
            ],
        ),
        (
            ExecutionEvent::PushFailed {
                change_id: "change-a".to_string(),
                remote: "origin".to_string(),
                branch: "change-a".to_string(),
                error: "push boom".to_string(),
            },
            vec![
                ("remote", serde_json::json!("origin")),
                ("branch", serde_json::json!("change-a")),
                ("detail", serde_json::json!("push boom")),
            ],
        ),
        (
            ExecutionEvent::ArchiveRetryScheduled {
                change_id: "change-a".to_string(),
                attempt: 2,
                max_attempts: 3,
                reason: Some("flaky".to_string()),
                summary: None,
            },
            vec![
                ("attempt", serde_json::json!(2)),
                ("max_attempts", serde_json::json!(3)),
                ("detail", serde_json::json!("flaky")),
                ("summary", serde_json::Value::Null),
            ],
        ),
        (
            ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "base dirty".to_string(),
                auto_resumable: true,
            },
            vec![
                ("detail", serde_json::json!("base dirty")),
                ("auto_resumable", serde_json::json!(true)),
            ],
        ),
        (
            ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 3,
                total: 7,
            },
            vec![
                ("completed", serde_json::json!(3)),
                ("total", serde_json::json!(7)),
            ],
        ),
    ];

    for (event, expected) in cases {
        let (event_type, _, payload) = describe_event(&event);
        for (field, value) in expected {
            assert_eq!(
                payload.get(field),
                Some(&value),
                "{event_type} lost or changed payload field {field}: {payload}"
            );
        }
    }
}

/// A raw agent command line never crosses the boundary.
#[test]
fn command_lines_are_summarised_rather_than_published() {
    let (_, _, payload) = describe_event(&ExecutionEvent::ApplyStarted {
        change_id: "change-a".to_string(),
        command: "claude --token s3cret apply".to_string(),
    });
    let summary = payload["command_summary"].as_str().expect("summary");
    assert!(!summary.contains("s3cret"), "{summary}");
    assert!(summary.starts_with("Command metadata:"), "{summary}");
}

/// Operator-facing detail is sanitized by the same rule a retained log is.
#[test]
fn published_detail_is_sanitized_like_a_log_message() {
    let (_, _, payload) = describe_event(&ExecutionEvent::ProcessingError {
        id: "change-a".to_string(),
        error: "\x1b[31mred\x1b[0m\nsecond line".to_string(),
    });
    assert_eq!(payload["detail"], "red\\nsecond line");
}

// ── One event, one ordered remote event ──────────────────────────────────────

/// Every internal event allocates exactly one sequence, and only a state-owning
/// event can move the revision.
#[tokio::test]
async fn every_event_allocates_exactly_one_sequence() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    let events = all_execution_events();
    let expected = events.len() as u64;
    for event in events {
        let ownership = event_ownership(&event);
        let (_, revision_before, sequence_before) = projection.snapshot();
        dispatch(&reducer, &sinks, event.clone()).await;
        let (_, revision_after, sequence_after) = projection.snapshot();

        assert_eq!(
            sequence_after - sequence_before,
            1,
            "{} allocated {} sequences",
            event_variant_name(&event),
            sequence_after - sequence_before
        );
        if !matches!(ownership, EventOwnership::State) {
            assert_eq!(
                revision_after,
                revision_before,
                "{} is {ownership:?} and must not advance the revision",
                event_variant_name(&event)
            );
        }
    }

    let (_, _, sequence) = projection.snapshot();
    assert_eq!(sequence, expected);
}

/// A state event that changes nothing observable is a no-op revision.
#[tokio::test]
async fn repeating_a_state_event_does_not_advance_the_revision() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::ProcessingStarted("change-a".to_string()),
    )
    .await;
    let first = projection.revision();
    assert!(first > 0, "the first transition must be visible");

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::ProcessingStarted("change-a".to_string()),
    )
    .await;
    let (_, revision, sequence) = projection.snapshot();
    assert_eq!(
        revision, first,
        "a repeated transition that changes nothing must not churn the revision"
    );
    assert_eq!(sequence, 2, "it is still an ordered event");
}

/// The same dispatch delivered twice at the frontend boundary changes nothing.
#[tokio::test]
async fn duplicate_dispatch_delivery_is_a_no_op() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();

    let event = ExecutionEvent::ProcessingStarted("change-a".to_string());
    reducer.write().await.apply_execution_event(&event);
    let authoritative = reducer.read().await.clone();
    let dispatch = EventDispatch {
        id: crate::events::next_dispatch_id(),
        event: &event,
        ownership: event_ownership(&event),
        state: Some(&authoritative),
    };

    web_state.apply_dispatch(&dispatch).await;
    let (snapshot_once, revision_once, sequence_once) = projection.snapshot();

    web_state.apply_dispatch(&dispatch).await;
    let (snapshot_twice, revision_twice, sequence_twice) = projection.snapshot();

    assert_eq!(revision_once, revision_twice, "revision was doubled");
    assert_eq!(sequence_once, sequence_twice, "sequence was doubled");
    assert_eq!(snapshot_once, snapshot_twice);

    // Reducer state is untouched by the frontend in either delivery.
    assert_eq!(reducer.read().await.apply_count("change-a"), 0);
}

/// A duplicated log delivery does not retain a second copy.
#[tokio::test]
async fn duplicate_log_delivery_retains_one_entry() {
    let web_state = Arc::new(WebState::new(&[]));
    let projection = web_state.remote_control().projection();

    let event = ExecutionEvent::Log(LogEntry::info("only once"));
    let dispatch = EventDispatch {
        id: crate::events::next_dispatch_id(),
        event: &event,
        ownership: EventOwnership::Log,
        state: None,
    };
    web_state.apply_dispatch(&dispatch).await;
    web_state.apply_dispatch(&dispatch).await;

    let (logs, revision, sequence) = projection.logs();
    assert_eq!(logs.len(), 1, "a repeated delivery retained a second copy");
    assert_eq!(revision, 0, "a log never advances the revision");
    assert_eq!(sequence, 1);
}

// ── Serial / parallel parity ─────────────────────────────────────────────────

/// The same event published in serial and in parallel mode reaches v2 with the
/// same ownership, the same wire type, and the same retained-log count.
#[tokio::test]
async fn serial_and_parallel_publish_identical_ownership() {
    let log_carrying = [
        ExecutionEvent::Log(LogEntry::info("ai output").with_operation("apply")),
        ExecutionEvent::Log(LogEntry::warn("hook warning")),
        ExecutionEvent::Log(LogEntry::error("lifecycle failure")),
    ];
    let structured = [
        ExecutionEvent::HookFailed {
            change_id: "change-a".to_string(),
            hook_type: "pre_apply".to_string(),
            error: "hook boom".to_string(),
        },
        ExecutionEvent::Warning {
            title: "degraded".to_string(),
            message: "analysis fell back".to_string(),
        },
        ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: crate::events::ownership_fixtures::blocker(),
        },
    ];

    let mut observed = Vec::new();
    for mode in [ExecutionMode::Serial, ExecutionMode::Parallel] {
        let (web_state, reducer) = bound_web_state(mode, &["change-a"]).await;
        let projection = web_state.remote_control().projection();
        let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

        for event in log_carrying.iter().chain(structured.iter()) {
            dispatch(&reducer, &sinks, event.clone()).await;
        }

        let (logs, _, sequence) = projection.logs();
        let events = match projection.events_after(0) {
            crate::web::remote_control_api::projection::EventsSince::Replay(events) => events,
            other => panic!("expected a replay, got {other:?}"),
        };
        observed.push((
            logs.len(),
            sequence,
            events
                .iter()
                .map(|event| (event.event_type.clone(), event.category))
                .collect::<Vec<_>>(),
        ));
    }

    let (serial_logs, serial_sequence, serial_events) = &observed[0];
    let (parallel_logs, parallel_sequence, parallel_events) = &observed[1];
    assert_eq!(
        serial_logs, parallel_logs,
        "the two modes retained different log counts"
    );
    assert_eq!(*serial_logs, log_carrying.len());
    assert_eq!(serial_sequence, parallel_sequence);
    assert_eq!(
        serial_events, parallel_events,
        "the two modes published different event ownership"
    );
    assert!(serial_events
        .iter()
        .any(|(name, category)| name == "log" && *category == EventCategory::Log));
    assert!(serial_events.iter().any(|(name, _)| name == "hook_failed"));
    assert!(serial_events.iter().any(|(name, _)| name == "warning"));
    assert!(serial_events
        .iter()
        .any(|(name, _)| name == "acceptance_gated"));
}

/// Retention is bounded at the documented ring size, oldest first.
#[tokio::test]
async fn log_retention_is_bounded_and_ordered() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    for index in 0..(MAX_LOGS + 5) {
        dispatch(
            &reducer,
            &sinks,
            ExecutionEvent::Log(LogEntry::info(format!("line-{index}"))),
        )
        .await;
    }

    let (logs, revision, sequence) = projection.logs();
    assert_eq!(logs.len(), MAX_LOGS);
    assert_eq!(logs.first().unwrap().message, "line-5");
    assert_eq!(
        logs.last().unwrap().message,
        format!("line-{}", MAX_LOGS + 4)
    );
    assert_eq!(revision, 0, "a chatty run never invalidates a client token");
    assert_eq!(sequence, (MAX_LOGS + 5) as u64);
}

// ── Terminal state alignment ─────────────────────────────────────────────────

/// A late `AllCompleted` must not overwrite a retained terminal mode, and the
/// two frontends must decide that the same way.
#[tokio::test]
async fn late_all_completed_preserves_retained_terminal_modes() {
    for (terminal, expected_mode) in [
        (ExecutionEvent::Stopped, "stopped"),
        (
            ExecutionEvent::Error {
                message: "fatal".to_string(),
            },
            "error",
        ),
    ] {
        let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
        let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

        dispatch(&reducer, &sinks, terminal.clone()).await;
        assert_eq!(web_state.get_state().await.app_mode, expected_mode);

        dispatch(&reducer, &sinks, ExecutionEvent::AllCompleted).await;
        assert_eq!(
            web_state.get_state().await.app_mode,
            expected_mode,
            "a late completion overwrote the authoritative terminal mode"
        );
        // The TUI reaches the same conclusion through the same predicate; its
        // side of the agreement is asserted in
        // `tui::state::event_handlers::completion`.
        assert!(!all_completed_may_overwrite_mode(expected_mode));
    }
}

/// A completion that follows ordinary running state still completes.
#[tokio::test]
async fn all_completed_still_completes_a_healthy_run() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::ProcessingStarted("change-a".to_string()),
    )
    .await;
    assert_eq!(web_state.get_state().await.app_mode, "running");

    dispatch(&reducer, &sinks, ExecutionEvent::AllCompleted).await;
    assert_eq!(web_state.get_state().await.app_mode, "select");
}

/// A duplicate `Stopped` reconciles without doubling anything a client sees.
#[tokio::test]
async fn duplicate_stopped_is_idempotent_on_the_stream() {
    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(&reducer, &sinks, ExecutionEvent::Stopped).await;
    let revision_after_first = projection.revision();

    dispatch(&reducer, &sinks, ExecutionEvent::Stopped).await;
    let (_, revision, sequence) = projection.snapshot();

    assert_eq!(web_state.get_state().await.app_mode, "stopped");
    assert_eq!(
        revision, revision_after_first,
        "a duplicate stop advanced the revision"
    );
    assert_eq!(sequence, 2, "both deliveries are still ordered");
}

/// Ownership consolidation must not break replay: a retained cursor still
/// replays in order, and an unusable one is reported as a gap.
#[tokio::test]
async fn replay_and_gap_recovery_survive_ownership_consolidation() {
    use crate::web::remote_control_api::projection::EventsSince;

    let (web_state, reducer) = bound_web_state(ExecutionMode::Parallel, &["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    for event in [
        ExecutionEvent::ProcessingStarted("change-a".to_string()),
        ExecutionEvent::Log(LogEntry::info("mid")),
        ExecutionEvent::Warning {
            title: "t".to_string(),
            message: "m".to_string(),
        },
        ExecutionEvent::AllCompleted,
    ] {
        dispatch(&reducer, &sinks, event).await;
    }

    let EventsSince::Replay(replayed) = projection.events_after(1) else {
        panic!("a retained cursor must replay");
    };
    assert_eq!(replayed.len(), 3);
    assert!(replayed
        .windows(2)
        .all(|pair| pair[0].event_sequence < pair[1].event_sequence));
    assert!(replayed
        .iter()
        .all(|event| event.state_revision <= projection.revision()));

    assert_eq!(
        projection.events_after(99),
        EventsSince::Gap,
        "a cursor from another incarnation is not replayable"
    );
    let gap = projection.gap_envelope(99);
    assert_eq!(gap.category, EventCategory::Gap);
    assert_eq!(gap.payload["requested_after"], 99);
}
