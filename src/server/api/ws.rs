use super::*;

/// GET /api/v1/ws - WebSocket stream of remote state updates
///
/// Current behavior:
/// - Sends periodic FullState snapshots (simple, reliable for clients).
/// - Also sends Ping messages to keep the connection alive.
/// - Streams Log entries from the execution log broadcast channel.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let registry = state.registry.clone();
    let log_rx = state.log_tx.subscribe();
    let state_update_rx = state.state_update_tx.subscribe();
    let context = WsSnapshotContext {
        sync_available: state.resolve_command.is_some(),
        orchestration_status: state.orchestration_status.clone(),
        shared_orchestrator_state: state.shared_orchestrator_state.clone(),
        active_commands: state.active_commands.clone(),
        db: state.db.clone(),
    };

    ws.on_upgrade(move |socket| handle_ws(socket, registry, log_rx, state_update_rx, context))
}

#[derive(Clone)]
struct WsSnapshotContext {
    sync_available: bool,
    orchestration_status: Arc<tokio::sync::RwLock<OrchestrationStatus>>,
    shared_orchestrator_state:
        Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    active_commands: SharedActiveCommands,
    db: Option<Arc<ServerDb>>,
}

async fn handle_ws(
    mut socket: WebSocket,
    registry: SharedRegistry,
    mut log_rx: tokio::sync::broadcast::Receiver<RemoteLogEntry>,
    mut state_update_rx: tokio::sync::broadcast::Receiver<RemoteStateUpdate>,
    context: WsSnapshotContext,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Snapshot
                let (entries, data_dir, all_selections, all_errors) = {
                    let reg = registry.read().await;
                    let entries = reg.list();
                    let data_dir = reg.data_dir().to_path_buf();
                    let all_selections: std::collections::HashMap<String, std::collections::HashMap<String, bool>> = entries
                        .iter()
                        .filter_map(|e| {
                            reg.change_selections_for_project(&e.id)
                                .map(|s| (e.id.clone(), s.clone()))
                        })
                        .collect();
                    let all_errors: std::collections::HashMap<String, std::collections::HashMap<String, String>> = entries
                        .iter()
                        .filter_map(|e| {
                            reg.error_changes_for_project(&e.id)
                                .map(|s| (e.id.clone(), s.clone()))
                        })
                        .collect();
                    (entries, data_dir, all_selections, all_errors)
                };

                let mut snapshot = Vec::new();
                for entry in &entries {
                    let selections = all_selections.get(&entry.id);
                    let errors = all_errors.get(&entry.id);
                    snapshot.push(
                        build_remote_project_snapshot_async(
                            &data_dir,
                            entry,
                            selections,
                            errors,
                            &context.shared_orchestrator_state,
                        )
                        .await,
                    );
                }

                // Collect worktree information for each project
                let mut worktrees_map = std::collections::HashMap::new();
                for entry in &entries {
                    let wt_path = data_dir
                        .join("worktrees")
                        .join(&entry.id)
                        .join(&entry.branch);
                    if wt_path.exists() {
                        if let Ok(wts) = crate::worktree_ops::get_worktrees(&wt_path).await {
                            let remote_wts: Vec<crate::remote::types::RemoteWorktreeInfo> =
                                wts.into_iter().map(Into::into).collect();
                            if !remote_wts.is_empty() {
                                worktrees_map.insert(entry.id.clone(), remote_wts);
                            }
                        }
                    }
                }
                let worktrees = if worktrees_map.is_empty() {
                    None
                } else {
                    Some(worktrees_map)
                };

                let ui_state = if let Some(db) = &context.db {
                    match db.get_all_ui_state() {
                        Ok(state) => state,
                        Err(e) => {
                            warn!(error = %e, "Failed to load ui_state for websocket snapshot");
                            std::collections::HashMap::new()
                        }
                    }
                } else {
                    std::collections::HashMap::new()
                };

                let orch_status = context.orchestration_status.read().await.as_str().to_string();
                let active_cmds = {
                    let ac = context.active_commands.read().await;
                    ac.snapshot()
                };
                if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::FullState {
                    projects: snapshot,
                    worktrees,
                    ui_state,
                    sync_available: context.sync_available,
                    orchestration_status: orch_status,
                    active_commands: active_cmds,
                }) {
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }

                // Keep-alive ping
                if let Ok(ping) = serde_json::to_string(&RemoteStateUpdate::Ping) {
                    if socket.send(Message::Text(ping.into())).await.is_err() {
                        break;
                    }
                }
            }

            log_result = log_rx.recv() => {
                match log_result {
                    Ok(entry) => {
                        if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::Log { entry }) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS log receiver lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Sender closed – stop streaming logs but keep WS open
                    }
                }
            }

            state_update_result = state_update_rx.recv() => {
                match state_update_result {
                    Ok(update) => {
                        if let Ok(payload) = serde_json::to_string(&update) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS state update receiver lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Sender closed – stop streaming incremental updates but keep WS open
                    }
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {
                        // Ignore client messages for now.
                    }
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

pub(super) async fn build_remote_project_snapshot_async(
    data_dir: &std::path::Path,
    entry: &ProjectEntry,
    change_selections: Option<&std::collections::HashMap<String, bool>>,
    error_changes: Option<&std::collections::HashMap<String, String>>,
    shared_orchestrator_state: &Arc<
        tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
    >,
) -> RemoteProject {
    let name = project_display_name(&entry.remote_url, &entry.branch);
    let repo = extract_repo_name(&entry.remote_url);
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);

    let mut changes = list_remote_changes_in_worktree(
        &worktree_path,
        &entry.id,
        &entry.branch,
        shared_orchestrator_state,
    )
    .await;

    // Apply selected/error state from registry.
    for change in &mut changes {
        let is_rejected = change.status == "rejected";
        let is_error = error_changes
            .and_then(|m| m.get(&change.id))
            .map(|error| {
                change.status = "error".to_string();
                change.iteration_number = None;
                !error.is_empty()
            })
            .unwrap_or(false);
        let default_selected = !is_error;
        change.selected = if is_rejected {
            false
        } else {
            change_selections
                .and_then(|m| m.get(&change.id))
                .copied()
                .unwrap_or(default_selected)
        };
    }

    let status_str = match entry.status {
        ProjectStatus::Idle => "idle",
        ProjectStatus::Running => "running",
        ProjectStatus::Stopped => "stopped",
    };
    let is_busy = matches!(entry.status, ProjectStatus::Running);

    RemoteProject {
        id: entry.id.clone(),
        name,
        repo,
        branch: entry.branch.clone(),
        status: status_str.to_string(),
        is_busy,
        error: None,
        sync_state: entry.sync_metadata.sync_state.as_str().to_string(),
        ahead_count: entry.sync_metadata.ahead_count,
        behind_count: entry.sync_metadata.behind_count,
        sync_required: entry.sync_metadata.sync_required,
        local_sha: entry.sync_metadata.local_sha.clone(),
        remote_sha: entry.sync_metadata.remote_sha.clone(),
        last_remote_check_at: entry.sync_metadata.last_remote_check_at.clone(),
        remote_check_error: entry.sync_metadata.remote_check_error.clone(),
        changes,
    }
}

/// Extract the repository name from a remote URL (last path segment without .git suffix).
///
/// For standard git remotes ending in `.git`, the `.git` suffix is stripped.
/// For unusual URLs, falls back to the best available non-empty label.
pub(super) fn extract_repo_name(remote_url: &str) -> String {
    let basename = remote_url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(remote_url)
        .trim_end_matches(".git");

    if basename.is_empty() {
        // Fallback: use the full URL rather than an empty string
        remote_url.to_string()
    } else {
        basename.to_string()
    }
}

pub(super) fn project_display_name(remote_url: &str, branch: &str) -> String {
    // Keep it short but recognizable: repo@branch
    let repo = extract_repo_name(remote_url);
    format!("{}@{}", repo, branch)
}

fn map_workspace_state_fallback(state: WorkspaceState) -> (String, Option<u32>) {
    match state {
        WorkspaceState::Created => ("created".to_string(), None),
        WorkspaceState::Applying { iteration } => ("applying".to_string(), Some(iteration)),
        WorkspaceState::Applied => ("applied".to_string(), None),
        WorkspaceState::Archiving => ("archiving".to_string(), None),
        WorkspaceState::Archived => ("archived".to_string(), None),
        WorkspaceState::Merged => ("merged".to_string(), None),
        WorkspaceState::Rejecting => ("rejecting".to_string(), None),
    }
}

async fn derive_change_status(
    change_id: &str,
    worktree_by_change: &std::collections::HashMap<String, std::path::PathBuf>,
    status_map: &std::collections::HashMap<String, &'static str>,
    base_branch: &str,
    rejected_marker_exists: bool,
) -> (String, Option<u32>) {
    if rejected_marker_exists {
        return ("rejected".to_string(), None);
    }

    if let Some(status) = status_map.get(change_id) {
        return (status.to_string(), None);
    }

    if let Some(wt_path) = worktree_by_change.get(change_id) {
        return match detect_workspace_state(change_id, wt_path, base_branch).await {
            Ok(state) => map_workspace_state_fallback(state),
            Err(_) => ("idle".to_string(), None),
        };
    }

    ("idle".to_string(), None)
}

pub(super) async fn list_remote_changes_in_worktree(
    worktree_path: &std::path::Path,
    project_id: &str,
    base_branch: &str,
    shared_orchestrator_state: &Arc<
        tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>,
    >,
) -> Vec<RemoteChange> {
    let changes_dir = worktree_path.join("openspec/changes");
    if !changes_dir.exists() {
        return Vec::new();
    }

    // Build a mapping from sanitized change_id -> worktree path.
    // This allows us to read progress from active worktrees created during parallel execution.
    let mut worktree_by_change: std::collections::HashMap<String, std::path::PathBuf> =
        std::collections::HashMap::new();
    if let Ok(worktrees) = crate::vcs::git::commands::list_worktrees(worktree_path).await {
        for (path, _head, branch, _is_detached, is_main) in worktrees {
            if is_main {
                continue;
            }
            if let Some(change_id) =
                GitWorkspaceManager::extract_change_id_from_worktree_name(&branch)
            {
                worktree_by_change.insert(change_id, std::path::PathBuf::from(path));
            }
        }
    }

    let entries = match std::fs::read_dir(&changes_dir) {
        Ok(e) => e,
        Err(e) => {
            debug!("Failed to read changes dir {:?}: {}", changes_dir, e);
            return Vec::new();
        }
    };

    let status_map: std::collections::HashMap<String, &'static str> = {
        let guard = shared_orchestrator_state.read().await;
        guard.all_display_statuses()
    };

    let mut changes = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        // Include changes that have proposal.md. Also keep REJECTED.md-bearing
        // directories visible in dashboard snapshots as terminal rows.
        let proposal_path = path.join("proposal.md");
        let rejected_marker_path = path.join("REJECTED.md");
        let rejected_marker_exists = rejected_marker_path.exists();
        if !proposal_path.exists() {
            continue;
        }

        let tasks_path = path.join("tasks.md");

        // Prefer worktree progress if the change is currently executing in a worktree.
        let wt_path_opt = worktree_by_change.get(dir_name).map(|p| p.as_path());
        let (completed, total) =
            match task_parser::parse_progress_with_fallback(dir_name, wt_path_opt) {
                Ok(p) => (p.completed, p.total),
                Err(_) => {
                    // Last resort: try base tasks.md directly.
                    match task_parser::parse_file(&tasks_path, Some(dir_name)) {
                        Ok(p) => (p.completed, p.total),
                        Err(_) => (0, 0),
                    }
                }
            };

        let last_modified = latest_modified_rfc3339(&[&proposal_path, &tasks_path])
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let (status, iteration_number) = derive_change_status(
            dir_name,
            &worktree_by_change,
            &status_map,
            base_branch,
            rejected_marker_exists,
        )
        .await;

        changes.push(RemoteChange {
            id: dir_name.to_string(),
            project: project_id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified,
            status,
            iteration_number,
            selected: true,
        });
    }

    changes.sort_by(|a, b| a.id.cmp(&b.id));
    changes
}

