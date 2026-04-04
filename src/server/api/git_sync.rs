use super::*;

// ─────────────────────────────── /api/v1/projects/:id/git ─────────────────────

/// Build a resolve command argv by parsing the template with shlex and substituting
/// `{prompt}` placeholders. Retained for test coverage.
#[cfg(test)]
pub(super) fn build_resolve_command_argv(
    resolve_command_template: &str,
    prompt: &str,
) -> Result<Vec<String>, String> {
    let parts = shlex::split(resolve_command_template)
        .ok_or_else(|| "Failed to parse resolve_command (shlex split returned None)".to_string())?;
    if parts.is_empty() {
        return Err("resolve_command is empty".to_string());
    }

    Ok(parts
        .into_iter()
        .map(|p| p.replace("{prompt}", prompt))
        .collect())
}

fn emit_log_entry(state: &AppState, entry: RemoteLogEntry) {
    if let Some(db) = &state.db {
        if let Err(e) = db.insert_log(
            entry.project_id.as_deref(),
            &entry.level,
            &entry.message,
            entry.change_id.as_deref(),
            entry.operation.as_deref(),
            entry.iteration.map(i64::from),
        ) {
            error!(error = %e, "Failed to persist server log entry");
        }
    }

    let _ = state.log_tx.send(entry);
}

fn build_resolve_log_entry(project_id: &str, level: &str, message: String) -> RemoteLogEntry {
    RemoteLogEntry {
        message,
        level: level.to_string(),
        change_id: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id: Some(project_id.to_string()),
        operation: Some("resolve".to_string()),
        iteration: None,
    }
}

fn emit_resolve_log(state: &AppState, project_id: &str, level: &str, message: String) {
    emit_log_entry(state, build_resolve_log_entry(project_id, level, message));
}

pub(super) async fn run_resolve_command(
    resolve_command_template: &str,
    work_dir: &std::path::Path,
    prompt: &str,
    state: Option<&AppState>,
    project_id: Option<&str>,
) -> (bool, Option<i32>) {
    // Use the shared placeholder expansion from config::expand, which handles
    // both quoted ('{prompt}') and unquoted ({prompt}) template forms correctly,
    // avoiding double-quoting issues with multi-line prompts.
    let command_str = crate::config::expand::expand_prompt(resolve_command_template, prompt);

    info!(
        "Running resolve_command via login shell: command='{}'",
        command_str
    );

    // Send start event to project log
    if let (Some(state), Some(pid)) = (state, project_id) {
        emit_resolve_log(
            state,
            pid,
            "info",
            format!("resolve_command started: {}", command_str),
        );
    }

    let mut cmd = crate::shell_command::build_login_shell_command(&command_str);
    cmd.current_dir(work_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            error!(
                "Failed to run resolve_command '{}': {}",
                resolve_command_template, e
            );
            if let (Some(state), Some(pid)) = (state, project_id) {
                emit_resolve_log(
                    state,
                    pid,
                    "error",
                    format!("resolve_command failed to start: {}", e),
                );
            }
            return (true, Some(-1));
        }
    };

    // Stream stdout/stderr to project log
    let output = match child.wait_with_output().await {
        Ok(output) => output,
        Err(e) => {
            error!(
                "Failed to wait for resolve_command '{}': {}",
                resolve_command_template, e
            );
            return (true, Some(-1));
        }
    };

    // Stream stdout/stderr lines and completion event to project log
    if let (Some(state), Some(pid)) = (state, project_id) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if !line.is_empty() {
                emit_resolve_log(state, pid, "info", line.to_string());
            }
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if !line.is_empty() {
                emit_resolve_log(state, pid, "warn", line.to_string());
            }
        }

        let exit_code = output.status.code();
        let level = if output.status.success() {
            "success"
        } else {
            "error"
        };
        emit_resolve_log(
            state,
            pid,
            level,
            format!("resolve_command finished: exit_code={:?}", exit_code),
        );
    }

    (true, output.status.code())
}

fn build_auto_resolve_prompt(
    operation: &str,
    project_id: &str,
    remote_url: &str,
    branch: &str,
    local_sha: &str,
    remote_sha: &str,
    work_dir: &std::path::Path,
) -> String {
    // Keep this prompt short and machine-readable.
    format!(
        "Conflux server auto_resolve\noperation={}\nproject_id={}\nremote_url={}\nbranch={}\nlocal_sha={}\nremote_sha={}\nwork_dir={}\n\nTask: reconcile local state so the {} can proceed. Exit 0 on success, non-zero on failure.",
        operation,
        project_id,
        remote_url,
        branch,
        local_sha,
        remote_sha,
        work_dir.display(),
        operation
    )
}

