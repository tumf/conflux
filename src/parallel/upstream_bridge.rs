//! Non-authoritative bridge from upstream lifecycle evidence to execution events.
//!
//! Upstream progress is reported through the existing log-event channel rather
//! than through new reducer state. That is deliberate: observability must not
//! become routing authority, and every excluded surface (serial mode, TUI,
//! server `git-sync`, per-change pre-sync, `PushToRemote`) keeps its current
//! routing because no new event variant reaches their reducers.
//!
//! A disabled run never constructs this observer, so it emits nothing.

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::events::LogEntry;
use crate::upstream::ports::{UpstreamEvent, UpstreamObserver};

use super::events::send_event;
use super::ParallelEvent;

/// Render one upstream lifecycle event as operator-visible text.
///
/// Kept pure so message content can be asserted without a channel.
pub(crate) fn render_upstream_event(event: &UpstreamEvent) -> (bool, String) {
    match event {
        UpstreamEvent::CheckpointStarted {
            remote,
            branch,
            trigger,
        } => (
            false,
            format!(
                "upstream: checkpoint started for {}/{} ({})",
                remote, branch, trigger
            ),
        ),
        UpstreamEvent::CheckpointDeferred { reason } => {
            (false, format!("upstream: checkpoint deferred ({})", reason))
        }
        UpstreamEvent::FetchCompleted {
            remote,
            branch,
            fetched_sha,
            local_sha,
        } => (
            false,
            format!(
                "upstream: fetched {}/{} at {} (local {})",
                remote, branch, fetched_sha, local_sha
            ),
        ),
        UpstreamEvent::NoOp { fetched_sha } => (
            false,
            format!("upstream: no-op, {} already integrated", fetched_sha),
        ),
        UpstreamEvent::IntegrationStarted { fetched_sha } => {
            (false, format!("upstream: integrating {}", fetched_sha))
        }
        UpstreamEvent::IntegrationCompleted { merge_sha } => {
            (false, format!("upstream: integrated as {}", merge_sha))
        }
        UpstreamEvent::Resolving { cause, attempt } => (
            false,
            format!("upstream: resolving {} (attempt {})", cause, attempt),
        ),
        UpstreamEvent::Reverifying { command } => {
            (false, format!("upstream: reverifying with `{}`", command))
        }
        UpstreamEvent::VerificationFailed { output_tail } => (
            true,
            format!("upstream: verification failed\n{}", output_tail),
        ),
        UpstreamEvent::Pushing {
            remote,
            branch,
            head,
        } => (
            false,
            format!("upstream: pushing {} to {}/{}", head, remote, branch),
        ),
        UpstreamEvent::PushFailed { classification } => {
            (true, format!("upstream: push failed ({})", classification))
        }
        UpstreamEvent::PushConfirmed {
            remote,
            branch,
            head,
        } => (
            false,
            format!("upstream: {}/{} confirmed at {}", remote, branch, head),
        ),
        UpstreamEvent::Stalled { reason } => (true, format!("upstream: stalled ({})", reason)),
        UpstreamEvent::Completed => (false, "upstream: cumulative base published".to_string()),
    }
}

/// Forwards upstream lifecycle evidence to the execution event channel.
pub struct EventUpstreamObserver {
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
}

impl EventUpstreamObserver {
    pub fn new(event_tx: Option<mpsc::Sender<ParallelEvent>>) -> Self {
        Self { event_tx }
    }
}

#[async_trait]
impl UpstreamObserver for EventUpstreamObserver {
    async fn observe(&self, event: UpstreamEvent) {
        let (is_warning, message) = render_upstream_event(&event);
        let entry = if is_warning {
            LogEntry::warn(&message)
        } else {
            LogEntry::info(&message)
        };
        send_event(&self.event_tx, ParallelEvent::Log(entry)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_integration_renders_operator_visible_outcomes() {
        let cases = [
            UpstreamEvent::CheckpointStarted {
                remote: "origin".into(),
                branch: "main".into(),
                trigger: "AfterDrain".into(),
            },
            UpstreamEvent::NoOp {
                fetched_sha: "abc".into(),
            },
            UpstreamEvent::IntegrationCompleted {
                merge_sha: "def".into(),
            },
            UpstreamEvent::Resolving {
                cause: "textual".into(),
                attempt: 1,
            },
            UpstreamEvent::Reverifying {
                command: "cargo test".into(),
            },
            UpstreamEvent::Pushing {
                remote: "origin".into(),
                branch: "main".into(),
                head: "ghi".into(),
            },
            UpstreamEvent::PushFailed {
                classification: "Race".into(),
            },
            UpstreamEvent::Stalled {
                reason: "no convergence".into(),
            },
            UpstreamEvent::Completed,
        ];
        for case in cases {
            let (_, message) = render_upstream_event(&case);
            assert!(message.starts_with("upstream: "), "message: {}", message);
        }
    }

    #[test]
    fn upstream_integration_marks_failures_as_warnings() {
        assert!(
            render_upstream_event(&UpstreamEvent::PushFailed {
                classification: "Stalled".into()
            })
            .0
        );
        assert!(render_upstream_event(&UpstreamEvent::Stalled { reason: "x".into() }).0);
        assert!(
            !render_upstream_event(&UpstreamEvent::NoOp {
                fetched_sha: "abc".into()
            })
            .0
        );
    }

    #[tokio::test]
    async fn upstream_integration_observer_without_channel_emits_nothing() {
        // A disabled run never constructs the observer at all; even so, an
        // observer with no channel must be inert rather than panic.
        let observer = EventUpstreamObserver::new(None);
        observer.observe(UpstreamEvent::Completed).await;
    }
}
