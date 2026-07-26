//! External lifecycle integration.
//!
//! Conflux can start one optional external *lifecycle adapter* as a child
//! process and stream newline-delimited JSON lifecycle messages to its stdin.
//! The adapter inherits the cflx process environment, which lets a terminal
//! manager adapter (for example a Herdr adapter) read `HERDR_ENV`,
//! `HERDR_SOCKET_PATH`, and `HERDR_PANE_ID` without Conflux depending on any
//! specific terminal manager.
//!
//! The integration is **observability-only**:
//!
//! - spawn failure warns once and disables the adapter for the process
//! - broken stdin, early child exit, or a write timeout warns and disables further sends
//! - a full queue drops redundant state refreshes rather than blocking workflow execution
//! - shutdown waits only for a bounded deadline, then terminates the adapter child
//! - adapter output and exit status never alter workflow routing
//!
//! Payloads are restricted to the fields defined by [`LifecycleMessage`]. They
//! never carry environment values, credentials, prompts, terminal buffers, or
//! error bodies.

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::config::LifecycleIntegrationConfig;

/// Version of the newline-delimited JSON lifecycle protocol.
///
/// Adapters MUST check this field and ignore messages with an unknown major
/// version instead of guessing at the payload shape.
pub const LIFECYCLE_PROTOCOL_VERSION: u32 = 1;

// ── Protocol model ─────────────────────────────────────────────────────────

/// How this cflx process was invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleExecutionMode {
    /// Interactive TUI (`cflx` or `cflx tui`).
    Tui,
    /// Non-interactive orchestration (`cflx run`).
    Run,
}

/// Semantic lifecycle state exposed to external tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Ready/selection UI or halted processing; nothing is executing.
    Idle,
    /// Orchestration (or graceful stop of orchestration) is executing.
    Working,
    /// A decision requiring user input is pending.
    Blocked,
}

/// Kind of lifecycle message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleEventKind {
    /// The cflx process started and the adapter is live.
    ProcessStarted,
    /// The semantic state changed.
    StateChanged,
    /// An optional session identity became known.
    SessionIdentified,
    /// The cflx process is shutting down.
    ProcessStopping,
}

/// Privacy-limited context attached to lifecycle messages.
///
/// Only fields declared here may ever be serialized. Adding a field is a
/// protocol change and requires an explicit specification update.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleContext {
    /// Absolute workspace/repository root path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Public OpenSpec change identifier currently in focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_id: Option<String>,
    /// Opaque session identifier reported by the frontend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl LifecycleContext {
    /// Context carrying only the workspace root.
    pub fn workspace(path: impl Into<String>) -> Self {
        Self {
            workspace: Some(path.into()),
            ..Default::default()
        }
    }

    /// Return true when no context field is populated.
    pub fn is_empty(&self) -> bool {
        self.workspace.is_none() && self.change_id.is_none() && self.session_id.is_none()
    }

    /// Attach a change identifier.
    pub fn with_change_id(mut self, change_id: Option<String>) -> Self {
        self.change_id = change_id;
        self
    }
}

/// A single serialized lifecycle message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleMessage {
    /// Protocol version; see [`LIFECYCLE_PROTOCOL_VERSION`].
    pub protocol_version: u32,
    /// Monotonically increasing, gap-free sequence number for this process.
    pub sequence: u64,
    /// Message kind.
    pub kind: LifecycleEventKind,
    /// Execution mode of the reporting cflx process.
    pub mode: LifecycleExecutionMode,
    /// Process ID of the reporting cflx process.
    pub pid: u32,
    /// Semantic state, present for [`LifecycleEventKind::StateChanged`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<LifecycleState>,
    /// Privacy-limited context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<LifecycleContext>,
}

impl LifecycleMessage {
    /// Serialize as one compact JSON line terminated by `\n`.
    pub fn to_jsonl(&self) -> String {
        // Serialization of this fixed-shape struct cannot fail; fall back to a
        // minimal well-formed line rather than panicking in an observability path.
        match serde_json::to_string(self) {
            Ok(mut line) => {
                line.push('\n');
                line
            }
            Err(err) => {
                warn!("Failed to serialize lifecycle message: {}", err);
                String::new()
            }
        }
    }
}

