use crate::orchestration::operator_command::{plan_bulk_marks, MarkExclusion, MarkTargetRow};
use crate::tui::events::{LogEntry, TuiCommand};

use super::{guards, AppState, ChangeState};

/// Why a change is excluded from the bulk execution-mark toggle (`x`).
///
/// Re-exported from the shared operator vocabulary rather than defined here: a
/// TUI-local exclusion enum would be free to disagree with the reason
/// `/api/v2` reports for the identical row. Only [`MarkExclusion::FinalStatus`]
/// and [`MarkExclusion::ArchiveComplete`] are reachable — a terminal row and a
/// reducer-archived row are the two things that are not mark targets.
pub(super) type BulkToggleExclusion = MarkExclusion;

/// Snapshot of the bulk toggle target set, taken once per operation.
///
/// Classification and target state come from the same snapshot so that every
/// eligible row receives the identical mark state. The classification itself is
/// [`plan_bulk_marks`], the same function the `/api/v2`
/// `set_all_execution_marks` command runs, so both frontends derive one target
/// set from the same rules.
pub(super) struct BulkToggleSnapshot {
    /// Indices (into `changes`) of the rows the operation applies to.
    pub(super) eligible: Vec<usize>,
    /// Terminal rows the plan excluded, in list order.
    ///
    /// Never surfaced as a warning: excluding a row that has no next run is not
    /// a refusal of anything the operator asked for.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) excluded: Vec<(String, BulkToggleExclusion)>,
    /// Mark state applied to every eligible row.
    ///
    /// `true` when at least one eligible row is unmarked (mark all), `false`
    /// when every eligible row is already marked (unmark all).
    pub(super) target_state: bool,
}

/// Classifies a single change for bulk toggle; `None` means eligible.
pub(super) fn classify_bulk_toggle_change(change: &ChangeState) -> Option<BulkToggleExclusion> {
    crate::orchestration::operator_command::classify_bulk_mark_row(
        &change.display_status_cache,
        change.archive_complete_cache,
    )
}

/// Classifies every change once and derives the shared target mark state.
pub(super) fn build_bulk_toggle_snapshot(changes: &[ChangeState]) -> BulkToggleSnapshot {
    let rows: Vec<MarkTargetRow<'_>> = changes
        .iter()
        .map(|change| MarkTargetRow {
            change_id: &change.id,
            display_status: &change.display_status_cache,
            // The synchronized presentation cache *is* the TUI's copy of the
            // reducer record; the shared classifier takes it as evidence so
            // orchestration never has to read TUI state.
            archive_complete: change.archive_complete_cache,
            marked: change.selected,
        })
        .collect();
    let plan = plan_bulk_marks(&rows);

    // The plan names changes; the TUI mutates rows, so translate once here
    // rather than letting the row list and the plan drift apart.
    let eligible = plan
        .eligible
        .iter()
        .filter_map(|id| changes.iter().position(|change| &change.id == id))
        .collect();

    BulkToggleSnapshot {
        eligible,
        excluded: plan.excluded,
        target_state: plan.target_state,
    }
}

pub(super) fn can_bulk_toggle_change(change: &ChangeState) -> bool {
    classify_bulk_toggle_change(change).is_none()
}

/// Applies the bulk execution-mark toggle and reports the outcome.
///
/// The target set is classified once up front and the same target state is
/// applied to every visible non-terminal row, in every execution mode. Nothing
/// but the execution mark is written: no queue command is emitted, and work
/// already admitted to the current run is untouched.
pub(super) fn toggle_all_marks(state: &mut AppState) -> Vec<TuiCommand> {
    // Overlays own input; `x` must never reach the underlying view. Key routing
    // already consumes the key, so this is the defense-in-depth half of the same
    // rule and stays silent rather than reporting a block the operator did not ask
    // for.
    if state.has_overlay() {
        return Vec::new();
    }

    let snapshot = build_bulk_toggle_snapshot(&state.changes);

    if snapshot.eligible.is_empty() {
        // A list that holds only terminal rows is a silent no-op: nothing was
        // refused, there is simply no run candidate left to express intent
        // about. Any other empty target set is a condition worth naming.
        if state.changes.is_empty() {
            report_bulk_toggle_block(state, "Bulk mark (x) has no changes to toggle".to_string());
        }
        return Vec::new();
    }

    for &index in &snapshot.eligible {
        if state.changes[index].selected == snapshot.target_state {
            continue;
        }

        state.changes[index].selected = snapshot.target_state;
        // One target, one write. The whole-row publish this replaced derived the
        // store from every cached row, so a row a concurrent event had already
        // invalidated came back marked.
        let toggled_id = state.changes[index].id.clone();
        state.request_mark_write(&toggled_id, snapshot.target_state);
        // Clear NEW flag when user interacts with the change
        if state.changes[index].is_new {
            state.changes[index].is_new = false;
            state.new_change_count = state.new_change_count.saturating_sub(1);
        }
    }

    let action = if snapshot.target_state {
        "marked"
    } else {
        "unmarked"
    };
    state.add_log(LogEntry::info(format!(
        "Toggled all: {} {} change(s)",
        snapshot.eligible.len(),
        action
    )));

    Vec::new()
}

