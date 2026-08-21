use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::openspec::Change;
use crate::tui::events::{LogEntry, TuiRefreshObservation};
use crate::tui::types::{WorkspaceDirtyState, WorktreeInfo};

use super::AppState;

fn is_refresh_merge_wait_terminal_status(status: &str) -> bool {
    matches!(status, "archived" | "merged" | "rejected")
}

/// Whether the reducer already owns a status that refresh-derived merge-wait
/// evidence must not overwrite.
///
/// Active execution is delegated to the shared classifier rather than restated
/// here. A second hand-written active list is what let `archiving` fall through
/// and repaint a healthy Archive as `merge wait`: `merge_wait_ids` proves the
/// archive commit exists without base integration, which says nothing about
/// whether Archive has returned.
fn is_reducer_owned_refresh_merge_wait_protected_status(status: &str) -> bool {
    if crate::orchestration::operator_command::is_active_status(status) {
        return true;
    }

    matches!(
        status,
        "resolve pending"
            | "reject pending"
            | "merged"
            | "rejected"
            | "error"
            // The reducer consumed this same refresh before the TUI did, so
            // `not queued` is its answer to the very observation below — a row
            // released by a process-level stop or an explicit dequeue, whose
            // guard holds until an explicit requeue. A restarted process has no
            // such guard and re-derives `merge wait` here instead.
            | "not queued"
    )
}

impl AppState {
    pub(crate) fn handle_dependency_blocked(&mut self, change_id: String) {
        let was_already_blocked = self
            .changes
            .iter_mut()
            .find(|c| c.id == change_id)
            .map(|change| {
                let was_blocked = change.display_status_cache == "blocked";
                change.set_display_status_cache("blocked");
                was_blocked
            })
            .unwrap_or(false);

        if was_already_blocked {
            tracing::debug!(
                change_id = %change_id,
                "Suppressing repeated dependency-blocked TUI log"
            );
            return;
        }

        self.add_log(
            LogEntry::info(format!("Change '{}' blocked by dependencies", change_id))
                .with_change_id(&change_id),
        );
    }

