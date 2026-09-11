//! Process-local dismissal of reviewed `merged` rows.
//!
//! A long-lived TUI keeps a `merged` row after its proposal has left the active
//! catalog, which is how completed work accumulates in the Changes view. This
//! module is the whole decision surface for hiding those rows again, and every
//! function in it moves presentation state only: rows, cursor, known-ID
//! bookkeeping, the NEW badge, the selected-proposal log filter, and the modal
//! axis. No repository file, archive, branch, worktree, reducer record, queue
//! entry, execution mark, or API projection is touched, and nothing here is
//! written outside this process.
//!
//! Two rules give the overlay its safety:
//!
//! * targets are **bound when the confirmation opens**, so a row that *becomes*
//!   `merged` while the operator is reading the overlay can never be swept up by
//!   a decision they never saw; and
//! * targets are **rechecked when it is confirmed**, so a bound row that stopped
//!   being `merged` is skipped rather than hidden.

use crate::tui::events::LogEntry;
use crate::tui::types::{ModalState, ViewMode};

use super::{AppState, ChangeState};

/// The one display status a row must currently carry to be dismissable.
///
/// Deliberately exact rather than a "terminal" family: `archived`, `pushed`,
/// `rejected`, and every waiting status stay visible, because this change is
/// about reviewed work the operator has already seen land on base.
pub(crate) const DISMISSABLE_STATUS: &str = "merged";

/// Whether a row is dismissable as it currently stands.
pub(crate) fn is_dismissable(change: &ChangeState) -> bool {
    change.display_status_cache == DISMISSABLE_STATUS
}

/// Whether the cursor row can be dismissed right now.
pub(crate) fn focused_row_is_dismissable(state: &AppState) -> bool {
    focused_dismissable_id(state).is_some()
}

/// Whether at least one row in the current Changes-list projection is dismissable.
///
/// "Projection", not "viewport": scrolling a merged row out of sight does not
/// make it a non-target, so the bulk hint and the bulk action agree on one
/// definition.
pub(crate) fn has_dismissable_rows(state: &AppState) -> bool {
    state.view_mode == ViewMode::Changes && state.changes.iter().any(is_dismissable)
}

/// The cursor row's ID, when the cursor is on a dismissable row in the Changes view.
fn focused_dismissable_id(state: &AppState) -> Option<String> {
    if state.view_mode != ViewMode::Changes {
        return None;
    }

    state
        .changes
        .get(state.cursor_index)
        .filter(|change| is_dismissable(change))
        .map(|change| change.id.clone())
}

/// Every dismissable ID in the current Changes-list projection, in list order.
fn projected_dismissable_ids(state: &AppState) -> Vec<String> {
    state
        .changes
        .iter()
        .filter(|change| is_dismissable(change))
        .map(|change| change.id.clone())
        .collect()
}

/// Whether an overlay already owns operator input.
///
/// Requesting a confirmation is an ordinary Changes-view interaction, so it
/// yields to anything that is already claiming keys instead of replacing it.
fn overlay_owns_input(state: &AppState) -> bool {
    state.has_overlay() || state.error_details_popup.is_some()
}

/// Open the individual dismissal confirmation for the cursor row.
///
/// Returns true when the confirmation was opened. A non-`merged` row is a silent
/// no-op: nothing opens, nothing is logged, and no hint advertised the key.
pub(crate) fn request_individual(state: &mut AppState) -> bool {
    if overlay_owns_input(state) {
        return false;
    }

    let Some(change_id) = focused_dismissable_id(state) else {
        return false;
    };

    state.modal = Some(ModalState::ConfirmDismissMergedRow {
        change_id: change_id.clone(),
    });
    state.add_log(
        LogEntry::info(format!(
            "Confirm dismissing merged row '{}': press Y to confirm, N/Esc to cancel",
            change_id
        ))
        .with_change_id(&change_id),
    );
    true
}

/// Open the bulk dismissal confirmation over every projected `merged` row.
///
/// Returns true when the confirmation was opened. An empty target set is a
/// silent no-op.
pub(crate) fn request_bulk(state: &mut AppState) -> bool {
    if overlay_owns_input(state) || state.view_mode != ViewMode::Changes {
        return false;
    }

    let change_ids = projected_dismissable_ids(state);
    if change_ids.is_empty() {
        return false;
    }

    let count = change_ids.len();
    state.modal = Some(ModalState::ConfirmDismissAllMergedRows { change_ids });
    state.add_log(LogEntry::info(format!(
        "Confirm dismissing {} merged row{}: press Y to confirm, N/Esc to cancel",
        count,
        if count == 1 { "" } else { "s" }
    )));
    true
}

