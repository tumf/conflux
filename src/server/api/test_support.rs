use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use tempfile::TempDir;

use crate::server::api::{
    build_router, refresh_project_sync_states_once, AppState, SERVER_LOG_BUFFER_SIZE,
};
use crate::server::registry::{create_shared_registry, OrchestrationStatus};

pub(super) fn make_state(temp_dir: &TempDir, auth_token: Option<&str>) -> AppState {
    let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
    let (log_tx, _) = tokio::sync::broadcast::channel(SERVER_LOG_BUFFER_SIZE);
    AppState {
        registry,
        runners: crate::server::runner::create_shared_runners(),
        db: None,
        auth_token: auth_token.map(|s| s.to_string()),
        max_concurrent_total: 4,
        resolve_command: None,
        log_tx,
        orchestration_status: Arc::new(tokio::sync::RwLock::new(OrchestrationStatus::default())),
        terminal_manager: crate::server::terminal::create_terminal_manager(),
        active_commands: crate::server::active_commands::create_shared_active_commands(),
        proposal_session_manager: crate::server::proposal_session::create_proposal_session_manager(
            crate::config::ProposalSessionConfig::default(),
            None,
        ),
    }
}

pub(super) fn make_router(temp_dir: &TempDir, auth_token: Option<&str>) -> Router {
    build_router(make_state(temp_dir, auth_token))
}

pub(super) async fn run_sync_monitor_once_for_tests(state: &AppState) {
    refresh_project_sync_states_once(&state.registry).await;
}

pub(super) fn create_local_git_repo_with_setup(
    parent: &Path,
    setup_script: Option<&str>,
) -> PathBuf {
    let repo_path = parent.join("test-origin");
    let src = parent.join("test-src");
    std::fs::create_dir_all(&src).unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&src)
        .output()
        .unwrap();

    if let Some(script) = setup_script {
        let wt_dir = src.join(".wt");
        std::fs::create_dir_all(&wt_dir).unwrap();
        let setup_path = wt_dir.join("setup");
        std::fs::write(&setup_path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&setup_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&setup_path, perms).unwrap();
        }
    }

    std::fs::write(src.join("README.md"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&src)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "clone",
            "--bare",
            src.to_str().unwrap(),
            repo_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    repo_path
}

pub(super) fn create_local_git_repo(parent: &Path) -> PathBuf {
    create_local_git_repo_with_setup(parent, None)
}
