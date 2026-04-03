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
        emit_log_entry(
            state,
            RemoteLogEntry {
                message: format!("resolve_command started: {}", command_str),
                level: "info".to_string(),
                change_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                project_id: Some(pid.to_string()),
                operation: Some("resolve".to_string()),
                iteration: None,
            },
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
                emit_log_entry(
                    state,
                    RemoteLogEntry {
                        message: format!("resolve_command failed to start: {}", e),
                        level: "error".to_string(),
                        change_id: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        project_id: Some(pid.to_string()),
                        operation: Some("resolve".to_string()),
                        iteration: None,
                    },
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
                emit_log_entry(
                    state,
                    RemoteLogEntry {
                        message: line.to_string(),
                        level: "info".to_string(),
                        change_id: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        project_id: Some(pid.to_string()),
                        operation: Some("resolve".to_string()),
                        iteration: None,
                    },
                );
            }
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if !line.is_empty() {
                emit_log_entry(
                    state,
                    RemoteLogEntry {
                        message: line.to_string(),
                        level: "warn".to_string(),
                        change_id: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        project_id: Some(pid.to_string()),
                        operation: Some("resolve".to_string()),
                        iteration: None,
                    },
                );
            }
        }

        let exit_code = output.status.code();
        let level = if output.status.success() {
            "success"
        } else {
            "error"
        };
        emit_log_entry(
            state,
            RemoteLogEntry {
                message: format!("resolve_command finished: exit_code={:?}", exit_code),
                level: level.to_string(),
                change_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                project_id: Some(pid.to_string()),
                operation: Some("resolve".to_string()),
                iteration: None,
            },
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
    use tempfile::TempDir;

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
}
