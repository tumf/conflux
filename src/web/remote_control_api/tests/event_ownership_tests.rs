//! Ownership tests for the internal-event → `/api/v2` boundary.
//!
//! These hold the projection to the contract the dispatch owner promises: one
//! internal event produces one ordered remote event, no field is dropped on the
//! way out, a repeated delivery changes nothing, and every frontend publishes
//! the same thing for the same event.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::events::{
    all_completed_may_overwrite_mode, event_ownership, event_variant_name,
    ownership_fixtures::all_execution_events, EventDispatch, EventOwnership, EventSink,
    ExecutionEvent, LogEntry,
};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::{EventCategory, MAX_LOGS};
use crate::web::remote_control_api::projection::describe_event;
use crate::web::state::{WebEventSink, WebState};

fn state_with(ids: &[&str]) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
    Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        ids.iter().map(|id| id.to_string()).collect(),
        10,
    )))
}

/// A web frontend bound to a reducer through the single dispatch owner.
async fn bound_web_state(
    ids: &[&str],
) -> (Arc<WebState>, Arc<tokio::sync::RwLock<OrchestratorState>>) {
    let web_state = Arc::new(WebState::new(&[]));
    let reducer = state_with(ids);
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
        "PersistentSchedulerIdle",
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
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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

// ── Frontend parity ──────────────────────────────────────────────────────────

/// The same event published through two frontends reaches v2 with the same
/// ownership, the same wire type, and the same retained-log count.
#[tokio::test]
async fn every_frontend_publishes_identical_ownership() {
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
    // Two independent frontends of the one execution path must publish the
    // identical ownership for the identical event stream.
    for _ in 0..2 {
        let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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

    let (first_logs, first_sequence, first_events) = &observed[0];
    let (second_logs, second_sequence, second_events) = &observed[1];
    assert_eq!(
        first_logs, second_logs,
        "two frontends retained different log counts"
    );
    assert_eq!(*first_logs, log_carrying.len());
    assert_eq!(first_sequence, second_sequence);
    assert_eq!(
        first_events, second_events,
        "two frontends published different event ownership"
    );
    assert!(first_events
        .iter()
        .any(|(name, category)| name == "log" && *category == EventCategory::Log));
    assert!(first_events.iter().any(|(name, _)| name == "hook_failed"));
    assert!(first_events.iter().any(|(name, _)| name == "warning"));
    assert!(first_events
        .iter()
        .any(|(name, _)| name == "acceptance_gated"));
}

/// Retention is bounded at the documented ring size, oldest first.
#[tokio::test]
async fn log_retention_is_bounded_and_ordered() {
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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
        let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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

/// Verification `persistent-idle-ready-regressions`: the idle dispatch publishes
/// one coherent Ready revision and no churn afterwards.
///
/// Everything here goes through the authoritative dispatch boundary and the
/// shared `WebState`, never a hand-mutated snapshot, so what is asserted is what
/// a `/api/v2` client would actually read.
#[tokio::test]
async fn persistent_idle_projects_api_ready_once() {
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        },
    )
    .await;
    assert_eq!(web_state.get_state().await.app_mode, "running");

    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    let (snapshot, revision_after_idle, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "select");
    assert!(
        snapshot.persistent_scheduler_idle,
        "the idle edge and its Ready snapshot share one revision"
    );

    // Duplicate and no-op idle observation is ordered but changes nothing a
    // client reads, so it allocates no new revision.
    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::AnalysisStarted {
            remaining_changes: 1,
            attempt_id: "attempt-1".to_string(),
        },
    )
    .await;
    let (snapshot, revision, sequence) = projection.snapshot();
    assert_eq!(
        revision, revision_after_idle,
        "a duplicate or no-op idle observation advanced the revision"
    );
    assert_eq!(snapshot.app_mode, "select");
    assert!(snapshot.persistent_scheduler_idle);
    assert!(sequence > 0, "every delivery is still ordered");

    // Admitted work clears the field in the same revision that reports Running.
    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        },
    )
    .await;
    let (snapshot, _, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "running");
    assert!(!snapshot.persistent_scheduler_idle);

    // Both terminal outcomes clear it too.
    for (terminal, expected_mode) in [
        (ExecutionEvent::Stopped, "stopped"),
        (
            ExecutionEvent::Error {
                message: "fatal".to_string(),
            },
            "error",
        ),
    ] {
        let (web_state, reducer) = bound_web_state(&["change-a"]).await;
        let projection = web_state.remote_control().projection();
        let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

        dispatch(
            &reducer,
            &sinks,
            ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            },
        )
        .await;
        dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
        assert!(projection.snapshot().0.persistent_scheduler_idle);

        dispatch(&reducer, &sinks, terminal).await;
        let (snapshot, _, _) = projection.snapshot();
        assert_eq!(snapshot.app_mode, expected_mode);
        assert!(
            !snapshot.persistent_scheduler_idle,
            "{expected_mode} must close the idle episode"
        );
    }
}

