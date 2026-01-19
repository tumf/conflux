//! History management operations for agent command execution.

use crate::history::{
    AcceptanceAttempt, AcceptanceHistory, ApplyAttempt, ApplyHistory, ArchiveAttempt,
    ArchiveHistory,
};
use std::process::ExitStatus;
use std::time::Instant;

/// Record an apply attempt after streaming execution completes.
/// Call this after `run_apply_streaming()` child process finishes.
pub fn record_apply_attempt(
    apply_history: &mut ApplyHistory,
    change_id: &str,
    status: &ExitStatus,
    start: Instant,
    stdout_tail: Option<String>,
    stderr_tail: Option<String>,
) {
    let duration = start.elapsed();
    let attempt = ApplyAttempt {
        attempt: apply_history.count(change_id) + 1,
        success: status.success(),
        duration,
        error: if status.success() {
            None
        } else {
            Some(format!("Exit code: {:?}", status.code()))
        },
        exit_code: status.code(),
        stdout_tail,
        stderr_tail,
    };
    apply_history.record(change_id, attempt);
}

/// Record an archive attempt after streaming execution completes.
/// Call this after `run_archive_streaming()` child process finishes.
pub fn record_archive_attempt(
    archive_history: &mut ArchiveHistory,
    change_id: &str,
    status: &ExitStatus,
    start: Instant,
    verification_result: Option<String>,
    stdout_tail: Option<String>,
    stderr_tail: Option<String>,
) {
    let duration = start.elapsed();
    let attempt = ArchiveAttempt {
        attempt: archive_history.count(change_id) + 1,
        success: status.success(),
        duration,
        error: if status.success() && verification_result.is_none() {
            None
        } else if verification_result.is_some() {
            Some(format!(
                "Archive command succeeded but verification failed: {}",
                verification_result.as_ref().unwrap()
            ))
        } else {
            Some(format!("Exit code: {:?}", status.code()))
        },
        verification_result,
        exit_code: status.code(),
        stdout_tail,
        stderr_tail,
    };
    archive_history.record(change_id, attempt);
}

/// Record an acceptance attempt after streaming execution completes.
/// Call this after `run_acceptance_streaming()` child process finishes.
pub fn record_acceptance_attempt(
    acceptance_history: &mut AcceptanceHistory,
    change_id: &str,
    attempt: AcceptanceAttempt,
) {
    acceptance_history.record(change_id, attempt);
}

/// Clear apply history for a change (call after archiving)
pub fn clear_apply_history(apply_history: &mut ApplyHistory, change_id: &str) {
    apply_history.clear(change_id);
}

/// Clear archive history for a change (call after successful archiving)
pub fn clear_archive_history(archive_history: &mut ArchiveHistory, change_id: &str) {
    archive_history.clear(change_id);
}

/// Clear acceptance history for a change (call after successful archiving)
#[allow(dead_code)]
pub fn clear_acceptance_history(acceptance_history: &mut AcceptanceHistory, change_id: &str) {
    acceptance_history.clear(change_id);
}

/// Get the next acceptance attempt number for a change.
pub fn next_acceptance_attempt_number(
    acceptance_history: &AcceptanceHistory,
    change_id: &str,
) -> u32 {
    acceptance_history.count(change_id) + 1
}

/// Get the count of consecutive CONTINUE attempts for a change.
pub fn count_consecutive_acceptance_continues(
    acceptance_history: &AcceptanceHistory,
    change_id: &str,
) -> u32 {
    acceptance_history.count_consecutive_continues(change_id)
}