/// Close a dismissal confirmation without changing anything it targeted.
///
/// Returns true when a dismissal confirmation was the overlay that closed.
pub(crate) fn cancel(state: &mut AppState) -> bool {
    if !matches!(
        state.modal,
        Some(
            ModalState::ConfirmDismissMergedRow { .. }
                | ModalState::ConfirmDismissAllMergedRows { .. }
        )
    ) {
        return false;
    }

    state.modal = None;
    state.add_log(LogEntry::info("Merged-row dismissal canceled".to_string()));
    true
}

/// Confirm the open dismissal confirmation.
///
/// The bound targets are rechecked against their *current* display status, so a
/// stale overlay can only ever dismiss fewer rows than it named, never a row
/// that changed underneath it. When nothing remains eligible the overlay closes
/// as a mutation-free no-op: no row moves, the dismissed set is untouched, and
/// no informational entry is logged.
///
/// Returns true when a dismissal confirmation was the overlay that closed.
pub(crate) fn confirm(state: &mut AppState) -> bool {
    let bound: Vec<String> = match state.modal.clone() {
        Some(ModalState::ConfirmDismissMergedRow { change_id }) => vec![change_id],
        Some(ModalState::ConfirmDismissAllMergedRows { change_ids }) => change_ids,
        _ => return false,
    };

    state.modal = None;

    let eligible: Vec<String> = bound
        .into_iter()
        .filter(|id| {
            state
                .changes
                .iter()
                .any(|change| &change.id == id && is_dismissable(change))
        })
        .collect();

    if eligible.is_empty() {
        return true;
    }

    dismiss(state, eligible);
    true
}

/// Hide the given rows for the rest of this process and repair local navigation.
///
/// The order matters: the log-filter target is captured before the rows move,
/// because it is derived from the cursor and would otherwise be read against a
/// list that no longer holds the row it was filtering on.
fn dismiss(state: &mut AppState, ids: Vec<String>) {
    let filter_target = if state.selected_proposal_log_filter {
        state
            .selected_proposal_log_filter_target()
            .map(str::to_string)
    } else {
        None
    };

    for id in &ids {
        state.dismissed_merged_ids.insert(id.clone());
    }

    state.changes.retain(|change| !ids.contains(&change.id));

    // An ID must not outlive its row, exactly as in catalog processing: a
    // known-ID entry left behind would classify the change as already seen, so a
    // later active observation of the same ID could never be painted as new.
    state.known_change_ids.retain(|id| !ids.contains(id));
    state.new_change_count = state.changes.iter().filter(|c| c.is_new).count();

    repair_cursor(state);

    // The filter is presentation only and its target is the cursor row, so a
    // dismissed target leaves it pointing at an unrelated proposal. Disable it
    // instead; `AppState::logs` is never modified, so nothing buffered is lost.
    if filter_target.is_some_and(|target| ids.contains(&target)) {
        state.selected_proposal_log_filter = false;
    }

    let summary = if ids.len() == 1 {
        format!("Dismissed merged row '{}' from this session", ids[0])
    } else {
        format!(
            "Dismissed {} merged rows from this session: {}",
            ids.len(),
            ids.join(", ")
        )
    };
    state.add_log(LogEntry::info(summary));
}

