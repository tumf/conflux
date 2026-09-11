use std::{collections::HashSet, time::Instant};

use crate::{
    openspec::Change, orchestration::operator_command::is_final_status, task_parser,
    tui::events::LogEntry,
};

use super::{AppState, ChangeState};

/// Drop catalog observations for rows the operator dismissed, and revive an ID
/// this refresh proves is active again.
///
/// Catalog processing is the only place rows are *generated*, so it is also the
/// only place dismissal has to be enforced: reducer synchronization mutates
/// existing rows and can never resurrect one. Two directions, both required by
/// the dismissal contract:
///
/// * a dismissed ID observed as an **active non-terminal** change is no longer
///   the retained terminal row the operator dismissed, so dismissal is cleared
///   and the row is rendered normally; and
/// * every other dismissed observation is removed from this pass entirely —
///   before new-row creation, NEW counting, and the detection log — so a
///   suppressed change is never announced as newly detected.
///
/// Terminality is judged from the reducer's own latest display-status snapshot
/// rather than from the row (the dismissed row is gone) or from the catalog
/// (which reports directory presence, not lifecycle).
fn suppress_dismissed_rows(
    state: &mut AppState,
    fetched_changes: Vec<Change>,
    rejected_changes: Vec<Change>,
) -> (Vec<Change>, Vec<Change>) {
    if state.dismissed_merged_ids.is_empty() {
        return (fetched_changes, rejected_changes);
    }

    let reactivated: Vec<String> = fetched_changes
        .iter()
        .filter(|change| state.dismissed_merged_ids.contains(&change.id))
        .filter(|change| {
            !state
                .reducer_display_status_snapshot
                .get(&change.id)
                .copied()
                .is_some_and(is_final_status)
        })
        .map(|change| change.id.clone())
        .collect();

    for id in &reactivated {
        state.dismissed_merged_ids.remove(id);
        state.add_log(
            LogEntry::info(format!(
                "Change '{}' is active again; restoring its dismissed row",
                id
            ))
            .with_change_id(id),
        );
    }

    let keep = |change: &Change| !state.dismissed_merged_ids.contains(&change.id);
    (
        fetched_changes.into_iter().filter(keep).collect(),
        rejected_changes.into_iter().filter(keep).collect(),
    )
}

