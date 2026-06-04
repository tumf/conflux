use super::*;

pub(super) fn emit_log_entry(state: &AppState, entry: RemoteLogEntry) {
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

pub(super) fn emit_resolve_log(state: &AppState, project_id: &str, level: &str, message: String) {
    emit_log_entry(state, build_resolve_log_entry(project_id, level, message));
}

pub(super) fn build_auto_resolve_prompt(
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
