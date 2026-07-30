//! Event transport tests: fetch-streamed SSE and non-browser WebSocket.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use futures_util::StreamExt;
use serde_json::json;
use tokio_tungstenite::tungstenite;

use crate::events::LogEntry;
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::dto::EventEnvelope;
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::{router, RemoteControlState};

use super::{get, harness, send, snapshot_with, RecordingExecutor};

const TOKEN: &str = "stream-token";

/// Read an SSE body until `expected` frames have arrived (or the read times out).
///
/// This is exactly what a browser doing `fetch()` + response streaming reads:
/// the same bytes, without `EventSource`, which cannot attach a bearer token.
/// The stream stays open for live events, so the read is bounded by a frame
/// count rather than by end-of-body.
async fn read_sse_frames(
    response: axum::http::Response<axum::body::Body>,
    expected: usize,
) -> String {
    let mut body = response.into_body().into_data_stream();
    let mut text = String::new();
    while text.matches("data: ").count() < expected {
        match tokio::time::timeout(Duration::from_millis(500), body.next()).await {
            Ok(Some(Ok(chunk))) => text.push_str(&String::from_utf8_lossy(&chunk)),
            _ => break,
        }
    }
    text
}

#[tokio::test]
async fn authenticated_fetch_streamed_sse_replays_retained_events_in_order() {
    let h = harness(Some(TOKEN), &[]);
    for i in 0..3u32 {
        h.projection.apply_state(
            "progress_updated",
            Some("c1".to_string()),
            json!({ "i": i }),
            snapshot_with("c1", &format!("s{i}")),
        );
    }

    let response = send(
        &h.router,
        get("/api/v2/events?after_sequence=0", Some(TOKEN)),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let text = read_sse_frames(response, 3).await;
    let sequences: Vec<u64> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|json| serde_json::from_str::<EventEnvelope>(json).ok())
        .map(|event| event.event_sequence)
        .collect();
    assert_eq!(sequences, vec![1, 2, 3], "replay must be ordered");
    assert!(text.contains("event: progress_updated"));
    assert!(text.contains("id: 1"));
}

#[tokio::test]
async fn sse_signals_a_replay_gap_for_a_cursor_older_than_the_ring() {
    let h = harness(None, &[]);
    for i in 0..(crate::web::remote_control_api::dto::MAX_EVENTS + 5) {
        h.projection.apply_log(LogEntry::info(format!("l{i}")));
    }

    let response = send(&h.router, get("/api/v2/events?after_sequence=1", None)).await;
    let text = read_sse_frames(response, 1).await;

    assert!(text.contains("event: replay_gap"), "got: {text}");
    assert!(
        text.contains("GET /api/v2/state"),
        "the gap must tell the client how to recover"
    );
}

#[tokio::test]
async fn sse_signals_a_gap_when_the_cursor_belongs_to_another_incarnation() {
    let h = harness(None, &[]);
    h.projection
        .apply_state("a", None, json!({}), snapshot_with("c1", "queued"));

    let response = send(
        &h.router,
        get(
            "/api/v2/events?after_sequence=1&instance_id=00000000000000000000000000000000",
            None,
        ),
    )
    .await;
    let text = read_sse_frames(response, 1).await;
    assert!(text.contains("event: replay_gap"), "got: {text}");
}

#[tokio::test]
async fn sse_delivers_live_events_after_the_replay_burst() {
    let h = harness(None, &[]);
    h.projection
        .apply_state("first", None, json!({}), snapshot_with("c1", "queued"));

    let response = send(&h.router, get("/api/v2/events?after_sequence=0", None)).await;
    let projection = h.projection.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        projection.apply_state("second", None, json!({}), snapshot_with("c1", "applying"));
    });

    let mut body = response.into_body().into_data_stream();
    let mut text = String::new();
    while let Ok(Some(Ok(chunk))) =
        tokio::time::timeout(Duration::from_millis(500), body.next()).await
    {
        text.push_str(&String::from_utf8_lossy(&chunk));
        if text.contains("event: second") {
            break;
        }
    }
    assert!(text.contains("event: first"), "got: {text}");
    assert!(text.contains("event: second"), "got: {text}");
}