/// Keep the cursor on a real row after rows were removed beneath it.
///
/// The old index is preserved when a row now occupies it — removing the row
/// above the cursor should not scroll the operator somewhere else — and clamped
/// to the final row otherwise. An empty list has nothing to select.
fn repair_cursor(state: &mut AppState) {
    if state.changes.is_empty() {
        state.cursor_index = 0;
        state.list_state.select(None);
        return;
    }

    if state.cursor_index >= state.changes.len() {
        state.cursor_index = state.changes.len() - 1;
    }
    state.list_state.select(Some(state.cursor_index));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};

    fn change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 1,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    /// Rows named `(id, display status)`, in list order.
    fn app(rows: &[(&str, &str)]) -> AppState {
        let mut app = AppState::new(rows.iter().map(|(id, _)| change(id)).collect());
        for (index, (_, status)) in rows.iter().enumerate() {
            app.changes[index].set_display_status_cache(status);
        }
        app.logs.clear();
        app
    }

    fn row_ids(app: &AppState) -> Vec<String> {
        app.changes.iter().map(|c| c.id.clone()).collect()
    }

    #[test]
    fn individual_dismissal_removes_only_the_confirmed_row() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);

        assert!(request_individual(&mut app));
        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmDismissMergedRow {
                change_id: "alpha".to_string()
            })
        );

        assert!(confirm(&mut app));
        assert_eq!(app.modal, None);
        assert_eq!(row_ids(&app), vec!["beta", "gamma"]);
        assert!(app.dismissed_merged_ids().contains("alpha"));
        assert!(!app.dismissed_merged_ids().contains("gamma"));
        assert!(!app.known_change_ids.contains("alpha"));
    }

    #[test]
    fn individual_dismissal_is_inert_on_a_non_merged_row() {
        let mut app = app(&[("alpha", "applying"), ("beta", "merged")]);

        assert!(!request_individual(&mut app));
        assert_eq!(app.modal, None);
        assert!(app.logs.is_empty());
        assert_eq!(row_ids(&app), vec!["alpha", "beta"]);
    }

    #[test]
    fn bulk_dismissal_removes_every_bound_merged_row_and_nothing_else() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);

        assert!(request_bulk(&mut app));
        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmDismissAllMergedRows {
                change_ids: vec!["alpha".to_string(), "gamma".to_string()]
            })
        );

        assert!(confirm(&mut app));
        assert_eq!(row_ids(&app), vec!["beta"]);
        assert_eq!(app.changes[0].display_status_cache, "not queued");
    }

    #[test]
    fn bulk_dismissal_binds_targets_at_open_time() {
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        assert!(request_bulk(&mut app));

        // `beta` becomes merged only *after* the operator saw the overlay, so
        // the decision they took cannot grow to include it.
        app.changes[1].set_display_status_cache("merged");

        assert!(confirm(&mut app));
        assert_eq!(row_ids(&app), vec!["beta"]);
        assert!(!app.dismissed_merged_ids().contains("beta"));
    }

    #[test]
    fn bulk_dismissal_has_no_target_when_no_row_is_merged() {
        let mut app = app(&[("alpha", "applying"), ("beta", "not queued")]);

        assert!(!request_bulk(&mut app));
        assert_eq!(app.modal, None);
        assert!(app.logs.is_empty());
    }

    #[test]
    fn cancellation_changes_no_row_and_no_dismissed_state() {
        for open in [
            request_individual as fn(&mut AppState) -> bool,
            request_bulk as fn(&mut AppState) -> bool,
        ] {
            let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
            assert!(open(&mut app));

            assert!(cancel(&mut app));
            assert_eq!(app.modal, None);
            assert_eq!(row_ids(&app), vec!["alpha", "beta"]);
            assert!(app.dismissed_merged_ids().is_empty());
        }
    }

    #[test]
    fn a_bound_row_that_stopped_being_merged_is_not_hidden() {
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        assert!(request_individual(&mut app));

        app.changes[0].set_display_status_cache("resolving");
        let logs_before = app.logs.len();

        assert!(confirm(&mut app));
        assert_eq!(app.modal, None);
        assert_eq!(row_ids(&app), vec!["alpha", "beta"]);
        assert!(app.dismissed_merged_ids().is_empty());
        assert_eq!(
            app.logs.len(),
            logs_before,
            "a no-op confirmation must not log a dismissal that did not happen"
        );
    }

    #[test]
    fn bulk_confirmation_dismisses_only_the_rows_still_merged() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "merged"),
            ("gamma", "not queued"),
        ]);
        assert!(request_bulk(&mut app));

        app.changes[1].set_display_status_cache("archiving");

        assert!(confirm(&mut app));
        assert_eq!(row_ids(&app), vec!["beta", "gamma"]);
        assert!(app.dismissed_merged_ids().contains("alpha"));
        assert!(!app.dismissed_merged_ids().contains("beta"));
    }

    #[test]
    fn cursor_survives_removing_the_first_middle_last_and_only_row() {
        // Removing a row above the cursor keeps the operator on the same index,
        // which is now a different (surviving) row.
        let mut first = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "not queued"),
        ]);
        first.cursor_index = 1;
        first.list_state.select(Some(1));
        first.cursor_index = 0;
        assert!(request_individual(&mut first));
        assert!(confirm(&mut first));
        assert_eq!(first.cursor_index, 0);
        assert_eq!(first.list_state.selected(), Some(0));
        assert_eq!(row_ids(&first), vec!["beta", "gamma"]);

        // Middle row.
        let mut middle = app(&[
            ("alpha", "not queued"),
            ("beta", "merged"),
            ("gamma", "not queued"),
        ]);
        middle.cursor_index = 1;
        assert!(request_individual(&mut middle));
        assert!(confirm(&mut middle));
        assert_eq!(middle.cursor_index, 1);
        assert_eq!(row_ids(&middle), vec!["alpha", "gamma"]);

        // Last row: the index no longer exists, so it clamps to the final row.
        let mut last = app(&[("alpha", "not queued"), ("beta", "merged")]);
        last.cursor_index = 1;
        assert!(request_individual(&mut last));
        assert!(confirm(&mut last));
        assert_eq!(last.cursor_index, 0);
        assert_eq!(last.list_state.selected(), Some(0));

        // Only row: nothing survives, so nothing is selected.
        let mut only = app(&[("alpha", "merged")]);
        assert!(request_individual(&mut only));
        assert!(confirm(&mut only));
        assert!(only.changes.is_empty());
        assert_eq!(only.cursor_index, 0);
        assert_eq!(only.list_state.selected(), None);
    }

    #[test]
    fn dismissing_every_row_leaves_an_empty_but_valid_projection() {
        let mut app = app(&[("alpha", "merged"), ("beta", "merged")]);
        assert!(request_bulk(&mut app));
        assert!(confirm(&mut app));

        assert!(app.changes.is_empty());
        assert_eq!(app.list_state.selected(), None);
        assert_eq!(app.new_change_count, 0);
        assert!(!has_dismissable_rows(&app));
        assert!(!focused_row_is_dismissable(&app));
    }

    #[test]
    fn dismissing_the_log_filter_target_disables_the_filter_without_dropping_logs() {
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        app.add_log(LogEntry::info("alpha ran").with_change_id("alpha"));
        let buffered = app.logs.len();
        app.toggle_selected_proposal_log_filter();
        assert!(app.selected_proposal_log_filter);
        assert_eq!(app.selected_proposal_log_filter_target(), Some("alpha"));

        assert!(request_individual(&mut app));
        assert!(confirm(&mut app));

        assert!(!app.selected_proposal_log_filter);
        assert!(
            app.logs.len() >= buffered,
            "disabling the filter must not delete buffered entries"
        );
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("alpha ran")));
    }

    #[test]
    fn dismissing_a_row_the_filter_does_not_target_keeps_the_filter_on() {
        let mut app = app(&[("alpha", "not queued"), ("beta", "merged")]);
        app.toggle_selected_proposal_log_filter();
        assert!(app.selected_proposal_log_filter);

        app.cursor_index = 1;
        assert!(request_individual(&mut app));
        app.cursor_index = 0;
        assert!(confirm(&mut app));

        assert!(app.selected_proposal_log_filter);
        assert_eq!(app.selected_proposal_log_filter_target(), Some("alpha"));
    }

    #[test]
    fn an_overlay_that_already_owns_input_refuses_a_new_dismissal_request() {
        let mut app = app(&[("alpha", "merged")]);
        app.modal = Some(ModalState::QrPopup);

        assert!(!request_individual(&mut app));
        assert!(!request_bulk(&mut app));
        assert_eq!(app.modal, Some(ModalState::QrPopup));
    }

    #[test]
    fn dismissal_requests_are_changes_view_only() {
        let mut app = app(&[("alpha", "merged")]);
        app.view_mode = ViewMode::Worktrees;

        assert!(!request_individual(&mut app));
        assert!(!request_bulk(&mut app));
        assert!(!has_dismissable_rows(&app));
        assert_eq!(app.modal, None);
    }

    #[test]
    fn hints_track_the_focused_row_and_the_projection_independently() {
        let mut app = app(&[("alpha", "not queued"), ("beta", "merged")]);

        // Cursor on a non-merged row: no individual target, but the projection
        // still holds one, off-cursor.
        assert!(!focused_row_is_dismissable(&app));
        assert!(has_dismissable_rows(&app));

        app.cursor_index = 1;
        assert!(focused_row_is_dismissable(&app));
    }

    #[test]
    fn confirm_and_cancel_ignore_every_other_overlay() {
        let mut app = app(&[("alpha", "merged")]);
        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "alpha".to_string(),
        });

        assert!(!confirm(&mut app));
        assert!(!cancel(&mut app));
        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmForceKill {
                change_id: "alpha".to_string()
            })
        );
    }
}