/// A lifecycle event published by a producer (TUI, run mode, orchestration).
///
/// Producers never know the adapter command or transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// The cflx process started.
    ProcessStarted { context: LifecycleContext },
    /// The semantic state changed.
    StateChanged {
        state: LifecycleState,
        context: LifecycleContext,
    },
    /// A session identity became known.
    ///
    /// Session identity is optional in the protocol and Conflux has no
    /// in-process producer for it yet; it exists so adapters can be written
    /// against the full message set. Fixture adapters exercise it in tests.
    #[allow(dead_code)]
    SessionIdentified { session_id: String },
    /// The cflx process is stopping.
    ProcessStopping,
}

/// Build the serializable message for an event using an explicit sequence number.
///
/// Kept separate from transport so message shape is unit-testable without
/// spawning a child process.
pub fn build_message(
    event: &LifecycleEvent,
    sequence: u64,
    mode: LifecycleExecutionMode,
    pid: u32,
) -> LifecycleMessage {
    let (kind, state, context) = match event {
        LifecycleEvent::ProcessStarted { context } => (
            LifecycleEventKind::ProcessStarted,
            None,
            Some(context.clone()),
        ),
        LifecycleEvent::StateChanged { state, context } => (
            LifecycleEventKind::StateChanged,
            Some(*state),
            Some(context.clone()),
        ),
        LifecycleEvent::SessionIdentified { session_id } => (
            LifecycleEventKind::SessionIdentified,
            None,
            Some(LifecycleContext {
                session_id: Some(session_id.clone()),
                ..Default::default()
            }),
        ),
        LifecycleEvent::ProcessStopping => (LifecycleEventKind::ProcessStopping, None, None),
    };

    LifecycleMessage {
        protocol_version: LIFECYCLE_PROTOCOL_VERSION,
        sequence,
        kind,
        mode,
        pid,
        state,
        context: context.filter(|ctx| !ctx.is_empty()),
    }
}

// ── Deduplication ──────────────────────────────────────────────────────────

/// Tracks the last emitted semantic state so unchanged states are not repeated.
#[derive(Debug, Default)]
pub struct LifecycleStateTracker {
    last: Option<(LifecycleState, LifecycleContext)>,
}

impl LifecycleStateTracker {
    /// Return true when this state/context pair differs from the last emitted one.
    ///
    /// Records the pair as emitted when it returns true.
    pub fn should_emit(&mut self, state: LifecycleState, context: &LifecycleContext) -> bool {
        if self
            .last
            .as_ref()
            .is_some_and(|(last_state, last_ctx)| *last_state == state && last_ctx == context)
        {
            return false;
        }
        self.last = Some((state, context.clone()));
        true
    }
}

// ── Publisher abstraction ──────────────────────────────────────────────────

/// Transport-agnostic lifecycle publisher.
///
/// Publishing is synchronous and non-blocking so producers on hot paths (the
/// TUI draw loop, orchestration event dispatch) can never be stalled by an
/// adapter.
pub trait LifecyclePublisher: Send + Sync {
    /// Publish one lifecycle event, dropping it if the transport is saturated.
    fn publish(&self, event: LifecycleEvent);
}

/// Cheap clonable handle passed to producers.
///
/// A disabled handle is a no-op, which is the default when no adapter is configured.
#[derive(Clone, Default)]
pub struct LifecycleHandle {
    publisher: Option<Arc<dyn LifecyclePublisher>>,
}

impl std::fmt::Debug for LifecycleHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleHandle")
            .field("enabled", &self.publisher.is_some())
            .finish()
    }
}

impl LifecycleHandle {
    /// A handle that discards every event.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Wrap a concrete publisher.
    pub fn from_publisher(publisher: Arc<dyn LifecyclePublisher>) -> Self {
        Self {
            publisher: Some(publisher),
        }
    }

    /// Return true when an adapter is active.
    pub fn is_enabled(&self) -> bool {
        self.publisher.is_some()
    }

    /// Publish an event; a no-op when disabled.
    pub fn publish(&self, event: LifecycleEvent) {
        if let Some(publisher) = &self.publisher {
            publisher.publish(event);
        }
    }

    /// Publish a semantic state change.
    pub fn publish_state(&self, state: LifecycleState, context: LifecycleContext) {
        self.publish(LifecycleEvent::StateChanged { state, context });
    }
}

// ── Dispatcher ─────────────────────────────────────────────────────────────

/// Owns the adapter child process, serialization, deduplication, bounded
/// buffering, and bounded shutdown.
pub struct LifecycleDispatcher {
    sender: Mutex<Option<mpsc::Sender<LifecycleEvent>>>,
    tracker: Mutex<LifecycleStateTracker>,
    dropped: AtomicU64,
    drop_warned: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    shutdown_timeout: Duration,
}

