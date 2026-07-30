//! Ordered event transports: authenticated SSE and non-browser WebSocket.
//!
//! Both transports carry the same envelopes in the same order and use the same
//! resume rule: subscribe first, then replay from the cursor, then drop replayed
//! sequences out of the live feed. Subscribing before reading the ring is what
//! makes the seam lossless — an event published between the two steps arrives on
//! the live feed instead of vanishing.
//!
//! Neither transport accepts a token anywhere but the `Authorization` header,
//! which is why neither is usable from browser-native `EventSource`/`WebSocket`.
//! The browser dashboard stays on legacy `/ws`.

use std::collections::HashSet;
use std::convert::Infallible;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use tracing::debug;

use super::dto::EventEnvelope;
use super::projection::{EventsSince, Projection};
use super::RemoteControlState;

/// Interval for SSE comment frames that keep intermediaries from closing an idle stream.
const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// Resume parameters accepted by both transports.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StreamParams {
    /// Resume strictly after this sequence.
    #[serde(default)]
    pub after_sequence: Option<u64>,
    /// Incarnation the cursor came from. A mismatch is a gap, never a silent replay.
    #[serde(default)]
    pub instance_id: Option<String>,
}

/// Build the initial burst for a cursor: either the retained replay or a single gap signal.
fn initial_events(projection: &Projection, params: &StreamParams) -> Vec<EventEnvelope> {
    let Some(after) = params.after_sequence else {
        // No cursor: the client is expected to have just fetched a snapshot, so
        // send nothing and let the live feed take over.
        return Vec::new();
    };
    if params
        .instance_id
        .as_deref()
        .is_some_and(|id| id != projection.instance_id())
    {
        // A cursor from a previous process incarnation means nothing here.
        return vec![projection.gap_envelope(after)];
    }
    match projection.events_after(after) {
        EventsSince::Replay(events) => events,
        EventsSince::Gap => vec![projection.gap_envelope(after)],
    }
}

/// Merge a replay burst with the live feed, suppressing sequences already replayed.
fn event_stream(
    projection: std::sync::Arc<Projection>,
    params: StreamParams,
) -> impl Stream<Item = EventEnvelope> {
    // Subscribe *before* reading the ring so nothing can slip through the seam.
    let rx = projection.subscribe();
    let replayed = initial_events(&projection, &params);
    let already_sent: HashSet<u64> = replayed.iter().map(|e| e.event_sequence).collect();

    let live = stream::unfold(
        (rx, projection, already_sent),
        |(mut rx, projection, already_sent)| async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if already_sent.contains(&event.event_sequence) {
                            continue;
                        }
                        return Some((event, (rx, projection, already_sent)));
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        // The subscriber fell behind the fan-out buffer, so the
                        // ordering guarantee is broken for this client. Say so
                        // rather than silently delivering a hole.
                        debug!("v2 event subscriber lagged by {skipped} events");
                        let gap = projection.gap_envelope(0);
                        return Some((gap, (rx, projection, already_sent)));
                    }
                    Err(RecvError::Closed) => return None,
                }
            }
        },
    );

    stream::iter(replayed).chain(live)
}

/// Server-Sent Events transport.
///
/// Authenticated browser clients must consume this with `fetch()` response
/// streaming; native `EventSource` cannot attach the bearer token and is not
/// supported.
#[utoipa::path(
    get,
    path = "/api/v2/events",
    tag = "remote-control",
    params(
        ("after_sequence" = Option<u64>, Query, description = "Resume strictly after this event sequence"),
        ("instance_id" = Option<String>, Query, description = "Incarnation the cursor belongs to")
    ),
    responses(
        (status = 200, description = "Ordered SSE event stream (fetch() response streaming)"),
        (status = 401, description = "Missing or invalid bearer credentials", body = super::dto::ApiError)
    )
)]
pub async fn events(
    State(state): State<RemoteControlState>,
    Query(params): Query<StreamParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = event_stream(state.projection.clone(), params).map(|envelope| {
        let event = Event::default()
            .id(envelope.event_sequence.to_string())
            .event(envelope.event_type.clone());
        Ok(match serde_json::to_string(&envelope) {
            Ok(json) => event.data(json),
            Err(_) => event.data("{}"),
        })
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
}

/// WebSocket transport for non-browser clients.
///
/// Authentication already happened in the v2 middleware, which also rejected any
/// token supplied through the query string or `Sec-WebSocket-Protocol`, so by the
/// time the upgrade runs the connection is known to be authorized.
#[utoipa::path(
    get,
    path = "/api/v2/ws",
    tag = "remote-control",
    params(
        ("after_sequence" = Option<u64>, Query, description = "Resume strictly after this event sequence"),
        ("instance_id" = Option<String>, Query, description = "Incarnation the cursor belongs to")
    ),
    responses(
        (status = 101, description = "Switching protocols"),
        (status = 401, description = "Missing bearer header, or credentials in query/subprotocol", body = super::dto::ApiError)
    )
)]
pub async fn ws(
    ws: WebSocketUpgrade,
    State(state): State<RemoteControlState>,
    Query(params): Query<StreamParams>,
) -> Response {
    ws.on_upgrade(move |socket| drive_socket(socket, state, params))
        .into_response()
}

async fn drive_socket(mut socket: WebSocket, state: RemoteControlState, params: StreamParams) {
    let mut stream = Box::pin(event_stream(state.projection.clone(), params));
    loop {
        tokio::select! {
            next = stream.next() => {
                let Some(envelope) = next else { break };
                let Ok(json) = serde_json::to_string(&envelope) else { continue };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}