    pub(crate) fn handle_dependency_resolved(&mut self, change_id: String) {
        let was_blocked = self
            .changes
            .iter_mut()
            .find(|c| c.id == change_id)
            .map(|change| {
                let was_blocked = change.display_status_cache == "blocked";
                if was_blocked {
                    change.set_display_status_cache("queued");
                }
                was_blocked
            })
            .unwrap_or(false);

        if !was_blocked {
            tracing::debug!(
                change_id = %change_id,
                "Suppressing repeated dependency-resolved TUI log"
            );
            return;
        }

        self.reset_analysis_log_dedupe();
        self.add_log(
            LogEntry::info(format!("Change '{}' dependencies resolved", change_id))
                .with_change_id(&change_id),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_changes_refreshed(
        &mut self,
        changes: Vec<Change>,
        rejected_changes: Vec<Change>,
        committed_change_ids: HashSet<String>,
        uncommitted_file_change_ids: HashSet<String>,
        worktree_change_ids: HashSet<String>,
        worktree_paths: HashMap<String, PathBuf>,
        _worktree_not_ahead_ids: HashSet<String>,
        merge_wait_ids: HashSet<String>,
    ) {
        let terminal_merge_wait_statuses = self.terminal_merge_wait_statuses(&merge_wait_ids);
        let reducer_protected_merge_wait_ids =
            self.reducer_protected_merge_wait_ids(&merge_wait_ids);

        self.worktree_paths = worktree_paths;
        self.update_changes_with_rejected(changes, rejected_changes);
        self.apply_parallel_eligibility(&committed_change_ids, &uncommitted_file_change_ids);
        self.apply_worktree_status(&worktree_change_ids);
        self.apply_refresh_merge_wait_status(
            &merge_wait_ids,
            &terminal_merge_wait_statuses,
            &reducer_protected_merge_wait_ids,
        );
    }

    fn terminal_merge_wait_statuses(
        &self,
        merge_wait_ids: &HashSet<String>,
    ) -> HashMap<String, String> {
        self.changes
            .iter()
            .filter(|change| {
                merge_wait_ids.contains(&change.id)
                    && is_refresh_merge_wait_terminal_status(&change.display_status_cache)
            })
            .map(|change| (change.id.clone(), change.display_status_cache.clone()))
            .collect()
    }

    fn reducer_protected_merge_wait_ids(
        &self,
        merge_wait_ids: &HashSet<String>,
    ) -> HashSet<String> {
        merge_wait_ids
            .iter()
            .filter(|change_id| {
                self.reducer_display_status_snapshot
                    .get(change_id.as_str())
                    .is_some_and(|status| {
                        is_reducer_owned_refresh_merge_wait_protected_status(status)
                    })
            })
            .cloned()
            .collect()
    }

    fn apply_refresh_merge_wait_status(
        &mut self,
        merge_wait_ids: &HashSet<String>,
        terminal_merge_wait_statuses: &HashMap<String, String>,
        reducer_protected_merge_wait_ids: &HashSet<String>,
    ) {
        if merge_wait_ids.is_empty() {
            return;
        }

        for change in &mut self.changes {
            if !merge_wait_ids.contains(&change.id) {
                continue;
            }

            if let Some(terminal_status) = terminal_merge_wait_statuses.get(&change.id) {
                change.set_display_status_cache(terminal_status);
                continue;
            }

            if is_refresh_merge_wait_terminal_status(&change.display_status_cache) {
                continue;
            }

            if reducer_protected_merge_wait_ids.contains(&change.id) {
                tracing::debug!(
                    change_id = %change.id,
                    reducer_status = ?self.reducer_display_status_snapshot.get(change.id.as_str()),
                    "Preserving reducer-owned active, pending, terminal, or error display over refresh merge-wait evidence"
                );
                continue;
            }

            change.set_display_status_cache("merge wait");
        }
    }

    pub(crate) fn handle_worktrees_refreshed(&mut self, worktrees: Vec<WorktreeInfo>) {
        self.worktrees = worktrees;

        if self.worktree_cursor_index >= self.worktrees.len() && !self.worktrees.is_empty() {
            self.worktree_cursor_index = self.worktrees.len() - 1;
        }
    }

    /// Adopt one presentation-only observation from the local refresh task.
    ///
    /// The whole path is deliberately narrow: the only way this frontend's
    /// workspace dirty value can move is a typed event the refresh task
    /// publishes after a *completed* `git status` read, so a failed or
    /// unfinished read has no way to be presented as clean. Nothing here touches
    /// the reducer, the mark store, the queue, or any workflow decision.
    pub(crate) fn adopt_workspace_dirty_observation(&mut self, observation: TuiRefreshObservation) {
        match observation {
            TuiRefreshObservation::WorkspaceDirty { dirty } => {
                self.workspace_dirty = WorkspaceDirtyState::observed(dirty);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn create_test_change_with_progress(id: &str, completed: u32, total: u32) -> Change {
        Change {
            completed_tasks: completed,
            total_tasks: total,
            ..create_test_change(id)
        }
    }

    fn count_blocked_logs(app: &AppState, change_id: &str) -> usize {
        let message = format!("Change '{}' blocked by dependencies", change_id);
        app.logs
            .iter()
            .filter(|entry| entry.message == message)
            .count()
    }

    type RefreshSets = (
        HashSet<String>,
        HashSet<String>,
        HashSet<String>,
        HashMap<String, PathBuf>,
        HashSet<String>,
    );

    fn empty_refresh_sets() -> RefreshSets {
        (
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashMap::new(),
            HashSet::new(),
        )
    }

    /// Unknown, clean, and dirty stay three distinguishable facts, and each
    /// successful observation replaces the previous one.
    #[test]
    fn workspace_dirty_header_state_adopts_successful_observations() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        assert_eq!(app.workspace_dirty(), WorkspaceDirtyState::Unknown);
        assert!(
            !app.workspace_dirty().shows_dirty_badge(),
            "an unobserved workspace must not render the badge"
        );

        app.adopt_workspace_dirty_observation(TuiRefreshObservation::WorkspaceDirty {
            dirty: true,
        });
        assert_eq!(app.workspace_dirty(), WorkspaceDirtyState::Dirty);
        assert!(app.workspace_dirty().shows_dirty_badge());

        app.adopt_workspace_dirty_observation(TuiRefreshObservation::WorkspaceDirty {
            dirty: false,
        });
        assert_eq!(
            app.workspace_dirty(),
            WorkspaceDirtyState::Clean,
            "a later successful clean observation must replace the dirty one"
        );
        assert!(!app.workspace_dirty().shows_dirty_badge());
        assert_ne!(
            app.workspace_dirty(),
            WorkspaceDirtyState::Unknown,
            "an observed clean workspace is a positive fact, not the absence of one"
        );
    }

    /// A failed Git status read publishes no observation at all, and the refresh
    /// traffic that keeps flowing around it must not reset what was last seen.
    #[test]
    fn workspace_dirty_header_state_preserves_last_success_on_failure() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.adopt_workspace_dirty_observation(TuiRefreshObservation::WorkspaceDirty {
            dirty: true,
        });

        // The same tick whose dirty read failed still delivers its ordinary
        // catalog refresh, and other events keep arriving behind it.
        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::new(),
        );
        app.handle_orchestrator_event(crate::tui::events::OrchestratorEvent::Stopped);

        assert_eq!(
            app.workspace_dirty(),
            WorkspaceDirtyState::Dirty,
            "a failed observation must never be reported as clean"
        );

        // Unknown is preserved by the same absence: nothing but a successful
        // observation may make a cleanliness claim.
        let mut unobserved = AppState::new(vec![create_test_change("change-a")]);
        unobserved.handle_orchestrator_event(crate::tui::events::OrchestratorEvent::Stopped);
        assert_eq!(unobserved.workspace_dirty(), WorkspaceDirtyState::Unknown);
    }

    #[test]
    fn changes_refreshed_adds_new_active_log_and_rejected_non_new_without_moving_cursor() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.cursor_index = 0;
        app.list_state.select(Some(0));
        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();

        app.handle_changes_refreshed(
            vec![
                create_test_change("change-a"),
                create_test_change("change-new"),
            ],
            vec![create_test_change("change-rejected")],
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::new(),
        );

        let active = app
            .changes
            .iter()
            .find(|c| c.id == "change-new")
            .expect("active new row");
        assert_eq!(active.display_status_cache, "not queued");
        assert!(active.is_new);
        assert!(!active.selected);

        let rejected = app
            .changes
            .iter()
            .find(|c| c.id == "change-rejected")
            .expect("rejected row");
        assert_eq!(rejected.display_status_cache, "rejected");
        assert!(!rejected.is_new);
        assert!(!rejected.selected);

        assert_eq!(app.cursor_index, 0, "refresh must not steal cursor focus");
        assert_eq!(app.new_change_count, 1);
        assert_eq!(
            app.logs
                .iter()
                .filter(|entry| entry.message == "Detected new change: change-new")
                .count(),
            1
        );
    }

    /// A proposal can be missing from a single filesystem snapshot while its
    /// worktree is refreshed or merged into the base branch. The row is dropped,
    /// but the proposal never stopped existing — so once discovery observes it
    /// again the row has to come back, rebuilt from *that* snapshot's data.
    ///
    /// The same pass proves the opposite half of the rule: a row deliberately
    /// retained through the absence (here, one with a recorded start) is updated
    /// in place when it is observed again, not duplicated, re-badged, or
    /// re-logged. Nothing in the sequence may move the cursor, select a row, or
    /// otherwise touch workflow intent.
    #[test]
    fn changes_refreshed_restores_change_after_transient_absence() {
        fn refresh(app: &mut AppState, active: Vec<Change>, rejected: Vec<Change>) {
            let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
            app.handle_changes_refreshed(
                active,
                rejected,
                committed,
                uncommitted,
                worktrees,
                paths,
                not_ahead,
                HashSet::new(),
            );
        }

        fn detection_logs(app: &AppState, change_id: &str) -> usize {
            let message = format!("Detected new change: {}", change_id);
            app.logs
                .iter()
                .filter(|entry| entry.message == message)
                .count()
        }

        fn row<'a>(app: &'a AppState, change_id: &str) -> &'a crate::tui::state::ChangeState {
            app.changes
                .iter()
                .find(|c| c.id == change_id)
                .unwrap_or_else(|| panic!("row for '{}' is missing", change_id))
        }