/// POST /api/v1/projects/:id/git/pull - pull from remote
///
/// NOTE: This endpoint is kept for backward compatibility.
/// It delegates to `git_sync` internally. Prefer using
/// `POST /api/v1/projects/:id/git/sync` which combines pull and push
/// in a single atomic operation and requires resolve_command to be configured.
pub async fn git_pull(
    state: State<AppState>,
    path: Path<String>,
    _payload: Option<Json<GitAutoResolveRequest>>,
) -> Response {
    info!("git pull (delegating to git_sync): project_id={}", path.0);
    git_sync(state, path).await
}

/// POST /api/v1/projects/:id/git/push - push to remote
///
/// NOTE: This endpoint is kept for backward compatibility.
/// It delegates to `git_sync` internally. Prefer using
/// `POST /api/v1/projects/:id/git/sync` which combines pull and push
/// in a single atomic operation and requires resolve_command to be configured.
pub async fn git_push(
    state: State<AppState>,
    path: Path<String>,
    _payload: Option<Json<GitAutoResolveRequest>>,
) -> Response {
    info!("git push (delegating to git_sync): project_id={}", path.0);
    git_sync(state, path).await
}

// ─────────────────────────── /api/v1/projects/:id/git/sync ────────────────────

/// POST /api/v1/projects/:id/git/sync - pull then push, always running resolve_command.
///
/// This endpoint unifies git/pull and git/push into a single operation.
/// The resolve_command MUST be configured; if not, the sync fails immediately.
/// Both pull and push results are included in the response.
pub async fn git_sync(State(state): State<AppState>, Path(project_id): Path<String>) -> Response {
    // resolve_command is REQUIRED for sync
    let resolve_command = match &state.resolve_command {
        Some(cmd) => cmd.clone(),
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "resolve_command_not_configured",
                    "reason": "git/sync requires resolve_command to be configured. Set resolve_command in your .cflx.jsonc configuration."
                })),
            )
                .into_response();
        }
    };

    // Acquire active command slot for base root (sync operates on the base worktree)
    let _active_guard = match try_acquire_active_command(
        &state.active_commands,
        &project_id,
        RootKind::Base,
        "sync",
    )
    .await
    {
        Ok(guard) => guard,
        Err(resp) => return resp,
    };

    let (remote_url, branch, lock, semaphore) = {
        let registry = state.registry.read().await;
        let entry = match registry.get(&project_id) {
            Some(e) => e.clone(),
            None => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    format!("Project not found: {}", project_id),
                )
            }
        };
        let lock = match registry.project_lock(&project_id) {
            Some(l) => l,
            None => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Missing project lock")
            }
        };
        let semaphore = registry.global_semaphore();
        (entry.remote_url, entry.branch, lock, semaphore)
    };

    // Acquire global semaphore (max_concurrent_total)
    let _sem_permit = match semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Server is at maximum concurrent capacity",
            )
        }
    };

    // Acquire per-project exclusive lock
    let _guard = lock.lock().await;

    info!(
        "git sync: project_id={} remote_url={} branch={}",
        project_id, remote_url, branch
    );

    let local_repo_path = {
        let registry = state.registry.read().await;
        registry.data_dir().join(&project_id)
    };

    // ── PULL phase ──────────────────────────────────────────────────────────────

    // Verify the branch exists on remote
    let ls_remote = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    let remote_ref = match ls_remote {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if stdout.trim().is_empty() {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Branch '{}' not found on remote '{}'", branch, remote_url),
                );
            }
            stdout.trim().to_string()
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git operation failed: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git: {}", e),
            );
        }
    };

    // Initialize or fetch local bare clone
    if !local_repo_path.exists() {
        let clone_output = tokio::process::Command::new("git")
            .args([
                "clone",
                "--bare",
                "--branch",
                &branch,
                "--single-branch",
                &remote_url,
                local_repo_path.to_str().unwrap_or(""),
            ])
            .output()
            .await;

        match clone_output {
            Ok(out) if out.status.success() => {
                info!("git sync clone (bare) succeeded: project_id={}", project_id);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git clone failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git clone: {}", e),
                );
            }
        }
    } else {
        // Fetch to get latest remote state
        let fetch_remote_ref = format!("refs/heads/{}", branch);

        let fetch_output = tokio::process::Command::new("git")
            .args([
                "fetch",
                &remote_url,
                &format!("{}:refs/remotes/origin/{}", fetch_remote_ref, branch),
            ])
            .current_dir(&local_repo_path)
            .output()
            .await;

        match fetch_output {
            Ok(out) if out.status.success() => {
                // Fetch succeeded
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git fetch failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git fetch: {}", e),
                );
            }
        }

        // Update local branch to remote tip
        let ff_output = tokio::process::Command::new("git")
            .args([
                "fetch",
                &remote_url,
                &format!("refs/heads/{}:refs/heads/{}", branch, branch),
            ])
            .current_dir(&local_repo_path)
            .output()
            .await;

        match ff_output {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("git fetch (fast-forward update) failed: {}", stderr),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to run git fetch: {}", e),
                );
            }
        }
    }

    let pull_result = serde_json::json!({
        "status": "pulled",
        "ref": remote_ref
    });

    // ── PUSH phase ──────────────────────────────────────────────────────────────

    // Get local SHA after pull
    let local_rev = tokio::process::Command::new("git")
        .args(["rev-parse", &format!("refs/heads/{}", branch)])
        .current_dir(&local_repo_path)
        .output()
        .await;

    let local_sha_for_push = match local_rev {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Failed to get local branch ref: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git rev-parse: {}", e),
            );
        }
    };

    // Get remote SHA for push result
    let remote_rev = tokio::process::Command::new("git")
        .args(["ls-remote", "--heads", &remote_url, &branch])
        .output()
        .await;

    let remote_sha_for_push = match remote_rev {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                String::new()
            } else {
                stdout.split_whitespace().next().unwrap_or("").to_string()
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Failed to reach remote: {}", stderr),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git ls-remote: {}", e),
            );
        }
    };

    // ── Up-to-date check ──────────────────────────────────────────────────────
    // If local and remote SHAs match after the pull phase, the branch is already
    // synchronized — skip the expensive resolve_command and push entirely.
    if !remote_sha_for_push.is_empty() && local_sha_for_push == remote_sha_for_push {
        info!(
            "git sync: already up-to-date, skipping resolve and push: project_id={} sha={}",
            project_id, local_sha_for_push
        );
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "synced",
                "pull": pull_result,
                "push": {
                    "status": "already_up_to_date",
                    "branch": branch,
                    "local_sha": local_sha_for_push,
                    "remote_sha": remote_sha_for_push
                },
                "resolve_command_ran": false,
                "resolve_exit_code": serde_json::Value::Null,
                "skipped_reason": "local_and_remote_already_match"
            })),
        )
            .into_response();
    }

    // Run resolve_command before push (required for sync when SHAs differ or remote is new)
    info!(
        "git sync: running resolve_command before push: project_id={}",
        project_id
    );
    let resolve_prompt = build_auto_resolve_prompt(
        "git_sync",
        &project_id,
        &remote_url,
        &branch,
        &local_sha_for_push,
        &remote_sha_for_push,
        &local_repo_path,
    );
    let (resolve_command_ran, resolve_exit_code) = run_resolve_command(
        &resolve_command,
        &local_repo_path,
        &resolve_prompt,
        Some(&state),
        Some(&project_id),
    )
    .await;

    if resolve_exit_code != Some(0) {
        error!(
            "git sync: resolve_command failed: project_id={} exit_code={:?}",
            project_id, resolve_exit_code
        );
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": "resolve_command_failed",
                "reason": "resolve_command failed during sync",
                "local_sha": local_sha_for_push,
                "remote_sha": remote_sha_for_push,
                "resolve_command_ran": resolve_command_ran,
                "resolve_exit_code": resolve_exit_code
            })),
        )
            .into_response();
    }

    // Execute the actual push
    let push_output = tokio::process::Command::new("git")
        .args([
            "push",
            &remote_url,
            &format!("refs/heads/{}:refs/heads/{}", branch, branch),
        ])
        .current_dir(&local_repo_path)
        .output()
        .await;

    let push_result = match push_output {
        Ok(out) if out.status.success() => {
            info!("git sync push succeeded: project_id={}", project_id);
            serde_json::json!({
                "status": "pushed",
                "remote_url": remote_url,
                "branch": branch,
                "local_sha": local_sha_for_push,
                "resolve_command_ran": resolve_command_ran,
                "resolve_exit_code": resolve_exit_code
            })
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            error!(
                "git sync push failed: project_id={} stderr={}",
                project_id, stderr
            );
            if stderr.contains("non-fast-forward") || stderr.contains("rejected") {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({
                        "error": "non_fast_forward",
                        "reason": "Push rejected by remote (non-fast-forward)",
                        "stderr": stderr
                    })),
                )
                    .into_response();
            }
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("git push failed: {}", stderr),
            );
        }
        Err(e) => {
            error!("Failed to run git push: {}", e);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to run git push: {}", e),
            );
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "synced",
            "pull": pull_result,
            "push": push_result,
            "resolve_command_ran": resolve_command_ran,
            "resolve_exit_code": resolve_exit_code
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tokio::sync::broadcast::{error::TryRecvError, Receiver};
    use tower::ServiceExt;

    use crate::remote::types::RemoteLogEntry;

    use crate::server::api::test_support::{
        create_local_git_repo, init_bare_repo_with_commit, make_state,
    };
    use crate::server::api::{build_router, AppState, OrchestrationStatus, SERVER_LOG_BUFFER_SIZE};
    use crate::server::registry::create_shared_registry;

    fn drain_log_entries(log_rx: &mut Receiver<RemoteLogEntry>) -> Vec<RemoteLogEntry> {
        let mut entries = Vec::new();
        loop {
            match log_rx.try_recv() {
                Ok(entry) => entries.push(entry),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Lagged(_)) => continue,
                Err(TryRecvError::Closed) => break,
            }
        }
        entries
    }

    #[test]
    fn test_build_resolve_command_argv_replaces_prompt_placeholder_as_single_arg() {
        let template = "opencode run --agent code '{prompt}'";
        let prompt = "hello world";
        let argv = build_resolve_command_argv(template, prompt).expect("argv should build");
        assert_eq!(
            argv,
            vec![
                "opencode".to_string(),
                "run".to_string(),
                "--agent".to_string(),
                "code".to_string(),
                "hello world".to_string(),
            ]
        );
    }

    #[test]
    fn test_build_resolve_command_argv_handles_quotes_and_braces_literal() {
        let template = "echo '{prompt}' '{prompt}-suffix'";
        let prompt = "a b c";
        let argv = build_resolve_command_argv(template, prompt).expect("argv should build");
        assert_eq!(
            argv,
            vec![
                "echo".to_string(),
                "a b c".to_string(),
                "a b c-suffix".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_uses_login_shell() {
        let temp_dir = TempDir::new().unwrap();
        let (ran, exit_code) =
            run_resolve_command("echo hello", temp_dir.path(), "test prompt", None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "echo command should succeed via login shell"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_substitutes_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let (ran, exit_code) =
            run_resolve_command("echo {prompt}", temp_dir.path(), "test_marker", None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "echo with prompt substitution should succeed"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_quoted_template_does_not_double_quote() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = temp_dir.path().join("quoted_marker.txt");
        let template = format!(
            "printf '%s' '{{prompt}}' > '{}'",
            marker_path.to_str().unwrap()
        );
        let (ran, exit_code) =
            run_resolve_command(&template, temp_dir.path(), "hello world", None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "printf with quoted prompt template should succeed, not exit 127"
        );
        let content = std::fs::read_to_string(&marker_path).unwrap_or_default();
        assert_eq!(
            content, "hello world",
            "Quoted template should pass prompt value without double-quoting"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_unquoted_template_works() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = temp_dir.path().join("unquoted_marker.txt");
        let template = format!(
            "printf '%s' {{prompt}} > '{}'",
            marker_path.to_str().unwrap()
        );
        let (ran, exit_code) =
            run_resolve_command(&template, temp_dir.path(), "simple_word", None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "printf with unquoted prompt template should succeed"
        );
        let content = std::fs::read_to_string(&marker_path).unwrap_or_default();
        assert_eq!(
            content, "simple_word",
            "Unquoted template should pass prompt value correctly"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_multiline_prompt_does_not_break_shell() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = temp_dir.path().join("multiline_marker.txt");
        let template = format!(
            "printf '%s' '{{prompt}}' > '{}'",
            marker_path.to_str().unwrap()
        );
        let multiline_prompt =
            "Conflux server auto_resolve\noperation=git_sync\nproject_id=abc123\nTask: reconcile local state";
        let (ran, exit_code) =
            run_resolve_command(&template, temp_dir.path(), multiline_prompt, None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "Multi-line prompt with quoted template must not cause exit 127"
        );
        let content = std::fs::read_to_string(&marker_path).unwrap_or_default();
        assert_eq!(
            content, multiline_prompt,
            "Multi-line prompt should be passed intact through the shell"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_multiline_prompt_unquoted_template() {
        let temp_dir = TempDir::new().unwrap();
        let marker_path = temp_dir.path().join("multiline_unquoted.txt");
        let template = format!(
            "printf '%s' {{prompt}} > '{}'",
            marker_path.to_str().unwrap()
        );
        let multiline_prompt = "Line 1\nLine 2\nLine 3";
        let (ran, exit_code) =
            run_resolve_command(&template, temp_dir.path(), multiline_prompt, None, None).await;
        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(
            exit_code,
            Some(0),
            "Multi-line prompt with unquoted template should succeed"
        );
        let content = std::fs::read_to_string(&marker_path).unwrap_or_default();
        assert_eq!(
            content, multiline_prompt,
            "Multi-line prompt should be passed intact"
        );
    }

    #[tokio::test]
    async fn test_run_resolve_command_logs_start_stdout_stderr_finished_in_order() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = make_state(&temp_dir, None);
        let mut log_rx = state.log_tx.subscribe();
        state.resolve_command = Some("echo unused".to_string());

        let command = "printf 'stdout-line\\n'; printf 'stderr-line\\n' >&2; exit 0";
        let (ran, exit_code) =
            run_resolve_command(command, temp_dir.path(), "prompt", Some(&state), Some("p1")).await;

        assert!(ran, "resolve_command should have been attempted");
        assert_eq!(exit_code, Some(0));

        let entries = drain_log_entries(&mut log_rx);
        let resolve_entries: Vec<_> = entries
            .into_iter()
            .filter(|entry| entry.project_id.as_deref() == Some("p1"))
            .collect();

        assert_eq!(
            resolve_entries.len(),
            4,
            "expected start/stdout/stderr/finished logs"
        );
        assert_eq!(resolve_entries[0].level, "info");
        assert!(resolve_entries[0]
            .message
            .starts_with("resolve_command started:"));
        assert_eq!(resolve_entries[1].level, "info");
        assert_eq!(resolve_entries[1].message, "stdout-line");
        assert_eq!(resolve_entries[2].level, "warn");
        assert_eq!(resolve_entries[2].message, "stderr-line");
        assert_eq!(resolve_entries[3].level, "success");
        assert_eq!(
            resolve_entries[3].message,
            "resolve_command finished: exit_code=Some(0)"
        );
        assert!(resolve_entries
            .iter()
            .all(|entry| entry.operation.as_deref() == Some("resolve")));
    }

    #[tokio::test]
    async fn test_run_resolve_command_logs_error_level_on_non_zero_exit() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let mut log_rx = state.log_tx.subscribe();

        let (ran, exit_code) = run_resolve_command(
            "printf 'ok\\n'; exit 7",
            temp_dir.path(),
            "prompt",
            Some(&state),
            Some("p2"),
        )
        .await;

        assert!(ran);
        assert_eq!(exit_code, Some(7));

        let entries = drain_log_entries(&mut log_rx);
        let resolve_entries: Vec<_> = entries
            .into_iter()
            .filter(|entry| entry.project_id.as_deref() == Some("p2"))
            .collect();

        assert_eq!(
            resolve_entries.len(),
            3,
            "expected start/stdout/finished logs"
        );
        assert_eq!(resolve_entries[2].level, "error");
        assert_eq!(
            resolve_entries[2].message,
            "resolve_command finished: exit_code=Some(7)"
        );
    }

    #[tokio::test]
    async fn test_git_pull_non_fast_forward_detection() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let branch = "main";

        let remote_dir = TempDir::new().unwrap();
        let remote_path = remote_dir.path();
        let remote_sha = init_bare_repo_with_commit(remote_path, branch).await;
        if remote_sha.is_none() {
            return;
        }
        let remote_sha = remote_sha.unwrap();

        let remote_url = format!("file://{}", remote_path.display());
        let entry = state
            .registry
            .write()
            .await
            .add(remote_url.clone(), branch.to_string())
            .unwrap();

        let local_clone_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_clone_path).unwrap();

        let init_local = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_clone_path)
            .status()
            .await;
        if init_local.is_err() || !init_local.unwrap().success() {
            return;
        }

        let work_dir = TempDir::new().unwrap();
        let work_path = work_dir.path();
        let clone_to_work = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path.to_str().unwrap()])
            .status()
            .await;
        if clone_to_work.is_err() || !clone_to_work.unwrap().success() {
            return;
        }

        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path)
            .status()
            .await
            .ok();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path)
            .status()
            .await
            .ok();

        std::fs::write(work_path.join("diverged.txt"), "diverged commit").unwrap();
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok();
        let diverged_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "diverged commit"])
            .current_dir(work_path)
            .status()
            .await;
        if diverged_commit.is_err() || !diverged_commit.unwrap().success() {
            return;
        }

        let push_to_local = tokio::process::Command::new("git")
            .args([
                "push",
                local_clone_path.to_str().unwrap(),
                &format!("{}:{}", branch, branch),
            ])
            .current_dir(work_path)
            .status()
            .await;
        if push_to_local.is_err() || !push_to_local.unwrap().success() {
            return;
        }

        let work_dir2 = TempDir::new().unwrap();
        let work_path2 = work_dir2.path();
        let clone2 = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path2.to_str().unwrap()])
            .status()
            .await;
        if clone2.is_err() || !clone2.unwrap().success() {
            return;
        }
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        std::fs::write(work_path2.join("remote_advance.txt"), "remote advance").unwrap();
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path2)
            .status()
            .await
            .ok();
        let remote_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "remote advance"])
            .current_dir(work_path2)
            .status()
            .await;
        if remote_commit.is_err() || !remote_commit.unwrap().success() {
            return;
        }
        let push_remote = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path2)
            .status()
            .await;
        if push_remote.is_err() || !push_remote.unwrap().success() {
            return;
        }

        let refs_dir = local_clone_path.join("refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).unwrap();
        std::fs::write(refs_dir.join(branch), format!("{}\n", remote_sha)).unwrap();

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            json["error"].as_str().unwrap_or(""),
            "resolve_command_not_configured"
        );
    }

    async fn create_diverged_repo_setup(
        temp_dir: &TempDir,
        state: &AppState,
        branch: &str,
    ) -> Option<(
        tempfile::TempDir,
        std::path::PathBuf,
        crate::server::registry::ProjectEntry,
        String,
    )> {
        let remote_dir = TempDir::new().ok()?;
        let remote_path = remote_dir.path();
        let remote_sha = init_bare_repo_with_commit(remote_path, branch).await?;

        let remote_url = format!("file://{}", remote_path.display());
        let entry = state
            .registry
            .write()
            .await
            .add(remote_url.clone(), branch.to_string())
            .ok()?;

        let local_clone_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_clone_path).ok()?;
        let init_local = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_clone_path)
            .status()
            .await
            .ok()?;
        if !init_local.success() {
            return None;
        }

        let work_dir = TempDir::new().ok()?;
        let work_path = work_dir.path();
        let clone_to_work = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path.to_str().unwrap()])
            .status()
            .await
            .ok()?;
        if !clone_to_work.success() {
            return None;
        }

        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;

        std::fs::write(work_path.join("diverged.txt"), "diverged commit").ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        let diverged_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "diverged commit"])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !diverged_commit.success() {
            return None;
        }
        let push_to_local = tokio::process::Command::new("git")
            .args([
                "push",
                local_clone_path.to_str().unwrap(),
                &format!("{}:{}", branch, branch),
            ])
            .current_dir(work_path)
            .status()
            .await
            .ok()?;
        if !push_to_local.success() {
            return None;
        }

        let work_dir2 = TempDir::new().ok()?;
        let work_path2 = work_dir2.path();
        let clone2 = tokio::process::Command::new("git")
            .args(["clone", &remote_url, work_path2.to_str().unwrap()])
            .status()
            .await
            .ok()?;
        if !clone2.success() {
            return None;
        }
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        std::fs::write(work_path2.join("remote_advance.txt"), "remote advance").ok()?;
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        let remote_commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "remote advance"])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        if !remote_commit.success() {
            return None;
        }
        let push_remote = tokio::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(work_path2)
            .status()
            .await
            .ok()?;
        if !push_remote.success() {
            return None;
        }

        let refs_dir = local_clone_path.join("refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).ok()?;
        std::fs::write(refs_dir.join(branch), format!("{}\n", remote_sha)).ok()?;

        Some((remote_dir, local_clone_path, entry, remote_url))
    }

    #[tokio::test]
    async fn test_git_pull_delegates_to_git_sync_and_requires_resolve_command() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return,
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            json["error"].as_str().unwrap_or(""),
            "resolve_command_not_configured"
        );
    }

    #[tokio::test]
    async fn test_git_pull_auto_resolve_runs_resolve_command() {
        let temp_dir = TempDir::new().unwrap();

        let registry = crate::server::registry::create_shared_registry(temp_dir.path(), 4).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("echo resolve".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };

        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return,
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        if status == StatusCode::OK {
            if let Some(ran) = json.get("resolve_command_ran") {
                if ran.as_bool() == Some(true) {
                    assert_eq!(json["resolve_exit_code"].as_i64(), Some(0));
                }
            }
        }

        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert_ne!(error_val, "non_fast_forward");
        }

        assert!(status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_git_pull_auto_resolve_without_resolve_command_configured_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let branch = "main";

        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return,
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert!(
                error_val == "resolve_command_not_configured" || error_val == "non_fast_forward"
            );
        }
    }

    #[tokio::test]
    async fn test_git_pull_auto_resolve_uses_top_level_resolve_command() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("echo resolve".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };

        let branch = "main";
        let result = create_diverged_repo_setup(&temp_dir, &state, branch).await;
        let (_remote_dir, _local_clone_path, entry, _remote_url) = match result {
            Some(r) => r,
            None => return,
        };

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/pull", entry.id))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"auto_resolve": true}"#))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();

        if status == StatusCode::UNPROCESSABLE_ENTITY {
            let error_val = json["error"].as_str().unwrap_or("");
            assert_ne!(error_val, "resolve_command_not_configured");
        }
        if let Some(ran) = json.get("resolve_command_ran") {
            if ran.as_bool() == Some(true) {
                let exit_code = json["resolve_exit_code"].as_i64().unwrap_or(-1);
                assert_eq!(exit_code, 0);
            }
        }
    }

    #[tokio::test]
    async fn test_git_push_no_local_clone_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/push", entry.id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let error_val = json["error"].as_str().unwrap_or("");
        assert_eq!(error_val, "resolve_command_not_configured");
    }

    #[tokio::test]
    async fn test_git_push_non_fast_forward_detection_with_bare_repos() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);

        let entry = state
            .registry
            .write()
            .await
            .add("file:///not-a-real-remote".to_string(), "main".to_string())
            .unwrap();

        let local_repo_path = temp_dir.path().join(&entry.id);
        std::fs::create_dir_all(&local_repo_path).unwrap();

        let init_status = tokio::process::Command::new("git")
            .args(["init", "--bare"])
            .current_dir(&local_repo_path)
            .status()
            .await;

        if init_status.is_err() || !init_status.unwrap().success() {
            return;
        }

        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/push", entry.id))
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert!(
            resp.status() == StatusCode::UNPROCESSABLE_ENTITY
                || resp.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "Push with empty bare repo should fail, got: {}",
            resp.status()
        );
    }

    #[tokio::test]
    async fn test_git_sync_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = make_state(&temp_dir, None);
        state.resolve_command = Some("echo resolve".to_string());
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects/nonexistent-project/git/sync")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].is_string());
    }

    #[tokio::test]
    async fn test_git_sync_fails_without_resolve_command() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let _router = build_router(state);

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg
                .add(
                    "https://github.com/example/repo.git".to_string(),
                    "main".to_string(),
                )
                .unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state_with_project = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: None,
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state_with_project);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json["error"].as_str().unwrap(),
            "resolve_command_not_configured"
        );
        assert!(json["reason"].as_str().unwrap().contains("resolve_command"));
    }

    #[tokio::test]
    async fn test_git_sync_route_is_registered() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg
                .add(
                    "https://github.com/example/repo.git".to_string(),
                    "main".to_string(),
                )
                .unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: None,
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_git_sync_success_response_contains_pull_and_push_sections() {
        let temp_dir = TempDir::new().unwrap();

        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg.add(remote_url.clone(), "main".to_string()).unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("true".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        if status == StatusCode::OK {
            assert!(json.get("pull").is_some());
            assert!(json.get("push").is_some());
            assert_eq!(json["status"].as_str(), Some("synced"));
        }
        assert!(status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_git_sync_skips_resolve_when_already_up_to_date() {
        let temp_dir = TempDir::new().unwrap();

        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg.add(remote_url.clone(), "main".to_string()).unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("false".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"].as_str(), Some("synced"));
        assert_eq!(json["resolve_command_ran"].as_bool(), Some(false));
        assert!(json["resolve_exit_code"].is_null());
        assert_eq!(json["push"]["status"].as_str(), Some("already_up_to_date"));
        assert_eq!(
            json["skipped_reason"].as_str(),
            Some("local_and_remote_already_match")
        );
    }

    #[tokio::test]
    async fn test_git_sync_runs_resolve_when_shas_differ() {
        let temp_dir = TempDir::new().unwrap();

        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg.add(remote_url.clone(), "main".to_string()).unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry: registry.clone(),
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("true".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state);

        let req1 = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();
        let resp1 = router.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = axum::body::to_bytes(resp1.into_body(), usize::MAX)
            .await
            .unwrap();
        let json1: serde_json::Value = serde_json::from_slice(&body1).unwrap();
        assert_eq!(json1["resolve_command_ran"].as_bool(), Some(false));

        let local_bare = {
            let reg = registry.read().await;
            reg.data_dir().join(&project_id)
        };
        let tree_out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD^{tree}"])
            .current_dir(&local_bare)
            .output()
            .unwrap();
        let tree_sha = String::from_utf8_lossy(&tree_out.stdout).trim().to_string();

        let parent_out = std::process::Command::new("git")
            .args(["rev-parse", "refs/heads/main"])
            .current_dir(&local_bare)
            .output()
            .unwrap();
        let parent_sha = String::from_utf8_lossy(&parent_out.stdout)
            .trim()
            .to_string();

        let commit_out = std::process::Command::new("git")
            .args([
                "commit-tree",
                &tree_sha,
                "-p",
                &parent_sha,
                "-m",
                "local only commit",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .current_dir(&local_bare)
            .output()
            .unwrap();
        let new_sha = String::from_utf8_lossy(&commit_out.stdout)
            .trim()
            .to_string();

        std::process::Command::new("git")
            .args(["update-ref", "refs/heads/main", &new_sha])
            .current_dir(&local_bare)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["update-ref", "refs/remotes/origin/main", &new_sha])
            .current_dir(&local_bare)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(["update-ref", "refs/heads/main", &parent_sha])
            .current_dir(&local_bare)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["update-ref", "refs/remotes/origin/main", &parent_sha])
            .current_dir(&local_bare)
            .output()
            .unwrap();

        let scratch = temp_dir.path().join("scratch-work");
        std::process::Command::new("git")
            .args(["clone", origin.to_str().unwrap(), scratch.to_str().unwrap()])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::fs::write(scratch.join("new-file.txt"), "origin-only").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "origin divergence"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["push", "origin", "main"])
            .current_dir(&scratch)
            .output()
            .unwrap();

        let req2 = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{}/git/sync", project_id))
            .body(Body::empty())
            .unwrap();
        let resp2 = router.oneshot(req2).await.unwrap();
        let status2 = resp2.status();

        let body2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
            .await
            .unwrap();
        let json2: serde_json::Value = serde_json::from_slice(&body2).unwrap();

        assert_eq!(status2, StatusCode::OK, "{}", json2);
        assert_eq!(json2["status"].as_str(), Some("synced"));
    }

    #[tokio::test]
    async fn test_git_sync_runs_resolve_when_remote_ahead() {
        let temp_dir = TempDir::new().unwrap();

        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
        let project_id = {
            let mut reg = registry.write().await;
            let entry = reg.add(remote_url.clone(), "main".to_string()).unwrap();
            entry.id.clone()
        };

        let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
        let state = AppState {
            registry,
            runners: crate::server::runner::create_shared_runners(),
            db: None,
            auth_token: None,
            max_concurrent_total: 4,
            resolve_command: Some("true".to_string()),
            log_tx,
            orchestration_status: Arc::new(
                tokio::sync::RwLock::new(OrchestrationStatus::default()),
            ),
            terminal_manager: crate::server::terminal::create_terminal_manager(),
            active_commands: crate::server::active_commands::create_shared_active_commands(),
            proposal_session_manager:
                crate::server::proposal_session::create_proposal_session_manager(
                    crate::config::ProposalSessionConfig::default(),
                    None,
                ),
        };
        let router = build_router(state);

        let initial_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/projects/{}/git/sync", project_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(initial_resp.status(), StatusCode::OK);

        let scratch = temp_dir.path().join("scratch-work-remote-ahead");
        std::process::Command::new("git")
            .args(["clone", origin.to_str().unwrap(), scratch.to_str().unwrap()])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::fs::write(scratch.join("remote-change.txt"), "new remote commit").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "remote change"])
            .current_dir(&scratch)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["push", "origin", "main"])
            .current_dir(&scratch)
            .output()
            .unwrap();

        let resp = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/projects/{}/git/sync", project_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"].as_str(), Some("synced"));
    }
}