fn latest_modified_rfc3339(paths: &[&std::path::Path]) -> Option<String> {
    use std::time::SystemTime;

    let mut latest: Option<SystemTime> = None;
    for p in paths {
        let m = std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if let Some(ts) = m {
            latest = Some(match latest {
                Some(cur) if cur >= ts => cur,
                _ => ts,
            });
        }
    }

    latest.map(|ts| chrono::DateTime::<chrono::Utc>::from(ts).to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_workspace_state_fallback_created_and_applied_are_aligned() {
        assert_eq!(
            map_workspace_state_fallback(WorkspaceState::Created),
            ("created".to_string(), None)
        );
        assert_eq!(
            map_workspace_state_fallback(WorkspaceState::Applied),
            ("applied".to_string(), None)
        );
    }

    #[test]
    fn test_map_workspace_state_fallback_preserves_iteration_for_applying() {
        assert_eq!(
            map_workspace_state_fallback(WorkspaceState::Applying { iteration: 7 }),
            ("applying".to_string(), Some(7))
        );
    }

    #[tokio::test]
    async fn test_derive_change_status_prefers_reducer_statuses() {
        let mut status_map = std::collections::HashMap::new();
        status_map.insert("c-accepting".to_string(), "accepting");
        status_map.insert("c-resolving".to_string(), "resolving");
        status_map.insert("c-merge-wait".to_string(), "merge wait");
        status_map.insert("c-resolve-pending".to_string(), "resolve pending");
        status_map.insert("c-blocked".to_string(), "blocked");

        let worktree_by_change = std::collections::HashMap::new();

        assert_eq!(
            derive_change_status(
                "c-accepting",
                &worktree_by_change,
                &status_map,
                "main",
                false
            )
            .await,
            ("accepting".to_string(), None)
        );
        assert_eq!(
            derive_change_status(
                "c-resolving",
                &worktree_by_change,
                &status_map,
                "main",
                false
            )
            .await,
            ("resolving".to_string(), None)
        );
        assert_eq!(
            derive_change_status(
                "c-merge-wait",
                &worktree_by_change,
                &status_map,
                "main",
                false
            )
            .await,
            ("merge wait".to_string(), None)
        );
        assert_eq!(
            derive_change_status(
                "c-resolve-pending",
                &worktree_by_change,
                &status_map,
                "main",
                false,
            )
            .await,
            ("resolve pending".to_string(), None)
        );
        assert_eq!(
            derive_change_status("c-blocked", &worktree_by_change, &status_map, "main", false)
                .await,
            ("blocked".to_string(), None)
        );
    }

    #[test]
    fn test_ws_display_status_matches_tui_reducer_display_status() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState, ReducerCommand};

        let mut orchestrator_state =
            OrchestratorState::with_mode(vec!["change-a".to_string()], 1, ExecutionMode::Parallel);

        orchestrator_state.apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        assert_eq!(orchestrator_state.display_status("change-a"), "queued");

        orchestrator_state.apply_execution_event(&ExecutionEvent::AcceptanceStarted {
            change_id: "change-a".to_string(),
            command: "accept --change change-a".to_string(),
        });
        assert_eq!(orchestrator_state.display_status("change-a"), "accepting");

        orchestrator_state.apply_execution_event(&ExecutionEvent::AcceptanceCompleted {
            change_id: "change-a".to_string(),
        });
        orchestrator_state.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "change-a".to_string(),
            reason: "manual resolve required".to_string(),
            auto_resumable: false,
        });
        assert_eq!(orchestrator_state.display_status("change-a"), "merge wait");

        orchestrator_state.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
        assert_eq!(
            orchestrator_state.display_status("change-a"),
            "resolve pending"
        );

        orchestrator_state.apply_execution_event(&ExecutionEvent::DependencyBlocked {
            change_id: "change-a".to_string(),
            dependency_ids: vec!["change-b".to_string()],
        });
        assert_eq!(orchestrator_state.display_status("change-a"), "blocked");
    }

    #[tokio::test]
    async fn test_derive_change_status_returns_idle_without_reducer_or_worktree() {
        let status_map = std::collections::HashMap::new();
        let worktree_by_change = std::collections::HashMap::new();

        assert_eq!(
            derive_change_status(
                "unknown-change",
                &worktree_by_change,
                &status_map,
                "main",
                false
            )
            .await,
            ("idle".to_string(), None)
        );
    }

    #[tokio::test]
    async fn test_derive_change_status_prefers_rejected_marker_over_reducer() {
        let mut status_map = std::collections::HashMap::new();
        status_map.insert("change-a".to_string(), "accepting");
        let worktree_by_change = std::collections::HashMap::new();

        assert_eq!(
            derive_change_status("change-a", &worktree_by_change, &status_map, "main", true).await,
            ("rejected".to_string(), None)
        );
    }

    #[tokio::test]
    async fn test_build_remote_project_snapshot_clears_rejected_selection() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let worktree_path = temp_dir
            .path()
            .join("worktrees")
            .join("project-1")
            .join("main");
        let rejected_change_dir = worktree_path
            .join("openspec")
            .join("changes")
            .join("change-rejected");
        std::fs::create_dir_all(&rejected_change_dir).unwrap();
        std::fs::write(rejected_change_dir.join("proposal.md"), "# proposal\n").unwrap();
        std::fs::write(rejected_change_dir.join("REJECTED.md"), "# REJECTED\n").unwrap();

        let entry = ProjectEntry {
            id: "project-1".to_string(),
            remote_url: "https://github.com/example/repo.git".to_string(),
            branch: "main".to_string(),
            status: ProjectStatus::Idle,
            sync_metadata: crate::server::registry::ProjectSyncMetadata::default(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let shared_state = Arc::new(tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::default(),
        ));
        let selections = std::collections::HashMap::from([("change-rejected".to_string(), true)]);

        let snapshot = build_remote_project_snapshot_async(
            temp_dir.path(),
            &entry,
            Some(&selections),
            None,
            &shared_state,
        )
        .await;

        let rejected = snapshot
            .changes
            .iter()
            .find(|change| change.id == "change-rejected")
            .expect("rejected change should be present");
        assert_eq!(rejected.status, "rejected");
        assert!(
            !rejected.selected,
            "rejected dashboard row must always be unselected"
        );
    }

    #[tokio::test]
    async fn test_list_remote_changes_includes_rejected_marker_change() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec").join("changes");

        let rejected_change_dir = changes_dir.join("change-rejected");
        std::fs::create_dir_all(&rejected_change_dir).unwrap();
        std::fs::write(rejected_change_dir.join("proposal.md"), "# proposal\n").unwrap();
        std::fs::write(rejected_change_dir.join("tasks.md"), "- [ ] pending\n").unwrap();
        std::fs::write(rejected_change_dir.join("REJECTED.md"), "# REJECTED\n").unwrap();

        let active_change_dir = changes_dir.join("change-active");
        std::fs::create_dir_all(&active_change_dir).unwrap();
        std::fs::write(active_change_dir.join("proposal.md"), "# proposal\n").unwrap();
        std::fs::write(active_change_dir.join("tasks.md"), "- [ ] pending\n").unwrap();

        let shared_state = Arc::new(tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::default(),
        ));

        let changes =
            list_remote_changes_in_worktree(temp_dir.path(), "project-1", "main", &shared_state)
                .await;

        let rejected_change = changes.iter().find(|change| change.id == "change-rejected");
        assert!(
            rejected_change.is_some(),
            "change with REJECTED.md marker must be included in dashboard snapshot"
        );
        assert_eq!(
            rejected_change.unwrap().status,
            "rejected",
            "REJECTED.md marker must force rejected status in dashboard snapshot"
        );

        assert!(changes.iter().any(|change| change.id == "change-active"));
    }

    #[test]
    fn test_extract_repo_name_standard_https() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo.git"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_https_without_git_suffix() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_ssh_url() {
        assert_eq!(
            extract_repo_name("git@github.com:owner/my-repo.git"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_trailing_slash() {
        assert_eq!(
            extract_repo_name("https://github.com/owner/my-repo/"),
            "my-repo"
        );
    }

    #[test]
    fn test_extract_repo_name_bare_name() {
        assert_eq!(extract_repo_name("my-repo"), "my-repo");
    }

    #[test]
    fn test_extract_repo_name_empty_falls_back_to_url() {
        let result = extract_repo_name("https://example.com/");
        assert!(!result.is_empty(), "Should not produce an empty repo name");
    }

    #[test]
    fn test_extract_repo_name_just_git_suffix() {
        assert_eq!(
            extract_repo_name("https://example.com/.git"),
            "https://example.com/.git",
            "A URL ending in .git with no basename should fall back to the full URL"
        );
    }
}