        fn rows_with_id(app: &AppState, change_id: &str) -> usize {
            app.changes.iter().filter(|c| c.id == change_id).count()
        }

        let mut app = AppState::new(vec![
            create_test_change("change-anchor"),
            create_test_change("change-retained"),
        ]);
        // A recorded start is one of the reasons the projection deliberately
        // keeps a row that the current snapshot does not observe.
        app.changes[1].started_at = Some(std::time::Instant::now());
        app.cursor_index = 1;
        app.list_state.select(Some(1));

        // Snapshot 1: everything is observable. Two proposals are detected for
        // the first time, both as ordinary active rows.
        refresh(
            &mut app,
            vec![
                create_test_change("change-anchor"),
                create_test_change_with_progress("change-retained", 2, 5),
                create_test_change_with_progress("change-vanishing", 1, 4),
                create_test_change("change-rejecting"),
            ],
            Vec::new(),
        );

        assert_eq!(rows_with_id(&app, "change-vanishing"), 1);
        assert!(row(&app, "change-vanishing").is_new);
        assert_eq!(detection_logs(&app, "change-vanishing"), 1);
        assert_eq!(
            row(&app, "change-rejecting").display_status_cache,
            "not queued"
        );
        assert_eq!(detection_logs(&app, "change-rejecting"), 1);
        assert_eq!(app.new_change_count, 2);