/// Records a bulk toggle that had no target for a reason worth naming.
fn report_bulk_toggle_block(state: &mut AppState, message: String) {
    state.warning_message = Some(message.clone());
    state.add_log(LogEntry::warn(message));
}

/// Toggle the cursor row's execution mark.
///
/// Every visible non-terminal row accepts this in every execution mode. A
/// terminal row is a silent no-op — it is not a run candidate, so there is
/// nothing to refuse and nothing to warn about.
pub(super) fn toggle_selection(state: &mut AppState) {
    if state.changes.is_empty() || state.cursor_index >= state.changes.len() {
        return;
    }

    if !guards::is_mark_target(&state.changes[state.cursor_index]) {
        return;
    }

    let mut new_change_count = state.new_change_count;
    let (target_id, marked, log_msg) = {
        let change = &mut state.changes[state.cursor_index];
        let log_msg = guards::toggle_execution_mark(change, &mut new_change_count);
        (change.id.clone(), change.selected, log_msg)
    };
    state.new_change_count = new_change_count;
    state.add_log(LogEntry::info(log_msg));
    // Target-scoped: the shared store belongs to every frontend, so one
    // interaction must never republish this frontend's whole cached row set.
    state.request_mark_write(&target_id, marked);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::operator_command::ParallelEligibility;
    use crate::tui::types::AppExecutionMode;
    use ratatui::style::Color;

    /// Every execution mode, so a lifecycle-independent rule is asserted against
    /// the whole axis rather than a sample of it.
    const ALL_MODES: [AppExecutionMode; 5] = [
        AppExecutionMode::Select,
        AppExecutionMode::Running,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ];

    /// Non-terminal statuses spanning idle, queued, active, error, and wait.
    const NON_TERMINAL_STATUSES: [&str; 10] = [
        "not queued",
        "queued",
        "preparing",
        "applying",
        "accepting",
        "archiving",
        "resolving",
        "error",
        "merge wait",
        "resolve pending",
    ];

    /// The four rows that are no longer run candidates.
    const TERMINAL_STATUSES: [&str; 4] = ["archived", "merged", "pushed", "rejected"];

    /// A row whose ineligibility, when any, is observed dirty proposal content.
    fn make_change_state(
        id: &str,
        display_status_cache: &str,
        is_parallel_eligible: bool,
    ) -> ChangeState {
        make_change_state_with_eligibility(
            id,
            display_status_cache,
            if is_parallel_eligible {
                ParallelEligibility::Eligible
            } else {
                ParallelEligibility::UncommittedProposalFiles
            },
        )
    }

    fn make_change_state_with_eligibility(
        id: &str,
        display_status_cache: &str,
        parallel_eligibility: ParallelEligibility,
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
            parallel_eligibility,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
            apply_operation_cache: "apply".to_string(),
            archive_complete_cache: false,
        }
    }

    fn state_with(mode: AppExecutionMode, changes: Vec<ChangeState>) -> AppState {
        let mut state = AppState::new(Vec::new());
        state.execution_mode = mode;
        state.changes = changes;
        state
    }

    /// Unit: Space marks every visible non-terminal row in every execution mode,
    /// with no lifecycle, activity, wait, or eligibility refusal.
    #[test]
    fn run_mark_intent_space_marks_every_non_terminal_row_in_every_mode() {
        for mode in ALL_MODES {
            for status in NON_TERMINAL_STATUSES {
                let mut state = state_with(mode, vec![make_change_state("a", status, true)]);

                state.toggle_selection();

                assert!(
                    state.changes[0].selected,
                    "{mode:?}/{status} must accept a mark"
                );
                assert_eq!(
                    state.take_pending_mark_writes(),
                    vec![("a".to_string(), true)],
                    "{mode:?}/{status} writes exactly its own mark"
                );
                assert!(
                    state.warning_message.is_none(),
                    "{mode:?}/{status} must not warn: {:?}",
                    state.warning_message
                );

                state.toggle_selection();
                assert!(
                    !state.changes[0].selected,
                    "{mode:?}/{status} must accept an unmark"
                );
                assert_eq!(
                    state.take_pending_mark_writes(),
                    vec![("a".to_string(), false)]
                );
            }
        }
    }

    /// Unit: a worktree-ineligible or settled terminal-error row still carries
    /// intent. Both conditions are decided at final run admission, not at mark
    /// time — and a settled Apply-limit failure adds no exclusion of its own.
    #[test]
    fn run_mark_intent_admits_ineligible_and_apply_limited_rows() {
        let limited = make_change_state("limited", "error", true);
        let rows = vec![
            make_change_state("uncommitted", "not queued", false),
            make_change_state_with_eligibility(
                "absent",
                "not queued",
                ParallelEligibility::ProposalAbsentFromHead,
            ),
            limited,
        ];

        for index in 0..rows.len() {
            let mut state = state_with(AppExecutionMode::Running, rows.clone());
            state.cursor_index = index;

            state.toggle_selection();

            assert!(
                state.changes[index].selected,
                "row {index} must accept mark intent"
            );
            assert!(
                state.warning_message.is_none(),
                "row {index} must not produce a mark-time admission warning: {:?}",
                state.warning_message
            );
        }
    }

    /// Unit: Space on a terminal row changes nothing and says nothing.
    #[test]
    fn run_mark_intent_space_on_a_terminal_row_is_a_silent_no_op() {
        for mode in ALL_MODES {
            for status in TERMINAL_STATUSES {
                for already_marked in [false, true] {
                    let mut row = make_change_state("t", status, true);
                    row.selected = already_marked;
                    let mut state = state_with(mode, vec![row]);

                    state.toggle_selection();

                    assert_eq!(
                        state.changes[0].selected, already_marked,
                        "{mode:?}/{status} must not move the mark"
                    );
                    assert!(
                        state.take_pending_mark_writes().is_empty(),
                        "{mode:?}/{status} must queue no mark write"
                    );
                    assert!(
                        state.warning_message.is_none(),
                        "{mode:?}/{status} must be silent: {:?}",
                        state.warning_message
                    );
                }
            }
        }
    }

    /// Unit: neither Space nor `x` ever emits a queue command, in any mode.
    #[test]
    fn run_mark_intent_never_emits_a_queue_command() {
        for mode in ALL_MODES {
            for status in NON_TERMINAL_STATUSES {
                let mut state = state_with(mode, vec![make_change_state("a", status, true)]);
                state.toggle_selection();
                assert!(
                    toggle_all_marks(&mut state).is_empty(),
                    "{mode:?}/{status} bulk toggle must emit no command"
                );
            }
        }
    }

    /// Unit: bulk `x` applies one target state to every non-terminal row in
    /// every mode and leaves terminal rows alone without a warning.
    #[test]
    fn run_mark_intent_bulk_toggle_covers_non_terminal_rows_in_every_mode() {
        for mode in ALL_MODES {
            let mut state = state_with(
                mode,
                vec![
                    make_change_state("idle", "not queued", true),
                    make_change_state("active", "applying", true),
                    make_change_state("uncommitted", "queued", false),
                    make_change_state("archived", "archived", true),
                ],
            );

            let commands = toggle_all_marks(&mut state);

            assert!(commands.is_empty(), "{mode:?} bulk toggle emits no command");
            assert!(state.changes[0].selected, "{mode:?} marks the idle row");
            assert!(state.changes[1].selected, "{mode:?} marks the active row");
            assert!(
                state.changes[2].selected,
                "{mode:?} marks the ineligible row"
            );
            assert!(
                !state.changes[3].selected,
                "{mode:?} leaves the terminal row unmarked"
            );
            assert!(
                state.warning_message.is_none(),
                "{mode:?} terminal exclusion must not warn: {:?}",
                state.warning_message
            );

            let written = state.take_pending_mark_writes();
            assert_eq!(written.len(), 3, "{mode:?} writes only the target rows");
            assert!(written
                .iter()
                .all(|(id, marked)| *marked && id != "archived"));

            // Fully marked target set now unmarks in one operation.
            assert!(toggle_all_marks(&mut state).is_empty());
            assert!(state.changes[..3].iter().all(|change| !change.selected));
            assert!(!state.changes[3].selected);
        }
    }

    /// Unit: a list holding only terminal rows is a silent no-op, while an empty
    /// list still reports why there was nothing to toggle.
    #[test]
    fn run_mark_intent_bulk_toggle_distinguishes_terminal_only_from_empty() {
        let mut terminal_only = state_with(
            AppExecutionMode::Select,
            TERMINAL_STATUSES
                .iter()
                .map(|status| make_change_state(status, status, true))
                .collect(),
        );
        assert!(toggle_all_marks(&mut terminal_only).is_empty());
        assert!(
            terminal_only.warning_message.is_none(),
            "terminal-only exclusion is silent: {:?}",
            terminal_only.warning_message
        );

        let mut empty = state_with(AppExecutionMode::Select, Vec::new());
        assert!(toggle_all_marks(&mut empty).is_empty());
        assert!(
            empty
                .warning_message
                .as_deref()
                .is_some_and(|message| message.contains("no changes to toggle")),
            "an empty list names the reason: {:?}",
            empty.warning_message
        );
    }

    /// Unit: an overlay owns input, so `x` never reaches the Changes view.
    #[test]
    fn run_mark_intent_bulk_toggle_defers_to_an_overlay() {
        let mut state = state_with(
            AppExecutionMode::Select,
            vec![make_change_state("a", "not queued", true)],
        );
        state.show_warning_popup("blocked", "diagnostic");

        assert!(toggle_all_marks(&mut state).is_empty());
        assert!(
            !state.changes[0].selected,
            "the underlying action never ran"
        );
    }

    /// Unit: the shared classifier is the only markability authority the TUI has.
    #[test]
    fn run_mark_intent_bulk_classification_only_excludes_terminal_rows() {
        for status in NON_TERMINAL_STATUSES {
            assert_eq!(
                classify_bulk_toggle_change(&make_change_state("a", status, false)),
                None,
                "{status} is a bulk target regardless of eligibility"
            );
        }
        for status in TERMINAL_STATUSES {
            assert_eq!(
                classify_bulk_toggle_change(&make_change_state("a", status, true)),
                Some(BulkToggleExclusion::FinalStatus),
                "{status} is excluded as terminal"
            );
        }
    }

    // ========================================================================
    // Reducer-recorded archive completion retires mark controls
    //
    // The paired control matters more than any individual case: `resolving` is a
    // post-archive status *and* a fresh-process resolve-retry status, so the only
    // thing that may separate them is the reducer's archive record.
    // ========================================================================

    /// Live post-archive display statuses a reducer-archived row can hold, plus
    /// the two terminal ones a completed post-archive lane reaches.
    const ARCHIVE_COMPLETE_STATUSES: [&str; 5] = [
        "resolving",
        "resolve pending",
        "merge wait",
        "merged",
        "pushed",
    ];

    /// A row the reducer has recorded as archive-complete.
    fn archived_change_state(id: &str, display_status_cache: &str) -> ChangeState {
        let mut change = make_change_state(id, display_status_cache, true);
        change.archive_complete_cache = true;
        change
    }

    /// Space on a reducer-archived row is a silent no-op in every mode, whatever
    /// post-archive status the row currently displays.
    #[test]
    fn tui_change_row_layout_mark_contract_space_on_an_archive_complete_row_is_a_silent_no_op() {
        for mode in ALL_MODES {
            for status in ARCHIVE_COMPLETE_STATUSES {
                for already_marked in [false, true] {
                    let mut row = archived_change_state("a", status);
                    row.selected = already_marked;
                    let mut state = state_with(mode, vec![row]);

                    state.toggle_selection();

                    assert_eq!(
                        state.changes[0].selected, already_marked,
                        "{mode:?}/{status}: an archive-complete row must not move its mark"
                    );
                    assert!(
                        state.take_pending_mark_writes().is_empty(),
                        "{mode:?}/{status}: no mark write may be queued"
                    );
                    assert!(
                        state.warning_message.is_none(),
                        "{mode:?}/{status}: refusal is silent: {:?}",
                        state.warning_message
                    );
                }
            }
        }
    }

    /// The control: the same `resolving` status with no reducer archive record —
    /// a fresh-process resolve retry — keeps its mark and its checkbox.
    #[test]
    fn tui_change_row_layout_mark_contract_resolving_without_an_archive_record_stays_markable() {
        for mode in ALL_MODES {
            let mut state = state_with(mode, vec![make_change_state("a", "resolving", true)]);
            assert!(
                !state.changes[0].archive_complete_cache,
                "the control row carries no archive evidence"
            );

            state.toggle_selection();

            assert!(
                state.changes[0].selected,
                "{mode:?}: a resolve retry with no archive on record is still a run candidate"
            );
            assert_eq!(
                state.take_pending_mark_writes(),
                vec![("a".to_string(), true)]
            );
        }
    }

    /// Bulk `x` excludes archive-complete rows with the shared stable reason and
    /// derives its target state from the remaining rows only.
    #[test]
    fn tui_change_row_layout_mark_contract_bulk_toggle_excludes_archive_complete_rows() {
        for mode in ALL_MODES {
            let mut rows = vec![
                make_change_state("idle", "not queued", true),
                // Same status as `archived-resolving` below, opposite evidence.
                make_change_state("fresh-resolving", "resolving", true),
            ];
            rows.extend(
                ARCHIVE_COMPLETE_STATUSES
                    .iter()
                    .map(|status| archived_change_state(&format!("archived-{status}"), status)),
            );
            let mut state = state_with(mode, rows);

            let commands = toggle_all_marks(&mut state);

            assert!(commands.is_empty(), "{mode:?} bulk toggle emits no command");
            assert!(state.changes[0].selected, "{mode:?} marks the idle row");
            assert!(
                state.changes[1].selected,
                "{mode:?} marks the archive-recordless resolving row"
            );
            for change in &state.changes[2..] {
                assert!(
                    !change.selected,
                    "{mode:?} must leave archive-complete row `{}` unmarked",
                    change.id
                );
            }
            assert!(
                state.warning_message.is_none(),
                "{mode:?} archive-complete exclusion must not warn: {:?}",
                state.warning_message
            );

            let written = state.take_pending_mark_writes();
            assert_eq!(written.len(), 2, "{mode:?} writes only the eligible rows");
            assert!(written
                .iter()
                .all(|(id, marked)| *marked && !id.starts_with("archived-")));
        }
    }

    /// The exclusion reason is the shared archive-complete token, distinct from
    /// the terminal-status one, and `x` is not offered for a list of only those.
    #[test]
    fn tui_change_row_layout_mark_contract_archive_complete_exclusion_has_its_own_reason() {
        // A live post-archive status reports archive completion; a terminal one
        // keeps reporting terminality, because that is what an operator sees.
        for status in ["resolving", "resolve pending", "merge wait"] {
            assert_eq!(
                classify_bulk_toggle_change(&archived_change_state("a", status)),
                Some(BulkToggleExclusion::ArchiveComplete),
                "`{status}` with an archive record is excluded as archive-complete"
            );
        }
        for status in ["merged", "pushed"] {
            assert_eq!(
                classify_bulk_toggle_change(&archived_change_state("a", status)),
                Some(BulkToggleExclusion::FinalStatus),
                "`{status}` is terminal, and terminality is the reason an operator reads"
            );
        }
        assert_eq!(
            classify_bulk_toggle_change(&make_change_state("a", "resolving", true)),
            None,
            "`resolving` with no archive record remains a bulk target"
        );
        assert_eq!(
            classify_bulk_toggle_change(&make_change_state("a", "archived", true)),
            Some(BulkToggleExclusion::FinalStatus),
            "a terminal status keeps reporting terminality, not archive completion"
        );

        let mut archive_complete_only = state_with(
            AppExecutionMode::Running,
            ARCHIVE_COMPLETE_STATUSES
                .iter()
                .map(|status| archived_change_state(status, status))
                .collect(),
        );
        assert!(
            !archive_complete_only.has_bulk_toggle_targets(),
            "a list of only archive-complete rows offers no bulk toggle"
        );
        assert!(toggle_all_marks(&mut archive_complete_only).is_empty());
        assert!(
            archive_complete_only.warning_message.is_none(),
            "and says nothing about it: {:?}",
            archive_complete_only.warning_message
        );
    }

    #[test]
    fn snapshot_marks_all_when_any_eligible_row_is_unmarked() {
        let mut changes = vec![
            make_change_state("a", "not queued", true),
            make_change_state("b", "not queued", true),
            make_change_state("c", "not queued", true),
        ];
        changes[0].selected = true;

        let snapshot = build_bulk_toggle_snapshot(&changes);

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
        // Terminal rows must not influence the target state.
        changes[1].selected = false;

        let snapshot = build_bulk_toggle_snapshot(&changes);

        assert_eq!(snapshot.eligible, vec![0]);
        assert_eq!(
            snapshot.excluded,
            vec![("b".to_string(), BulkToggleExclusion::FinalStatus)]
        );
        assert!(
            !snapshot.target_state,
            "a fully marked target set must unmark all eligible rows"
        );
    }

    #[test]
    fn snapshot_with_no_changes_has_empty_target_set() {
        let snapshot = build_bulk_toggle_snapshot(&[]);

        assert!(snapshot.eligible.is_empty());
        assert!(snapshot.excluded.is_empty());
        assert!(!snapshot.target_state);
    }
}