// ── WebSocket ────────────────────────────────────────────────────────────────

/// Serve a v2 router on an ephemeral loopback port for real handshake tests.
async fn serve(token: Option<&str>) -> (String, Arc<Projection>) {
    let projection = Arc::new(Projection::new());
    let auth = RemoteControlAuth::new(token.map(str::to_string), &[]).unwrap();
    let app = router(RemoteControlState::new(
        projection.clone(),
        Arc::new(auth),
        RecordingExecutor::new(),
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("{addr}"), projection)
}

#[tokio::test]
async fn an_authorized_header_upgrade_receives_ordered_replay_and_live_events() {
    let (addr, projection) = serve(Some(TOKEN)).await;
    projection.apply_state("first", None, json!({}), snapshot_with("c1", "queued"));

    let request = tungstenite::handshake::client::Request::builder()
        .uri(format!("ws://{addr}/api/v2/ws?after_sequence=0"))
        .header("host", addr.clone())
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header(
            "sec-websocket-key",
            tungstenite::handshake::client::generate_key(),
        )
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(())
        .unwrap();

    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("an Authorization-header upgrade must be accepted");

    let first = socket.next().await.unwrap().unwrap();
    let event: EventEnvelope = serde_json::from_str(first.to_text().unwrap()).unwrap();
    assert_eq!(event.event_sequence, 1);
    assert_eq!(event.event_type, "first");
    assert_eq!(event.instance_id, projection.instance_id());

    projection.apply_state("second", None, json!({}), snapshot_with("c1", "applying"));
    let second = socket.next().await.unwrap().unwrap();
    let event: EventEnvelope = serde_json::from_str(second.to_text().unwrap()).unwrap();
    assert_eq!(event.event_sequence, 2);
    assert_eq!(event.event_type, "second");

    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn an_upgrade_without_credentials_is_refused() {
    let (addr, _) = serve(Some(TOKEN)).await;
    let result = tokio_tungstenite::connect_async(format!("ws://{addr}/api/v2/ws")).await;
    let error = result.expect_err("an unauthenticated upgrade must fail");
    assert!(
        format!("{error}").contains("401"),
        "expected an unauthorized handshake, got: {error}"
    );
}

#[tokio::test]
async fn a_query_token_upgrade_is_refused_over_a_real_handshake() {
    let (addr, _) = serve(Some(TOKEN)).await;
    let result =
        tokio_tungstenite::connect_async(format!("ws://{addr}/api/v2/ws?token={TOKEN}")).await;
    let error = result.expect_err("a query-string token must never authenticate");
    assert!(format!("{error}").contains("401"), "got: {error}");
}

#[tokio::test]
async fn a_subprotocol_token_upgrade_is_refused_over_a_real_handshake() {
    let (addr, _) = serve(Some(TOKEN)).await;
    let request = tungstenite::handshake::client::Request::builder()
        .uri(format!("ws://{addr}/api/v2/ws"))
        .header("host", addr.clone())
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header(
            "sec-websocket-key",
            tungstenite::handshake::client::generate_key(),
        )
        .header("sec-websocket-protocol", format!("bearer.{TOKEN}"))
        .body(())
        .unwrap();

    let error = tokio_tungstenite::connect_async(request)
        .await
        .expect_err("a subprotocol token must never authenticate");
    assert!(format!("{error}").contains("401"), "got: {error}");
}

#[tokio::test]
async fn a_disconnecting_client_does_not_disturb_the_projection() {
    let (addr, projection) = serve(None).await;
    projection.apply_state("first", None, json!({}), snapshot_with("c1", "queued"));

    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/api/v2/ws?after_sequence=0"))
            .await
            .unwrap();
    let _ = socket.next().await.unwrap().unwrap();
    socket.close(None).await.unwrap();
    drop(socket);

    projection.apply_state("second", None, json!({}), snapshot_with("c1", "applying"));
    let (_, revision, sequence) = projection.snapshot();
    assert_eq!(revision, 2);
    assert_eq!(sequence, 2);
}
