//! Dynamic queue handling for runtime change additions (TUI mode).
//!
//! This module provides utilities for:
//! - Checking if debounce period has elapsed for queue changes
//! - Queue state management for re-analysis triggers

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Check if debounce period has elapsed for queue changes.
///
/// Returns `true` if:
/// - No recent queue changes, OR
/// - 10 seconds have passed since the last queue change
///
/// This prevents immediate re-analysis when the queue changes, giving time for
/// multiple changes to be queued before triggering expensive re-analysis.
pub async fn should_reanalyze_queue(
    last_queue_change_at: &Arc<Mutex<Option<std::time::Instant>>>,
    bypass_debounce: bool,
) -> bool {
    if bypass_debounce {
        info!("Bypassing queue debounce because execution capacity recovered");
        return true;
    }

    let last_change = last_queue_change_at.lock().await;
    match *last_change {
        None => {
            // No recent queue changes, proceed with re-analysis
            true
        }
        Some(timestamp) => {
            let elapsed = timestamp.elapsed();
            let debounce_duration = std::time::Duration::from_secs(10);

            if elapsed >= debounce_duration {
                info!(
                    "Debounce period elapsed ({:.1}s >= 10s), proceeding with re-analysis",
                    elapsed.as_secs_f64()
                );
                true
            } else {
                info!(
                    "Debounce period active ({:.1}s < 10s), deferring re-analysis",
                    elapsed.as_secs_f64()
                );
                false
            }
        }
    }
}

/// Reason for triggering re-analysis (for logging and diagnostics)
#[derive(Debug, Clone, Copy)]
pub enum ReanalysisReason {
    /// Initial analysis (first iteration)
    Initial,
    /// Task completion (apply/archive/acceptance finished)
    Completion,
    /// Manual resolve completed and released a scheduler slot.
    ResolveCompletion,
    /// Available slots transitioned from zero to positive while queued work exists.
    SlotRecovery,
    /// Queue notification (dynamic queue has new items)
    QueueNotification,
}

impl std::fmt::Display for ReanalysisReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReanalysisReason::Initial => write!(f, "initial"),
            ReanalysisReason::Completion => write!(f, "completion"),
            ReanalysisReason::ResolveCompletion => write!(f, "resolve_completion"),
            ReanalysisReason::SlotRecovery => write!(f, "slot_recovery"),
            ReanalysisReason::QueueNotification => write!(f, "queue"),
        }
    }
}