pub(super) fn update_changes_with_rejected(
    state: &mut AppState,
    fetched_changes: Vec<Change>,
    rejected_changes: Vec<Change>,
) {
    // Progress is written to shared orchestration state from the *unfiltered*
    // observation on purpose. Dismissal is a local presentation decision, and a
    // row an operator stopped looking at must not change what the reducer, the
    // API, or any other frontend is told about the workspace.
    if let Some(shared_state) = &state.shared_orchestrator_state {
        if let Ok(mut guard) = shared_state.try_write() {
            for change in &fetched_changes {
                if change.total_tasks > 0 {
                    guard.set_task_progress(
                        change.id.clone(),
                        change.completed_tasks,
                        change.total_tasks,
                    );
                }
            }
            for change in &rejected_changes {
                if change.total_tasks > 0 {
                    guard.set_task_progress(
                        change.id.clone(),
                        change.completed_tasks,
                        change.total_tasks,
                    );
                }
            }
        }
    }

    // Everything below is presentation: rows, identity bookkeeping, the NEW
    // badge, and the detection log. Dismissed observations are removed here so
    // none of them can see a row the operator hid.
    let (fetched_changes, rejected_changes) =
        suppress_dismissed_rows(state, fetched_changes, rejected_changes);
    let active_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
    let rejected_ids: HashSet<String> = rejected_changes.iter().map(|c| c.id.clone()).collect();

    let new_ids: Vec<String> = fetched_changes
        .iter()
        .chain(rejected_changes.iter())
        .filter(|c| !state.known_change_ids.contains(&c.id))
        .map(|c| c.id.clone())
        .collect();
    let new_active_ids: Vec<String> = fetched_changes
        .iter()
        .filter(|c| !state.known_change_ids.contains(&c.id))
        .map(|c| c.id.clone())
        .collect();

    for fetched in &fetched_changes {
        if let Some(existing) = state.changes.iter_mut().find(|c| c.id == fetched.id) {
            let was_rejected = existing.display_status_cache == "rejected";
            let was_archived = existing.display_status_cache == "archived";
            let is_merge_wait = existing.display_status_cache == "merge wait";
            let is_resolve_wait = existing.display_status_cache == "resolve pending";

            if was_rejected && !rejected_ids.contains(&fetched.id) {
                existing.set_display_status_cache("not queued");
            }

            let (completed, total) = if let Some(shared_state) = &state.shared_orchestrator_state {
                if let Ok(guard) = shared_state.try_read() {
                    let progress = guard.task_progress(&fetched.id);
                    if progress.1 > 0 {
                        progress
                    } else {
                        (fetched.completed_tasks, fetched.total_tasks)
                    }
                } else {
                    (fetched.completed_tasks, fetched.total_tasks)
                }
            } else {
                (fetched.completed_tasks, fetched.total_tasks)
            };

            if was_archived {
                existing.set_display_status_cache("not queued");
                if total > 0 {
                    existing.completed_tasks = completed;
                    existing.total_tasks = total;
                }
            } else if is_merge_wait || is_resolve_wait {
                if total > 0 {
                    existing.completed_tasks = completed;
                    existing.total_tasks = total;
                }
            } else if total > 0 {
                existing.completed_tasks = completed;
                existing.total_tasks = total;
            } else {
                let worktree_path = state.worktree_paths.get(&fetched.id).map(|p| p.as_path());
                match existing.display_status_cache.as_str() {
                    "archiving" | "resolving" | "archived" | "merged" => {
                        if let Ok(progress) =
                            task_parser::parse_progress_with_fallback(&fetched.id, worktree_path)
                        {
                            if progress.total > 0 {
                                existing.completed_tasks = progress.completed;
                                existing.total_tasks = progress.total;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for rejected in &rejected_changes {
        if let Some(existing) = state.changes.iter_mut().find(|c| c.id == rejected.id) {
            let (completed, total) = if let Some(shared_state) = &state.shared_orchestrator_state {
                if let Ok(guard) = shared_state.try_read() {
                    let progress = guard.task_progress(&rejected.id);
                    if progress.1 > 0 {
                        progress
                    } else {
                        (rejected.completed_tasks, rejected.total_tasks)
                    }
                } else {
                    (rejected.completed_tasks, rejected.total_tasks)
                }
            } else {
                (rejected.completed_tasks, rejected.total_tasks)
            };

            if total > 0 {
                existing.completed_tasks = completed;
                existing.total_tasks = total;
            }
            existing.set_display_status_cache("rejected");
        }
    }

    for id in &new_ids {
        if let Some(fetched) = fetched_changes.iter().find(|c| &c.id == id) {
            let mut new_state = ChangeState::from_change(fetched);
            new_state.is_new = true;
            state.changes.push(new_state);
            continue;
        }

        if let Some(rejected) = rejected_changes.iter().find(|c| &c.id == id) {
            let mut rejected_state = ChangeState::from_change(rejected);
            rejected_state.is_new = false;
            rejected_state.set_display_status_cache("rejected");
            state.changes.push(rejected_state);
        }
    }

    for id in &new_active_ids {
        state.add_log(LogEntry::info(format!("Detected new change: {}", id)));
    }

    state.known_change_ids.extend(new_ids);
    state.last_refresh = Instant::now();

    if let Some(shared_state) = &state.shared_orchestrator_state {
        if let Ok(guard) = shared_state.try_read() {
            for change in &mut state.changes {
                let apply_count = guard.apply_count(&change.id);
                if apply_count > 0 {
                    match change.iteration_number {
                        Some(existing) => {
                            if apply_count > existing {
                                change.iteration_number = Some(apply_count);
                            }
                        }
                        None => change.iteration_number = Some(apply_count),
                    }
                }
            }
        }
    }

    state.changes.retain(|c| {
        active_ids.contains(&c.id)
            || rejected_ids.contains(&c.id)
            // Either an open active interval or a retained accumulated
            // duration proves this row executed, so an inactive row whose
            // interval was already closed survives a temporary catalog absence
            // exactly as a still-running one does — no separate durable flag.
            || c.has_execution_history()
            || matches!(
                c.display_status_cache.as_str(),
                "archiving"
                    | "archived"
                    | "merged"
                    | "merge wait"
                    | "resolving"
                    | "resolve pending"
                    | "rejected"
                    | "error"
            )
    });

    // Identity bookkeeping converges to the row projection that just survived.
    //
    // An ID must not outlive its row. A change can be absent from a single
    // filesystem snapshot while its worktree is refreshed or merged; the retain
    // above drops the row, and a known-ID entry left behind would classify the
    // change as already seen when it is observed again, so the row could never
    // be reconstructed. Conversely an ID whose row was deliberately retained
    // through the absence — recorded execution history, or a terminal/wait
    // display status — stays known, so re-observing it updates that row in
    // place instead of pushing a duplicate NEW row.
    //
    // This is observability bookkeeping only: it decides whether a row is
    // painted as newly detected and logged, never queue membership, dispatch,
    // resume routing, acceptance, or archive routing.
    let retained_row_ids: HashSet<&str> = state.changes.iter().map(|c| c.id.as_str()).collect();
    state
        .known_change_ids
        .retain(|id| retained_row_ids.contains(id.as_str()));

    // Counted from the settled projection rather than from the pre-retain row
    // list, so the NEW badge can never advertise a row that this same pass
    // dropped.
    state.new_change_count = state.changes.iter().filter(|c| c.is_new).count();

    // Rows this pass created or re-created start with no archive-completion cache,
    // so the retained reducer snapshot is reapplied before anything renders. Doing
    // it here rather than waiting for the next reducer sync is what keeps a
    // post-archive row from flashing an empty checkbox for one frame.
    state.reapply_reducer_archive_completion();

    if state.cursor_index >= state.changes.len() && !state.changes.is_empty() {
        state.cursor_index = state.changes.len() - 1;
        state.list_state.select(Some(state.cursor_index));
    }
}

#[cfg(test)]
mod dismissal_tests {
    use std::collections::HashMap;

    use super::*;
    use crate::openspec::ProposalMetadata;

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

    /// An app whose rows carry exactly the reducer statuses given.
    ///
    /// The statuses go through the reducer sync rather than being written onto
    /// the rows, so the retained snapshot the suppression rule reads is the same
    /// one production fills.
    fn app(rows: &[(&str, &'static str)]) -> AppState {
        let mut app = AppState::new(rows.iter().map(|(id, _)| change(id)).collect());
        let display: HashMap<String, &'static str> = rows
            .iter()
            .map(|(id, status)| (id.to_string(), *status))
            .collect();
        app.apply_display_statuses_from_reducer(&display);
        app.logs.clear();
        app
    }

    fn row_ids(app: &AppState) -> Vec<String> {
        app.changes.iter().map(|c| c.id.clone()).collect()
    }

    /// Dismiss the focused row through the confirmation the operator uses.
    fn dismiss_focused(app: &mut AppState) {
        assert!(app.request_dismiss_merged_row());
        assert!(app.confirm_dismiss_merged_rows());
    }

    #[test]
    fn a_dismissed_row_is_not_recreated_by_a_retained_terminal_observation() {
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        dismiss_focused(&mut app);
        app.logs.clear();

        // The catalog still lists `alpha` while the reducer keeps reporting the
        // terminal outcome the operator dismissed.
        for _ in 0..3 {
            update_changes_with_rejected(&mut app, vec![change("alpha"), change("beta")], vec![]);
        }

        assert_eq!(row_ids(&app), vec!["beta"]);
        assert!(app.dismissed_merged_ids().contains("alpha"));
        assert!(!app.known_change_ids.contains("alpha"));
        assert_eq!(app.new_change_count, 0);
        assert!(
            !app.logs
                .iter()
                .any(|entry| entry.message.contains("Detected new change: alpha")),
            "a suppressed row must not be announced as newly detected"
        );
    }

    #[test]
    fn a_rejected_observation_of_a_dismissed_id_stays_suppressed() {
        let mut app = app(&[("alpha", "merged")]);
        dismiss_focused(&mut app);

        update_changes_with_rejected(&mut app, vec![], vec![change("alpha")]);

        assert!(app.changes.is_empty());
        assert!(app.dismissed_merged_ids().contains("alpha"));
    }

    #[test]
    fn an_active_non_terminal_observation_revives_a_dismissed_id() {
        let mut app = app(&[("alpha", "merged"), ("beta", "not queued")]);
        dismiss_focused(&mut app);

        // The reducer no longer reports a terminal outcome for `alpha`, and the
        // catalog observes it again: this is a different change wearing the same
        // ID, not the row that was dismissed.
        app.apply_display_statuses_from_reducer(&HashMap::new());
        update_changes_with_rejected(&mut app, vec![change("alpha"), change("beta")], vec![]);

        assert!(row_ids(&app).contains(&"alpha".to_string()));
        assert!(app.dismissed_merged_ids().is_empty());
        assert_eq!(app.new_change_count, 1);

        // And it keeps rendering on every later refresh.
        update_changes_with_rejected(&mut app, vec![change("alpha"), change("beta")], vec![]);
        assert!(row_ids(&app).contains(&"alpha".to_string()));
    }

    #[test]
    fn suppression_leaves_every_other_row_untouched() {
        let mut app = app(&[
            ("alpha", "merged"),
            ("beta", "not queued"),
            ("gamma", "merged"),
        ]);
        dismiss_focused(&mut app);

        update_changes_with_rejected(
            &mut app,
            vec![change("alpha"), change("beta"), change("gamma")],
            vec![],
        );

        assert_eq!(row_ids(&app), vec!["beta", "gamma"]);
        assert_eq!(app.changes[1].display_status_cache, "merged");
    }

    #[test]
    fn dismissal_does_not_change_what_the_reducer_is_told_about_the_workspace() {
        // Presentation is local; shared orchestration state is not. A dismissed
        // change's observed task progress must still reach the shared state that
        // `/api/v2` and every other frontend read.
        let mut app = app(&[("alpha", "merged")]);
        dismiss_focused(&mut app);

        let shared = std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::new(Vec::new(), 1),
        ));
        app.shared_orchestrator_state = Some(shared.clone());

        let mut observed = change("alpha");
        observed.completed_tasks = 2;
        observed.total_tasks = 5;
        update_changes_with_rejected(&mut app, vec![observed], vec![]);

        let guard = shared.try_read().expect("uncontended shared state");
        assert_eq!(guard.task_progress("alpha"), (2, 5));
        assert!(app.changes.is_empty());
    }
}
