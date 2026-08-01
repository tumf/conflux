use crate::tui::events::{LogEntry, TuiCommand};
use crate::tui::types::AppMode;

use super::{guards, processing_logic, AppState, ChangeState};

/// Why a change is excluded from the bulk execution-mark toggle (`x`).
///
/// Every variant is user-actionable: the TUI reports the grouped reasons so a
/// partially applied bulk toggle never looks like it stopped halfway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BulkToggleExclusion {
    /// Running mode never converts an in-flight row into a stop request.
    ActiveInRunning,
    /// Parallel mode cannot queue a change that is not committed yet.
    ParallelUncommitted,
    /// Rejected proposals are read-only.
    Rejected,
}

impl BulkToggleExclusion {
    /// Ordering used when grouping exclusion reasons for display.
    const ALL: [BulkToggleExclusion; 3] = [
        BulkToggleExclusion::ActiveInRunning,
        BulkToggleExclusion::ParallelUncommitted,
        BulkToggleExclusion::Rejected,
    ];

    /// Short reason describing what the user can do about the exclusion.
    pub(super) fn reason(self) -> &'static str {
        match self {
            BulkToggleExclusion::ActiveInRunning => "in progress (use K to stop)",
            BulkToggleExclusion::ParallelUncommitted => {
                "uncommitted in parallel mode (commit first)"
            }
            BulkToggleExclusion::Rejected => "rejected and read-only",
        }
    }
}

/// Snapshot of the bulk toggle target set, taken once per operation.
///
/// Classification and target state come from the same snapshot so that every
/// eligible row receives the identical mark state.
pub(super) struct BulkToggleSnapshot {
    /// Indices (into `changes`) of the rows the operation applies to.
    pub(super) eligible: Vec<usize>,
    /// Excluded rows paired with their reason, in list order.
    pub(super) excluded: Vec<(String, BulkToggleExclusion)>,
    /// Mark state applied to every eligible row.
    ///
    /// `true` when at least one eligible row is unmarked (mark all), `false`
    /// when every eligible row is already marked (unmark all).
    pub(super) target_state: bool,
}

