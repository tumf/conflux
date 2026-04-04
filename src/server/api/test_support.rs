use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    fn run_git(args: &[&str], current_dir: &Path) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(current_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git command failed: git {}\nstdout: {}\nstderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let unique = format!(
        "{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let repo_path = parent.join(format!("test-origin-{unique}"));
    let src = parent.join(format!("test-src-{unique}"));
    std::fs::create_dir_all(&src).unwrap();

    run_git(&["init", "-b", "main"], &src);
    run_git(&["config", "user.email", "test@example.com"], &src);
    run_git(&["config", "user.name", "Test"], &src);

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
    run_git(&["add", "."], &src);
    run_git(&["commit", "-m", "init"], &src);
    run_git(
        &[
            "clone",
            "--bare",
            src.to_str().unwrap(),
            repo_path.to_str().unwrap(),
        ],
        parent,
    );

    repo_path
}

pub(super) fn create_local_git_repo(parent: &Path) -> PathBuf {
    create_local_git_repo_with_setup(parent, None)
}