impl LifecyclePublisher for LifecycleDispatcher {
    fn publish(&self, event: LifecycleEvent) {
        // Deduplicate unchanged semantic states before touching the queue.
        if let LifecycleEvent::StateChanged { state, context } = &event {
            let mut tracker = match self.tracker.lock() {
                Ok(tracker) => tracker,
                Err(poisoned) => poisoned.into_inner(),
            };
            if !tracker.should_emit(*state, context) {
                return;
            }
        }

        let sender = {
            let guard = match self.sender.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.clone()
        };

        let Some(sender) = sender else {
            return;
        };

        if sender.try_send(event).is_err() {
            // Full queue or closed transport: drop rather than block workflow execution.
            let dropped = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
            if !self.drop_warned.swap(true, Ordering::Relaxed) {
                warn!(
                    "External lifecycle adapter is not keeping up; dropping lifecycle messages (dropped={dropped})"
                );
            } else {
                debug!("Dropped lifecycle message (total dropped={dropped})");
            }
        }
    }
}

impl LifecycleDispatcher {
    /// Number of lifecycle messages dropped because the bounded queue was full.
    pub fn dropped_messages(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Close the queue and wait for the adapter within the bounded deadline.
    async fn shutdown(&self) {
        self.publish(LifecycleEvent::ProcessStopping);

        // Dropping the sender closes the channel, which makes the worker flush,
        // close adapter stdin, and reap the child.
        {
            let mut guard = match self.sender.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take();
        }

        let worker = {
            let mut guard = match self.worker.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };

        if let Some(worker) = worker {
            match tokio::time::timeout(self.shutdown_timeout, worker).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => debug!("Lifecycle adapter worker ended abnormally: {}", err),
                Err(_) => {
                    // Bounded deadline exceeded: the worker future is dropped, which
                    // drops the child handle. `kill_on_drop` terminates the adapter.
                    warn!(
                        "External lifecycle adapter did not shut down within {:?}; terminating it",
                        self.shutdown_timeout
                    );
                }
            }
        }

        // Final diagnostic summary; observability only, never an error condition.
        let dropped = self.dropped_messages();
        if dropped > 0 {
            warn!(
                "External lifecycle adapter could not keep up; {} lifecycle message(s) were dropped",
                dropped
            );
        }
    }
}

/// Owner of an optional lifecycle integration for one cflx process.
pub struct LifecycleIntegration {
    dispatcher: Option<Arc<LifecycleDispatcher>>,
}

impl LifecycleIntegration {
    /// A disabled integration (no adapter process, no events).
    pub fn disabled() -> Self {
        Self { dispatcher: None }
    }