impl BulkToggleSnapshot {
    /// Grouped exclusion reasons with counts, e.g. `2 rejected and read-only`.
    pub(super) fn exclusion_summary(&self) -> String {
        BulkToggleExclusion::ALL
            .iter()
            .filter_map(|reason| {
                let count = self
                    .excluded
                    .iter()
                    .filter(|(_, actual)| actual == reason)
                    .count();
                (count > 0).then(|| format!("{} {}", count, reason.reason()))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Classifies a single change for bulk toggle; `None` means eligible.
pub(super) fn classify_bulk_toggle_change(
    mode: AppMode,
    parallel_mode: bool,
    change: &ChangeState,
) -> Option<BulkToggleExclusion> {
    if matches!(mode, AppMode::Running) && change.is_active_display_status() {
        return Some(BulkToggleExclusion::ActiveInRunning);
    }

    match guards::classify_toggle_block(
        change.is_parallel_eligible,
        parallel_mode,
        &change.display_status_cache,
    ) {
        Some(guards::ToggleBlockReason::ParallelUncommitted) => {
            Some(BulkToggleExclusion::ParallelUncommitted)
        }
        Some(guards::ToggleBlockReason::Rejected) => Some(BulkToggleExclusion::Rejected),
        None => None,
    }
}

/// Classifies every change once and derives the shared target mark state.
pub(super) fn build_bulk_toggle_snapshot(
    mode: AppMode,
    parallel_mode: bool,
    changes: &[ChangeState],
) -> BulkToggleSnapshot {
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();

    for (index, change) in changes.iter().enumerate() {
        match classify_bulk_toggle_change(mode.clone(), parallel_mode, change) {
            Some(reason) => excluded.push((change.id.clone(), reason)),
            None => eligible.push(index),
        }
    }

    // If any eligible row is unmarked, mark them all; otherwise unmark them all.
    let target_state = eligible.iter().any(|&index| !changes[index].selected);

    BulkToggleSnapshot {
        eligible,
        excluded,
        target_state,
    }
}

pub(super) fn can_bulk_toggle_change(
    mode: AppMode,
    parallel_mode: bool,
    change: &ChangeState,
) -> bool {
    classify_bulk_toggle_change(mode, parallel_mode, change).is_none()
}

/// Modes where the bulk execution-mark toggle is meaningful.
pub(super) fn is_bulk_toggle_mode(mode: &AppMode) -> bool {
    matches!(mode, AppMode::Select | AppMode::Stopped | AppMode::Running)
}

/// Applies the bulk execution-mark toggle and reports the outcome.
///
/// The target set is classified once up front, the same target state is applied
/// to every eligible row, and excluded rows are reported with their reason so a
/// partial-looking result is always explained.
pub(super) fn toggle_all_marks(state: &mut AppState) -> Vec<TuiCommand> {
    if !is_bulk_toggle_mode(&state.mode) {
        report_bulk_toggle_block(
            state,
            "Bulk mark (x) is only available in Select, Running, or Stopped mode".to_string(),
        );
        return Vec::new();
    }

    let snapshot =
        build_bulk_toggle_snapshot(state.mode.clone(), state.parallel_mode, &state.changes);

    if snapshot.eligible.is_empty() {
        let message = if snapshot.excluded.is_empty() {
            "Bulk mark (x) has no changes to toggle".to_string()
        } else {
            format!(
                "Bulk mark (x) has no eligible changes: {} excluded ({})",
                snapshot.excluded.len(),
                snapshot.exclusion_summary()
            )
        };
        report_bulk_toggle_block(state, message);
        return Vec::new();
    }

    let is_running = matches!(state.mode, AppMode::Running);
    let mut commands = Vec::new();

    for &index in &snapshot.eligible {
        if state.changes[index].selected == snapshot.target_state {
            continue;
        }

        state.changes[index].selected = snapshot.target_state;
        // Clear NEW flag when user interacts with the change
        if state.changes[index].is_new {
            state.changes[index].is_new = false;
            state.new_change_count = state.new_change_count.saturating_sub(1);
        }

        // In Running mode, emit queue commands for NotQueued/Queued rows
        // (same semantics as single-row Space toggle).
        // MergeWait/ResolveWait only toggle the execution mark.
        if is_running {
            match state.changes[index].display_status_cache.as_str() {
                "not queued" if snapshot.target_state => {
                    let id = state.changes[index].id.clone();
                    state.add_log(LogEntry::info(format!("Added to queue: {}", id)));
                    commands.push(TuiCommand::AddToQueue(id));
                }
                "queued" if !snapshot.target_state => {
                    let id = state.changes[index].id.clone();
                    state.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                    commands.push(TuiCommand::RemoveFromQueue(id));
                }
                _ => {}
            }
        }
    }

    let action = if snapshot.target_state {
        "marked"
    } else {
        "unmarked"
    };
    let mut summary = format!(
        "Toggled all: {} {} change(s)",
        snapshot.eligible.len(),
        action
    );
    if !snapshot.excluded.is_empty() {
        summary.push_str(&format!(
            ", {} excluded ({})",
            snapshot.excluded.len(),
            snapshot.exclusion_summary()
        ));
        // Surface exclusions where the user is looking, not only in the log pane.
        state.warning_message = Some(summary.clone());
    }
    state.add_log(LogEntry::info(summary));

    // Keep the shared process-local mark store in sync with the TUI projection.
    state.publish_execution_marks();

    commands
}

/// Records a bulk toggle that changed nothing, so `x` is never silent.
fn report_bulk_toggle_block(state: &mut AppState, message: String) {
    state.warning_message = Some(message.clone());
    state.add_log(LogEntry::warn(message));
}

pub(super) fn toggle_selection(state: &mut AppState) -> Option<TuiCommand> {
    if state.changes.is_empty() || state.cursor_index >= state.changes.len() {
        return None;
    }

    {
        let change = &state.changes[state.cursor_index];
        if let guards::ToggleGuardResult::Blocked(msg) = guards::validate_change_toggleable(
            change.is_parallel_eligible,
            state.parallel_mode,
            &change.display_status_cache,
            &change.id,
        ) {
            state.warning_message = Some(msg);
            return None;
        }
    }

    let mode = state.mode.clone();
    let mut new_change_count = state.new_change_count;
    let result = {
        let change = &mut state.changes[state.cursor_index];
        match mode {
            AppMode::Select => guards::handle_toggle_select_mode(change, &mut new_change_count),
            AppMode::Running => guards::handle_toggle_running_mode(change, &mut new_change_count),
            AppMode::Stopped => guards::handle_toggle_stopped_mode(change, &mut new_change_count),
            AppMode::Stopping
            | AppMode::Error
            | AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup
            | AppMode::ConfirmForceKill { .. } => return None,
        }
    };
    state.new_change_count = new_change_count;

    let command = dispatch_toggle_result(state, result);
    // Keep the shared process-local mark store in sync with the TUI projection so
    // other operator frontends observe the same intent.
    state.publish_execution_marks();
    command
}

fn dispatch_toggle_result(
    state: &mut AppState,
    result: guards::ToggleActionResult,
) -> Option<TuiCommand> {
    match result {
        guards::ToggleActionResult::StateOnly(log_msg) => {
            if let Some(msg) = log_msg {
                state.add_log(LogEntry::info(msg));
            }
            None
        }
        guards::ToggleActionResult::Command(cmd, log_msg) => {
            if let Some(msg) = log_msg {
                state.add_log(LogEntry::info(msg));
            }
            Some(cmd)
        }
        guards::ToggleActionResult::None => None,
    }
}

pub(super) fn start_processing(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Select {
        return None;
    }

    match processing_logic::collect_start_processing_targets(&state.changes, state.parallel_mode) {
        Ok(selected) => {
            processing_logic::mark_changes_queued(&mut state.changes, &selected);
            processing_logic::sync_queue_intent(
                state.shared_orchestrator_state.as_ref(),
                &selected,
            );
            state.reset_for_run();
            state.mode = AppMode::Running;
            state.add_log(LogEntry::info(format!(
                "Starting processing {} change(s)",
                selected.len()
            )));
            Some(processing_logic::build_start_command(selected))
        }
        Err(message) => {
            state.warning_message = Some(message);
            None
        }
    }
}

pub(super) fn resume_processing(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Stopped {
        return None;
    }

    match processing_logic::collect_resume_processing_targets(&state.changes) {
        Ok(marked_ids) => {
            processing_logic::mark_changes_queued(&mut state.changes, &marked_ids);
            processing_logic::sync_queue_intent(
                state.shared_orchestrator_state.as_ref(),
                &marked_ids,
            );
            state.reset_for_run();
            state.mode = AppMode::Running;
            state.add_log(LogEntry::info(format!(
                "Resuming processing {} change(s)...",
                marked_ids.len()
            )));
            Some(processing_logic::build_start_command(marked_ids))
        }
        Err(message) => {
            state.warning_message = Some(message);
            None
        }
    }
}

pub(super) fn retry_error_changes(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Error {
        return None;
    }

    let error_ids = processing_logic::collect_retry_error_targets(&state.changes);
    if error_ids.is_empty() {
        return None;
    }

    let retry_ids = processing_logic::sync_retry_error_intent(
        state.shared_orchestrator_state.as_ref(),
        &error_ids,
    );
    if retry_ids.is_empty() {
        state.warning_message = Some("No marked error changes are retryable".to_string());
        return None;
    }

    processing_logic::mark_changes_queued(&mut state.changes, &retry_ids);
    for entry in processing_logic::emit_retry_logs(&retry_ids) {
        state.add_log(entry);
    }

    // Operator retry must start the run with explicit-retry semantics so a
    // reconciled acceptance hold is consumed and acceptance resumes instead of
    // apply being rerun.
    state.set_pending_explicit_retry();
    state.reset_for_run();
    state.mode = AppMode::Running;
    state.publish_execution_marks();
    Some(processing_logic::build_start_command(retry_ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn make_change_state(
        id: &str,
        display_status_cache: &str,
        is_parallel_eligible: bool,
    ) -> ChangeState {
        ChangeState {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: display_status_cache.to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
            display_color_cache: Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        }
    }

    #[test]
    fn running_mode_excludes_active_rows_from_bulk_toggle() {
        let change = make_change_state("active", "applying", true);

        assert!(!can_bulk_toggle_change(AppMode::Running, false, &change));
        assert!(can_bulk_toggle_change(AppMode::Select, false, &change));
    }

    #[test]
    fn parallel_mode_excludes_uncommitted_rows_from_bulk_toggle() {
        let ineligible = make_change_state("uncommitted", "not queued", false);

        assert!(!can_bulk_toggle_change(AppMode::Select, true, &ineligible));
        assert!(can_bulk_toggle_change(AppMode::Select, false, &ineligible));
    }

    #[test]
    fn classify_bulk_toggle_change_reports_actionable_reasons() {
        let active = make_change_state("active", "applying", true);
        let rejected = make_change_state("rejected", "rejected", true);
        let uncommitted = make_change_state("uncommitted", "not queued", false);
        let eligible = make_change_state("eligible", "not queued", true);

        assert_eq!(
            classify_bulk_toggle_change(AppMode::Running, false, &active),
            Some(BulkToggleExclusion::ActiveInRunning)
        );
        assert_eq!(
            classify_bulk_toggle_change(AppMode::Select, false, &rejected),
            Some(BulkToggleExclusion::Rejected)
        );
        assert_eq!(
            classify_bulk_toggle_change(AppMode::Select, true, &uncommitted),
            Some(BulkToggleExclusion::ParallelUncommitted)
        );
        assert_eq!(
            classify_bulk_toggle_change(AppMode::Select, false, &eligible),
            None
        );

        // Every reason must give the user something to act on.
        for reason in BulkToggleExclusion::ALL {
            assert!(!reason.reason().is_empty());
        }
    }

    #[test]
    fn snapshot_marks_all_when_any_eligible_row_is_unmarked() {
        let mut changes = vec![
            make_change_state("a", "not queued", true),
            make_change_state("b", "not queued", true),
            make_change_state("c", "not queued", true),
        ];
        changes[0].selected = true;

        let snapshot = build_bulk_toggle_snapshot(AppMode::Select, false, &changes);

        assert_eq!(snapshot.eligible, vec![0, 1, 2]);
        assert!(snapshot.excluded.is_empty());
        assert!(
            snapshot.target_state,
            "a partially marked target set must mark all eligible rows"
        );
    }

    #[test]
    fn snapshot_unmarks_all_when_every_eligible_row_is_marked() {
        let mut changes = vec![
            make_change_state("a", "not queued", true),
            make_change_state("b", "rejected", true),
        ];
        changes[0].selected = true;
        // Ineligible rows must not influence the target state.
        changes[1].selected = false;

        let snapshot = build_bulk_toggle_snapshot(AppMode::Select, false, &changes);

        assert_eq!(snapshot.eligible, vec![0]);
        assert_eq!(
            snapshot.excluded,
            vec![("b".to_string(), BulkToggleExclusion::Rejected)]
        );
        assert!(
            !snapshot.target_state,
            "a fully marked target set must unmark all eligible rows"
        );
    }

    #[test]
    fn snapshot_exclusion_summary_groups_reasons_with_counts() {
        let changes = vec![
            make_change_state("active-a", "applying", true),
            make_change_state("active-b", "archiving", true),
            make_change_state("rejected", "rejected", true),
            make_change_state("eligible", "not queued", true),
        ];

        let snapshot = build_bulk_toggle_snapshot(AppMode::Running, false, &changes);

        assert_eq!(snapshot.eligible, vec![3]);
        assert_eq!(snapshot.excluded.len(), 3);
        assert_eq!(
            snapshot.exclusion_summary(),
            format!(
                "2 {}, 1 {}",
                BulkToggleExclusion::ActiveInRunning.reason(),
                BulkToggleExclusion::Rejected.reason()
            )
        );
    }

    #[test]
    fn snapshot_with_no_changes_has_empty_target_set() {
        let snapshot = build_bulk_toggle_snapshot(AppMode::Select, false, &[]);

        assert!(snapshot.eligible.is_empty());
        assert!(snapshot.excluded.is_empty());
        assert!(!snapshot.target_state);
        assert_eq!(snapshot.exclusion_summary(), "");
    }

    #[test]
    fn is_bulk_toggle_mode_covers_only_interactive_modes() {
        assert!(is_bulk_toggle_mode(&AppMode::Select));
        assert!(is_bulk_toggle_mode(&AppMode::Running));
        assert!(is_bulk_toggle_mode(&AppMode::Stopped));
        assert!(!is_bulk_toggle_mode(&AppMode::Error));
        assert!(!is_bulk_toggle_mode(&AppMode::Stopping));
    }
}