/// A late idle event never sets the field, so the two frontends cannot disagree
/// about whether a stopped or failed run still has a scheduler to command.
#[tokio::test]
async fn late_persistent_idle_retains_transitional_and_terminal_modes() {
    for (earlier, expected_mode) in [
        (ExecutionEvent::Stopping, "stopping"),
        (ExecutionEvent::Stopped, "stopped"),
        (
            ExecutionEvent::Error {
                message: "fatal".to_string(),
            },
            "error",
        ),
    ] {
        let (web_state, reducer) = bound_web_state(&["change-a"]).await;
        let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

        // Each of the three modes is reached from a live run, which is the only
        // way the process reaches them: a graceful stop is never admitted from
        // pre-run Select, so arranging one there would be a state that cannot
        // exist — and `Stopping` over parked Ready means something else entirely
        // (an idle-origin stop, which keeps its episode on purpose).
        dispatch(
            &reducer,
            &sinks,
            ExecutionEvent::WorkspacePreparationStarted {
                change_id: "change-a".to_string(),
            },
        )
        .await;
        dispatch(&reducer, &sinks, earlier).await;
        dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;

        let state = web_state.get_state().await;
        assert_eq!(
            state.app_mode, expected_mode,
            "a late idle event overwrote {expected_mode}"
        );
        assert!(!state.persistent_scheduler_idle);
        // The TUI reaches the same conclusion through the same predicate; its
        // side of the agreement lives in `tui::state::event_handlers::completion`.
        assert!(!crate::events::persistent_idle_may_project_ready(
            expected_mode
        ));
    }

    // Pre-run Select is already Ready and owns no live scheduler.
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];
    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    let state = web_state.get_state().await;
    assert_eq!(state.app_mode, "select");
    assert!(
        !state.persistent_scheduler_idle,
        "pre-run Select must not be turned into a live idle episode"
    );
}

/// Verification `persistent-idle-ready-regressions`: an idle-origin stop keeps
/// its episode identity, and cancel-stop returns to Ready rather than Running.
#[tokio::test]
async fn idle_origin_stop_and_cancel_preserve_ready() {
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        },
    )
    .await;
    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;

    // Both halves travel as authoritative dispatches — the exact existing
    // `Stopping` event and the accepted cancel-stop outcome — so this is the
    // same path a TUI keypress takes, not a web-only projection call.
    dispatch(&reducer, &sinks, ExecutionEvent::Stopping).await;
    let state = web_state.get_state().await;
    assert_eq!(state.app_mode, "stopping");
    assert!(
        state.persistent_scheduler_idle,
        "a stop requested from idle Ready is still the same episode"
    );

    dispatch(&reducer, &sinks, stop_cancelled()).await;
    let state = web_state.get_state().await;
    assert_eq!(state.app_mode, "select");
    assert!(state.persistent_scheduler_idle);

    // Work that wins the race before cancel-stop preserves Stopping, clears the
    // episode, and makes the later cancel-stop restore Running instead.
    dispatch(&reducer, &sinks, ExecutionEvent::Stopping).await;
    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        },
    )
    .await;
    let state = web_state.get_state().await;
    assert_eq!(state.app_mode, "stopping");
    assert!(!state.persistent_scheduler_idle);

    dispatch(&reducer, &sinks, stop_cancelled()).await;
    assert_eq!(web_state.get_state().await.app_mode, "running");
}

/// One accepted Start's `RunDispatched`, shaped as run control publishes it for
/// a woken live scheduler.
fn accepted_idle_start(change_ids: &[&str]) -> ExecutionEvent {
    ExecutionEvent::OperatorCommandApplied {
        effect: crate::events::OperatorCommandEffect::RunDispatched {
            change_ids: change_ids.iter().map(|id| (*id).to_string()).collect(),
            explicit_retry: false,
            scheduler_started: false,
        },
    }
}

