//! Process-local dismissal of reviewed `merged` rows.
//!
//! A long-lived TUI keeps a `merged` row after its proposal has left the active
//! catalog, which is how completed work accumulates in the Changes view. This
//! module is the whole decision surface for hiding those rows again, and every
//! function in it moves presentation state only: rows, cursor, known-ID
//! bookkeeping, the NEW badge, and the selected-proposal log filter. No
//! repository file, archive, branch, worktree, reducer record, queue entry,
//! execution mark, or API projection is touched, and nothing here is written
//! outside this process.
//!
//! Dismissal is immediate: `d` and `D` act in the same input-handling turn that
//! delivered them, with no confirmation overlay and no second key press. That is
//! safe because the action hides local rows and nothing else — restarting the
//! TUI restores every dismissed row — and because targets are derived from
//! *current* display status in that same turn, so there is no window in which a
//! bound target can go stale.

use crate::tui::events::LogEntry;
use crate::tui::types::ViewMode;

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
/// Dismissal is an ordinary Changes-view interaction, so it yields to anything
/// that is already claiming keys. Normal key routing already sends keys to the
/// active overlay before the view sees them; this is defense in depth for any
/// caller that reaches the action directly.
fn overlay_owns_input(state: &AppState) -> bool {
    state.has_overlay() || state.error_details_popup.is_some()
}

/// Dismiss the cursor row immediately, when it is currently `merged`.
///
/// Returns true when a row was dismissed. A non-`merged` row is a silent no-op:
/// nothing is hidden, nothing is logged, and no hint advertised the key.
pub(crate) fn dismiss_focused(state: &mut AppState) -> bool {
    if overlay_owns_input(state) {
        return false;
    }

    let Some(change_id) = focused_dismissable_id(state) else {
        return false;
    };

    dismiss(state, vec![change_id]);
    true
}

/// Dismiss every projected `merged` row immediately.
///
/// Returns true when at least one row was dismissed. An empty target set is a
/// silent no-op.
pub(crate) fn dismiss_all_projected(state: &mut AppState) -> bool {
    if overlay_owns_input(state) || state.view_mode != ViewMode::Changes {
        return false;
    }

    let change_ids = projected_dismissable_ids(state);
    if change_ids.is_empty() {
        return false;
    }

    dismiss(state, change_ids);
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
    use crate::tui::types::ModalState;

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
    fn individual_dismissal_removes_only_the_focused_row_and_opens_no_modal() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);

        assert!(dismiss_focused(&mut app));

        assert_eq!(app.modal, None, "dismissal must not open an overlay");
        assert_eq!(row_ids(&app), vec!["beta", "gamma"]);
        assert!(app.dismissed_merged_ids().contains("alpha"));
        assert!(!app.dismissed_merged_ids().contains("gamma"));
        assert!(!app.known_change_ids.contains("alpha"));
    }

    #[test]
    fn individual_dismissal_is_inert_on_a_non_merged_row() {
        let mut app = app(&[("alpha", "applying"), ("beta", "merged")]);

        assert!(!dismiss_focused(&mut app));
        assert_eq!(app.modal, None);
        assert!(app.logs.is_empty());
        assert!(app.dismissed_merged_ids().is_empty());
        assert_eq!(row_ids(&app), vec!["alpha", "beta"]);
    }

    #[test]
    fn bulk_dismissal_removes_every_projected_merged_row_and_nothing_else() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);

        assert!(dismiss_all_projected(&mut app));

        assert_eq!(app.modal, None);
        assert_eq!(row_ids(&app), vec!["beta"]);
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(app.dismissed_merged_ids().contains("alpha"));
        assert!(app.dismissed_merged_ids().contains("gamma"));
    }

    #[test]
    fn bulk_dismissal_targets_the_whole_projection_regardless_of_the_cursor() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);
        // The cursor sits on the one row that must survive; the bulk target set
        // is derived from the projection, never from the focused row.
        app.cursor_index = 1;

        assert!(dismiss_all_projected(&mut app));

        assert_eq!(row_ids(&app), vec!["beta"]);
    }

    #[test]
    fn bulk_dismissal_has_no_target_when_no_row_is_merged() {
        let mut app = app(&[("alpha", "applying"), ("beta", "not queued")]);

        assert!(!dismiss_all_projected(&mut app));
        assert_eq!(app.modal, None);
        assert!(app.logs.is_empty());
        assert!(app.dismissed_merged_ids().is_empty());
    }

    #[test]
    fn a_row_that_stopped_being_merged_is_not_hidden() {
        // Status is read in the same turn the key is handled, so there is no
        // bound-target window at all: a row that left `merged` simply is not a
        // target when the action runs.
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        app.changes[0].set_display_status_cache("resolving");

        assert!(!dismiss_focused(&mut app));
        assert!(!dismiss_all_projected(&mut app));
        assert_eq!(row_ids(&app), vec!["alpha", "beta"]);
        assert!(app.dismissed_merged_ids().is_empty());
        assert!(
            app.logs.is_empty(),
            "a no-op dismissal must not log a dismissal that did not happen"
        );
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
        first.cursor_index = 0;
        assert!(dismiss_focused(&mut first));
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
        assert!(dismiss_focused(&mut middle));
        assert_eq!(middle.cursor_index, 1);
        assert_eq!(row_ids(&middle), vec!["alpha", "gamma"]);

        // Last row: the index no longer exists, so it clamps to the final row.
        let mut last = app(&[("alpha", "not queued"), ("beta", "merged")]);
        last.cursor_index = 1;
        assert!(dismiss_focused(&mut last));
        assert_eq!(last.cursor_index, 0);
        assert_eq!(last.list_state.selected(), Some(0));

        // Only row: nothing survives, so nothing is selected.
        let mut only = app(&[("alpha", "merged")]);
        assert!(dismiss_focused(&mut only));
        assert!(only.changes.is_empty());
        assert_eq!(only.cursor_index, 0);
        assert_eq!(only.list_state.selected(), None);
    }

    #[test]
    fn dismissing_every_row_leaves_an_empty_but_valid_projection() {
        let mut app = app(&[("alpha", "merged"), ("beta", "merged")]);
        assert!(dismiss_all_projected(&mut app));

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

        assert!(dismiss_focused(&mut app));

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

        // Bulk dismissal sweeps `beta` while the filter still targets the
        // cursor's own surviving row.
        assert!(dismiss_all_projected(&mut app));

        assert!(app.selected_proposal_log_filter);
        assert_eq!(app.selected_proposal_log_filter_target(), Some("alpha"));
    }

    #[test]
    fn an_overlay_that_already_owns_input_refuses_a_dismissal() {
        let mut app = app(&[("alpha", "merged")]);
        app.modal = Some(ModalState::QrPopup);

        assert!(!dismiss_focused(&mut app));
        assert!(!dismiss_all_projected(&mut app));
        assert_eq!(app.modal, Some(ModalState::QrPopup));
        assert_eq!(row_ids(&app), vec!["alpha"]);
        assert!(app.dismissed_merged_ids().is_empty());
    }

    #[test]
    fn dismissal_is_changes_view_only() {
        let mut app = app(&[("alpha", "merged")]);
        app.view_mode = ViewMode::Worktrees;

        assert!(!dismiss_focused(&mut app));
        assert!(!dismiss_all_projected(&mut app));
        assert!(!has_dismissable_rows(&app));
        assert_eq!(app.modal, None);
        assert_eq!(row_ids(&app), vec!["alpha"]);
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
}