    /// Start the configured adapter, if any.
    ///
    /// Never fails: configuration and spawn problems are reported as warnings
    /// and downgrade the integration to a no-op so the requested TUI or run
    /// operation always continues.
    pub fn start(
        config: Option<&LifecycleIntegrationConfig>,
        mode: LifecycleExecutionMode,
    ) -> Self {
        let Some(config) = config else {
            return Self::disabled();
        };

        if !config.is_enabled() {
            debug!("External lifecycle integration is configured but disabled");
            return Self::disabled();
        }

        if let Err(err) = config.validate() {
            warn!("External lifecycle integration disabled: {}", err);
            return Self::disabled();
        }

        let (program, args) = config
            .command
            .split_first()
            .expect("validated non-empty argv");

        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            // The adapter shares the terminal with the TUI; never let it write there.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                warn!(
                    "External lifecycle adapter `{}` could not be started ({}); continuing without lifecycle reporting. Check `lifecycle_integration.command` in your cflx configuration.",
                    program, err
                );
                return Self::disabled();
            }
        };

        let Some(mut stdin) = child.stdin.take() else {
            warn!("External lifecycle adapter stdin was unavailable; continuing without lifecycle reporting");
            return Self::disabled();
        };

        let (tx, mut rx) = mpsc::channel::<LifecycleEvent>(config.queue_capacity());
        let write_timeout = Duration::from_millis(config.write_timeout_ms());
        let shutdown_timeout = Duration::from_millis(config.shutdown_timeout_ms());
        let pid = std::process::id();

        let worker = tokio::spawn(async move {
            let mut sequence: u64 = 0;
            let mut writable = true;

            while let Some(event) = rx.recv().await {
                if !writable {
                    // Keep draining so producers never block on a dead adapter.
                    continue;
                }

                sequence += 1;
                let line = build_message(&event, sequence, mode, pid).to_jsonl();
                if line.is_empty() {
                    continue;
                }

                match tokio::time::timeout(write_timeout, stdin.write_all(line.as_bytes())).await {
                    Ok(Ok(())) => match tokio::time::timeout(write_timeout, stdin.flush()).await {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => {
                            warn!("External lifecycle adapter flush failed ({}); disabling lifecycle reporting", err);
                            writable = false;
                        }
                        Err(_) => {
                            warn!("External lifecycle adapter stopped reading; disabling lifecycle reporting");
                            writable = false;
                        }
                    },
                    Ok(Err(err)) => {
                        warn!(
                            "External lifecycle adapter write failed ({}); disabling lifecycle reporting",
                            err
                        );
                        writable = false;
                    }
                    Err(_) => {
                        warn!("External lifecycle adapter stopped reading; disabling lifecycle reporting");
                        writable = false;
                    }
                }
            }

            // Closing stdin signals end-of-stream to the adapter.
            let _ = stdin.shutdown().await;
            drop(stdin);
            let _ = child.wait().await;
        });

        Self {
            dispatcher: Some(Arc::new(LifecycleDispatcher {
                sender: Mutex::new(Some(tx)),
                tracker: Mutex::new(LifecycleStateTracker::default()),
                dropped: AtomicU64::new(0),
                drop_warned: AtomicBool::new(false),
                worker: Mutex::new(Some(worker)),
                shutdown_timeout,
            })),
        }
    }

    /// Return a cheap clonable handle for producers.
    pub fn handle(&self) -> LifecycleHandle {
        match &self.dispatcher {
            Some(dispatcher) => {
                LifecycleHandle::from_publisher(dispatcher.clone() as Arc<dyn LifecyclePublisher>)
            }
            None => LifecycleHandle::disabled(),
        }
    }

    /// Return true when an adapter process is active.
    ///
    /// Library/test surface: entrypoints deliberately stay branch-free and just
    /// publish through the handle, which is a no-op when no adapter is running.
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.dispatcher.is_some()
    }

    /// Emit `process_stopping` and shut the adapter down within its bounded deadline.
    ///
    /// Safe to call more than once; later calls are no-ops.
    pub async fn shutdown(&self) {
        if let Some(dispatcher) = &self.dispatcher {
            dispatcher.shutdown().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(event: &LifecycleEvent, sequence: u64) -> LifecycleMessage {
        build_message(event, sequence, LifecycleExecutionMode::Tui, 4242)
    }

    #[test]
    fn process_started_message_carries_version_sequence_mode_and_pid() {
        let event = LifecycleEvent::ProcessStarted {
            context: LifecycleContext::workspace("/repo"),
        };

        let built = message(&event, 1);

        assert_eq!(built.protocol_version, LIFECYCLE_PROTOCOL_VERSION);
        assert_eq!(built.sequence, 1);
        assert_eq!(built.kind, LifecycleEventKind::ProcessStarted);
        assert_eq!(built.mode, LifecycleExecutionMode::Tui);
        assert_eq!(built.pid, 4242);
        assert_eq!(built.state, None);
        assert_eq!(
            built.context.and_then(|ctx| ctx.workspace),
            Some("/repo".to_string())
        );
    }

    #[test]
    fn state_changed_message_carries_semantic_state_and_change_id() {
        let event = LifecycleEvent::StateChanged {
            state: LifecycleState::Blocked,
            context: LifecycleContext::workspace("/repo")
                .with_change_id(Some("change-a".to_string())),
        };

        let built = message(&event, 7);

        assert_eq!(built.kind, LifecycleEventKind::StateChanged);
        assert_eq!(built.state, Some(LifecycleState::Blocked));
        let context = built.context.expect("context present");
        assert_eq!(context.change_id.as_deref(), Some("change-a"));
    }

    #[test]
    fn process_stopping_message_has_no_state_or_context() {
        let built = message(&LifecycleEvent::ProcessStopping, 9);

        assert_eq!(built.kind, LifecycleEventKind::ProcessStopping);
        assert_eq!(built.state, None);
        assert_eq!(built.context, None);
    }

    #[test]
    fn session_identified_message_reports_session_only() {
        let built = message(
            &LifecycleEvent::SessionIdentified {
                session_id: "session-1".to_string(),
            },
            3,
        );

        assert_eq!(built.kind, LifecycleEventKind::SessionIdentified);
        let context = built.context.expect("context present");
        assert_eq!(context.session_id.as_deref(), Some("session-1"));
        assert_eq!(context.workspace, None);
        assert_eq!(context.change_id, None);
    }

    #[test]
    fn serialized_line_is_single_compact_json_object() {
        let line = message(
            &LifecycleEvent::StateChanged {
                state: LifecycleState::Working,
                context: LifecycleContext::workspace("/repo"),
            },
            2,
        )
        .to_jsonl();

        assert!(line.ends_with('\n'), "line must be newline terminated");
        assert_eq!(line.matches('\n').count(), 1, "one message per line");

        let parsed: serde_json::Value =
            serde_json::from_str(line.trim_end()).expect("line must be valid JSON");
        assert_eq!(parsed["protocol_version"], 1);
        assert_eq!(parsed["kind"], "state_changed");
        assert_eq!(parsed["state"], "working");
        assert_eq!(parsed["mode"], "tui");
        assert_eq!(parsed["sequence"], 2);
    }

    #[test]
    fn serialized_payload_exposes_only_allowed_fields() {
        let line = message(
            &LifecycleEvent::StateChanged {
                state: LifecycleState::Idle,
                context: LifecycleContext::workspace("/repo")
                    .with_change_id(Some("change-a".to_string())),
            },
            5,
        )
        .to_jsonl();

        let parsed: serde_json::Value = serde_json::from_str(line.trim_end()).expect("valid JSON");
        let object = parsed.as_object().expect("object payload");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "context",
                "kind",
                "mode",
                "pid",
                "protocol_version",
                "sequence",
                "state"
            ]
        );

        let context = object["context"].as_object().expect("context object");
        let mut context_keys: Vec<&str> = context.keys().map(String::as_str).collect();
        context_keys.sort_unstable();
        assert_eq!(context_keys, vec!["change_id", "workspace"]);
    }

    #[test]
    fn empty_context_is_omitted_from_payload() {
        let built = message(
            &LifecycleEvent::ProcessStarted {
                context: LifecycleContext::default(),
            },
            1,
        );

        assert_eq!(built.context, None);
        let parsed: serde_json::Value =
            serde_json::from_str(built.to_jsonl().trim_end()).expect("valid JSON");
        assert!(parsed.get("context").is_none());
    }

    #[test]
    fn state_tracker_suppresses_unchanged_states() {
        let mut tracker = LifecycleStateTracker::default();
        let context = LifecycleContext::workspace("/repo");

        assert!(tracker.should_emit(LifecycleState::Idle, &context));
        assert!(!tracker.should_emit(LifecycleState::Idle, &context));
        assert!(tracker.should_emit(LifecycleState::Working, &context));
        assert!(!tracker.should_emit(LifecycleState::Working, &context));
        assert!(tracker.should_emit(LifecycleState::Idle, &context));
    }

    #[test]
    fn state_tracker_treats_context_change_as_a_new_state() {
        let mut tracker = LifecycleStateTracker::default();
        let base = LifecycleContext::workspace("/repo");
        let with_change =
            LifecycleContext::workspace("/repo").with_change_id(Some("change-a".to_string()));

        assert!(tracker.should_emit(LifecycleState::Working, &base));
        assert!(tracker.should_emit(LifecycleState::Working, &with_change));
        assert!(!tracker.should_emit(LifecycleState::Working, &with_change));
    }

    #[test]
    fn disabled_handle_is_a_noop() {
        let handle = LifecycleHandle::disabled();

        assert!(!handle.is_enabled());
        handle.publish(LifecycleEvent::ProcessStopping);
    }

    #[test]
    fn integration_without_configuration_is_disabled() {
        let integration = LifecycleIntegration::start(None, LifecycleExecutionMode::Run);

        assert!(!integration.is_enabled());
        assert!(!integration.handle().is_enabled());
    }

    #[test]
    fn integration_with_disabled_configuration_is_disabled() {
        let config = LifecycleIntegrationConfig {
            enabled: Some(false),
            command: vec!["adapter".to_string()],
            ..Default::default()
        };

        let integration = LifecycleIntegration::start(Some(&config), LifecycleExecutionMode::Run);

        assert!(!integration.is_enabled());
    }

    #[test]
    fn integration_with_invalid_configuration_is_disabled() {
        let config = LifecycleIntegrationConfig {
            enabled: Some(true),
            command: Vec::new(),
            ..Default::default()
        };

        let integration = LifecycleIntegration::start(Some(&config), LifecycleExecutionMode::Tui);

        assert!(!integration.is_enabled());
    }
}