        // Snapshot 2: the transient absence. Discovery observes neither the
        // active proposal nor the rejected one, and the retained row survives on
        // its recorded start alone.
        refresh(
            &mut app,
            vec![create_test_change("change-anchor")],
            Vec::new(),
        );

        assert_eq!(rows_with_id(&app, "change-vanishing"), 0);
        assert_eq!(rows_with_id(&app, "change-rejecting"), 0);
        assert!(
            !app.known_change_ids.contains("change-vanishing"),
            "an identity entry must not outlive the row it belongs to"
        );
        assert!(
            !app.known_change_ids.contains("change-rejecting"),
            "an identity entry must not outlive the row it belongs to"
        );
        assert!(
            app.known_change_ids.contains("change-retained"),
            "a row kept through the absence must stay known"
        );
        assert_eq!(rows_with_id(&app, "change-retained"), 1);
        assert_eq!(
            app.new_change_count, 0,
            "the NEW badge must not count a row this pass dropped"
        );

        // Snapshot 3: both proposals are observable again, with progress that
        // differs from what the removed rows last held.
        refresh(
            &mut app,
            vec![
                create_test_change("change-anchor"),
                create_test_change_with_progress("change-retained", 3, 5),
                create_test_change_with_progress("change-vanishing", 2, 4),
            ],
            vec![create_test_change("change-rejecting")],
        );

        let restored = row(&app, "change-vanishing");
        assert_eq!(rows_with_id(&app, "change-vanishing"), 1);
        assert_eq!(
            (restored.completed_tasks, restored.total_tasks),
            (2, 4),
            "the restored row must be rebuilt from the current refresh data"
        );
        assert_eq!(restored.display_status_cache, "not queued");
        assert!(
            restored.is_new,
            "a reappearing active proposal is newly detected again"
        );
        assert_eq!(
            detection_logs(&app, "change-vanishing"),
            2,
            "exactly one additional detection log for the reappearance"
        );