/// Verification `idle-start-running-regressions`: the accepted Start's own
/// revision is where a `/api/v2` client reads Running, and the no-work park is
/// where it reads Ready again.
///
/// Revision *identity* is the property: a client that replaces local state at
/// the revision the command reported must find the mode and the idle fact the
/// command produced, not a later or earlier one. Duplicate and committed-nothing
/// dispatches are checked in the same stream, because "one coherent revision"
/// and "no spurious revision" are the same guarantee read from two directions.
#[tokio::test]
async fn idle_start_running_accepted_start_publishes_one_coherent_snapshot() {
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::WorkspacePreparationStarted {
            change_id: "change-a".to_string(),
        },
    )
    .await;
    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    let (_, parked_revision, _) = projection.snapshot();

    // A dispatch that committed nothing is not an accepted Start, so it leaves
    // the idle snapshot exactly as it found it.
    dispatch(&reducer, &sinks, accepted_idle_start(&[])).await;
    let (snapshot, revision, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "select");
    assert!(snapshot.persistent_scheduler_idle);
    assert_eq!(
        revision, parked_revision,
        "a committed-nothing dispatch must publish no state revision"
    );

    // The accepted Start, and the one revision that carries its whole effect.
    dispatch(&reducer, &sinks, accepted_idle_start(&["change-a"])).await;
    let (snapshot, running_revision, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "running");
    assert!(
        !snapshot.persistent_scheduler_idle,
        "the accepted Start closes the idle episode in the same revision"
    );
    assert!(
        running_revision > parked_revision,
        "the accepted Start is a state-owning outcome"
    );

    // A second identical outcome finds the episode already open, and analysis
    // is neither work nor an episode transition.
    dispatch(&reducer, &sinks, accepted_idle_start(&["change-a"])).await;
    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::AnalysisStarted {
            remaining_changes: 1,
            attempt_id: "attempt-1".to_string(),
        },
    )
    .await;
    let (snapshot, revision, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "running");
    assert_eq!(
        revision, running_revision,
        "a duplicate accepted outcome and an analysis start publish no new revision"
    );

    // Nothing was admitted, so the rearmed park restores the idle snapshot.
    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    let (snapshot, ready_revision, _) = projection.snapshot();
    assert_eq!(snapshot.app_mode, "select");
    assert!(snapshot.persistent_scheduler_idle);
    assert!(ready_revision > running_revision);

    dispatch(&reducer, &sinks, ExecutionEvent::PersistentSchedulerIdle).await;
    assert_eq!(
        projection.snapshot().1,
        ready_revision,
        "duplicate idle observation still creates no additional revision"
    );
}

/// The authoritative event an accepted cancel-stop publishes.
fn stop_cancelled() -> ExecutionEvent {
    ExecutionEvent::OperatorCommandApplied {
        effect: crate::events::OperatorCommandEffect::StopCancelled,
    }
}

/// A duplicate `Stopped` reconciles without doubling anything a client sees.
#[tokio::test]
async fn duplicate_stopped_is_idempotent_on_the_stream() {
    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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

/// The stop a remote client sees is the reducer's, not a frontend repair.
///
/// `/api/v2` never had a stopped-row fixup of its own, so before the reducer
/// owned this transition an operator's stop left `accepting` on the wire for as
/// long as the process ran. The assertions here are the API half of the same
/// one-authority contract the TUI reads back.
#[tokio::test]
async fn stopped_projection_reconciles_change_status() {
    use crate::openspec::{Change, ProposalMetadata};
    use crate::orchestration::operator_command::ExecutionMarkStore;
    use crate::web::remote_control_api::dto::QueueIntent;

    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
    let marks = Arc::new(ExecutionMarkStore::new());
    marks.set("change-a", true);
    web_state.set_execution_marks(marks.clone()).await;
    web_state
        .update(&[Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 2,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }])
        .await;
    let projection = web_state.remote_control().projection();
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];

    dispatch(
        &reducer,
        &sinks,
        ExecutionEvent::AcceptanceStarted {
            change_id: "change-a".to_string(),
            command: "accept".to_string(),
        },
    )
    .await;
    let (snapshot, revision_before_stop, sequence_before_stop) = projection.snapshot();
    assert_eq!(snapshot.changes[0].display_status, "accepting");

    dispatch(&reducer, &sinks, ExecutionEvent::Stopped).await;

    let (snapshot, revision_after_stop, sequence_after_stop) = projection.snapshot();
    let change = &snapshot.changes[0];
    assert_eq!(
        change.display_status, "not queued",
        "the stopped run's row is still published as active"
    );
    assert_eq!(
        change.queue_intent,
        QueueIntent::NotQueued,
        "display status and queue intent must agree at one revision"
    );
    assert!(
        change.execution_marked,
        "a process stop must not clear the mark that keeps the row resumable"
    );
    assert_eq!(
        revision_after_stop,
        revision_before_stop + 1,
        "the reconciled stop is exactly one state revision"
    );
    assert_eq!(sequence_after_stop, sequence_before_stop + 1);

    dispatch(&reducer, &sinks, ExecutionEvent::Stopped).await;

    let (snapshot, revision, sequence) = projection.snapshot();
    assert_eq!(snapshot.changes[0].display_status, "not queued");
    assert_eq!(
        revision, revision_after_stop,
        "a duplicate stop advanced the revision"
    );
    assert_eq!(
        sequence,
        sequence_after_stop + 1,
        "both deliveries are still ordered"
    );
}

/// Ownership consolidation must not break replay: a retained cursor still
/// replays in order, and an unusable one is reported as a gap.
#[tokio::test]
async fn replay_and_gap_recovery_survive_ownership_consolidation() {
    use crate::web::remote_control_api::projection::EventsSince;

    let (web_state, reducer) = bound_web_state(&["change-a"]).await;
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
