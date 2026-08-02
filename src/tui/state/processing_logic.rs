use std::{collections::HashSet, time::Instant};

use crate::{openspec::Change, task_parser, tui::events::LogEntry};

use super::{AppState, ChangeState};

pub(super) fn update_changes_with_rejected(
    state: &mut AppState,
    fetched_changes: Vec<Change>,
    rejected_changes: Vec<Change>,
) {
    let active_ids: HashSet<String> = fetched_changes.iter().map(|c| c.id.clone()).collect();
    let rejected_ids: HashSet<String> = rejected_changes.iter().map(|c| c.id.clone()).collect();

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
                existing.selected = false;
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
            existing.selected = false;
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
            rejected_state.selected = false;
            rejected_state.set_display_status_cache("rejected");
            state.changes.push(rejected_state);
        }
    }

    for id in &new_active_ids {
        state.add_log(LogEntry::info(format!("Detected new change: {}", id)));
    }

    state.known_change_ids.extend(new_ids);
    state.new_change_count = state.changes.iter().filter(|c| c.is_new).count();
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
            || c.started_at.is_some()
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

    if state.cursor_index >= state.changes.len() && !state.changes.is_empty() {
        state.cursor_index = state.changes.len() - 1;
        state.list_state.select(Some(state.cursor_index));
    }
}
