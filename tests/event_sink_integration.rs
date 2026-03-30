use conflux::events::{dispatch_event, EventSink, ExecutionEvent, MockEventSink};
use conflux::orchestration::state::OrchestratorState;

#[tokio::test]
async fn mock_event_sink_collects_dispatched_events() {
    let state = tokio::sync::RwLock::new(OrchestratorState::new(vec!["change-a".to_string()], 5));
    let mock_sink = std::sync::Arc::new(MockEventSink::new());
    let sinks: Vec<std::sync::Arc<dyn EventSink>> = vec![mock_sink.clone()];

    dispatch_event(
        &state,
        &sinks,
        ExecutionEvent::ProcessingStarted("change-a".to_string()),
    )
    .await;

    let events = mock_sink.events().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events.first(),
        Some(ExecutionEvent::ProcessingStarted(id)) if id == "change-a"
    ));
}