        // The retained row was never removed, so re-observing it is an in-place
        // update: no duplicate, no NEW badge, no detection log.
        let retained = row(&app, "change-retained");
        assert_eq!(rows_with_id(&app, "change-retained"), 1);
        assert_eq!((retained.completed_tasks, retained.total_tasks), (3, 5));
        assert!(!retained.is_new);
        assert_eq!(detection_logs(&app, "change-retained"), 0);

        // A proposal whose row was dropped and which comes back rejected is
        // rebuilt read-only: no active NEW badge, no active detection log.
        let rejected = row(&app, "change-rejecting");
        assert_eq!(rows_with_id(&app, "change-rejecting"), 1);
        assert_eq!(rejected.display_status_cache, "rejected");
        assert!(!rejected.is_new);
        assert!(!rejected.selected);
        assert_eq!(
            detection_logs(&app, "change-rejecting"),
            1,
            "the rejected reappearance adds no active detection log"
        );
        assert_eq!(
            app.new_change_count, 1,
            "only the reappearing active proposal counts toward the NEW badge"
        );

        // Reappearance is observability only: no cursor movement, no mark, and
        // the identity set still equals exactly the rows that are present.
        assert_eq!(app.cursor_index, 1, "reappearance must not move the cursor");
        assert_eq!(app.list_state.selected(), Some(1));
        assert!(
            app.changes.iter().all(|c| !c.selected),
            "refresh must never mark a change for execution"
        );
        let projected_ids: HashSet<String> = app.changes.iter().map(|c| c.id.clone()).collect();
        assert_eq!(app.known_change_ids, projected_ids);
    }

    #[test]
    fn merge_wait_refresh_corrects_stale_resolve_pending_row() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.changes[0].set_display_status_cache("resolve pending");
        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "change-a".to_string(),
            "merge wait",
        )]));
        app.changes[0].set_display_status_cache("resolve pending");

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["change-a".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    #[test]
    fn merge_wait_refresh_preserves_reducer_owned_resolve_pending_row() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "change-a".to_string(),
            "resolve pending",
        )]));

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["change-a".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
    }

    #[test]
    fn merge_wait_refresh_preserves_reducer_owned_resolving_row() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.apply_display_statuses_from_reducer(&HashMap::from([(
            "change-a".to_string(),
            "resolving",
        )]));

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["change-a".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "resolving");
    }

    #[test]
    fn merge_wait_refresh_preserves_reducer_owned_reject_pending_and_error_rows() {
        let mut app = AppState::new(vec![
            create_test_change("reject-pending"),
            create_test_change("error-change"),
        ]);
        app.apply_display_statuses_from_reducer(&HashMap::from([
            ("reject-pending".to_string(), "reject pending"),
            ("error-change".to_string(), "error"),
        ]));

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![
                create_test_change("reject-pending"),
                create_test_change("error-change"),
            ],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["reject-pending".to_string(), "error-change".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "reject pending");
        assert_eq!(app.changes[1].display_status_cache, "error");
    }

    /// The precedence rule is bound to the shared vocabulary, not to a list
    /// copied into this module: adding a status to `ACTIVE_STATUSES` must extend
    /// this protection automatically, which is exactly what `archiving` did not
    /// get from the previous hand-written subset.
    #[test]
    fn merge_wait_refresh_protects_every_shared_active_status() {
        for active_status in crate::orchestration::operator_command::ACTIVE_STATUSES {
            let mut app = AppState::new(vec![create_test_change("change-a")]);
            app.apply_display_statuses_from_reducer(&HashMap::from([(
                "change-a".to_string(),
                active_status,
            )]));
            assert_eq!(
                app.changes[0].display_status_cache, active_status,
                "reducer sync did not place '{}' on the row",
                active_status
            );

            let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
            app.handle_changes_refreshed(
                vec![create_test_change("change-a")],
                Vec::new(),
                committed,
                uncommitted,
                worktrees,
                paths,
                not_ahead,
                HashSet::from(["change-a".to_string()]),
            );

            assert_eq!(
                app.changes[0].display_status_cache, active_status,
                "refresh merge-wait evidence overwrote active status '{}'",
                active_status
            );
        }
    }

    /// Startup restoration is the reason the refresh detector still exists: a
    /// fresh process owns no reducer lifecycle history, so archived-but-not-
    /// integrated evidence is the only thing that can surface the manual row.
    #[test]
    fn merge_wait_refresh_restores_fresh_process_archived_workspace() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        assert!(
            app.reducer_display_status_snapshot.is_empty(),
            "a fresh process must start with no reducer lifecycle history"
        );

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![create_test_change("change-a")],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["change-a".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    /// `merge wait` still means what it always meant — concrete manual deferral
    /// — and the reducer, not the refresh scan, is what establishes it. The
    /// auto-resumable and active rows in the same pass prove refresh evidence
    /// alone never invents it.
    #[test]
    fn merge_wait_refresh_preserves_concrete_manual_merge_deferral() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::OrchestratorState;

        let mut state = OrchestratorState::new(
            vec![
                "manual".to_string(),
                "auto".to_string(),
                "active".to_string(),
            ],
            10,
        );
        state.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "manual".to_string(),
            reason: "base has local commits".to_string(),
            auto_resumable: false,
        });
        state.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "auto".to_string(),
            reason: "merge in progress".to_string(),
            auto_resumable: true,
        });
        state.apply_execution_event(&ExecutionEvent::ArchiveStarted {
            change_id: "active".to_string(),
            command: "archive".to_string(),
        });

        assert_eq!(state.display_status("manual"), "merge wait");
        assert_ne!(
            state.display_status("auto"),
            "merge wait",
            "auto-resumable deferral is scheduler-owned retry intent, not manual wait"
        );
        assert_ne!(
            state.display_status("active"),
            "merge wait",
            "active execution is not a manual wait"
        );

        let mut app = AppState::new(vec![
            create_test_change("manual"),
            create_test_change("auto"),
            create_test_change("active"),
        ]);
        app.apply_display_statuses_from_reducer(&state.all_display_statuses());

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![
                create_test_change("manual"),
                create_test_change("auto"),
                create_test_change("active"),
            ],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from([
                "manual".to_string(),
                "auto".to_string(),
                "active".to_string(),
            ]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert_eq!(app.changes[1].display_status_cache, "resolve pending");
        assert_eq!(app.changes[2].display_status_cache, "archiving");
    }

    #[test]
    fn merge_wait_refresh_preserves_terminal_rows() {
        let mut app = AppState::new(vec![
            create_test_change("merged-change"),
            create_test_change("rejected-change"),
        ]);
        app.changes[0].set_display_status_cache("merged");
        app.changes[1].set_display_status_cache("rejected");

        let (committed, uncommitted, worktrees, paths, not_ahead) = empty_refresh_sets();
        app.handle_changes_refreshed(
            vec![
                create_test_change("merged-change"),
                create_test_change("rejected-change"),
            ],
            Vec::new(),
            committed,
            uncommitted,
            worktrees,
            paths,
            not_ahead,
            HashSet::from(["merged-change".to_string(), "rejected-change".to_string()]),
        );

        assert_eq!(app.changes[0].display_status_cache, "merged");
        assert_eq!(app.changes[1].display_status_cache, "rejected");
    }

    #[test]
    fn repeated_dependency_blocked_updates_status_without_duplicate_log() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        app.handle_dependency_blocked("change-a".to_string());
        app.handle_dependency_blocked("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "blocked");
        assert_eq!(count_blocked_logs(&app, "change-a"), 1);
    }

    #[test]
    fn dependency_resolved_then_reblocked_logs_again() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        app.handle_dependency_blocked("change-a".to_string());
        app.handle_dependency_resolved("change-a".to_string());
        app.handle_dependency_blocked("change-a".to_string());

        assert_eq!(app.changes[0].display_status_cache, "blocked");
        assert_eq!(count_blocked_logs(&app, "change-a"), 2);
    }
}
