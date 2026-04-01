use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use futures_util::StreamExt;
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

use conflux::config::ProposalSessionConfig;
use conflux::remote::types::RemoteLogEntry;
use conflux::server::active_commands::create_shared_active_commands;
use conflux::server::api::{build_router, AppState, SERVER_LOG_BUFFER_SIZE};
use conflux::server::proposal_session::create_proposal_session_manager;
use conflux::server::registry::{create_shared_registry, OrchestrationStatus};
use conflux::server::runner::create_shared_runners;
use conflux::server::terminal::create_terminal_manager;

fn create_mock_acp_path(repo_root: &Path) -> PathBuf {
    repo_root.join("tests/fixtures/mock_acp_agent.py")
}

fn proposal_worktree_path(base_dir: &Path, project_id: &str, session_id: &str) -> PathBuf {
    base_dir
        .join("worktrees")
        .join(project_id)
        .join(format!("proposal-{session_id}"))
}

fn create_local_git_repo(parent: &Path) -> PathBuf {
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
    std::fs::write(src.join("README.md"), "hello\n").unwrap();
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

async fn create_project(router: axum::Router, remote_url: String) -> (axum::Router, String) {
    let body = serde_json::json!({
        "remote_url": remote_url,
        "branch": "main"
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/projects")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let project_id = json["id"].as_str().unwrap().to_string();
    (router, project_id)
}

async fn next_non_heartbeat_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    use futures_util::StreamExt;

    loop {
        let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let json: Value = serde_json::from_str(&message).unwrap();
        if json["type"] != "heartbeat" {
            return json;
        }
    }
}

fn make_state_with_transport_env(
    temp_dir: &TempDir,
    transport_env: std::collections::HashMap<String, String>,
) -> AppState {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proposal_config = ProposalSessionConfig {
        transport_command: "python3".to_string(),
        transport_args: vec![create_mock_acp_path(&repo_root).display().to_string()],
        transport_env,
        session_inactivity_timeout_secs: 1,
    };

    let registry = create_shared_registry(temp_dir.path(), 4).unwrap();
    let (log_tx, _) = broadcast::channel::<RemoteLogEntry>(SERVER_LOG_BUFFER_SIZE);

    AppState {
        registry,
        runners: create_shared_runners(),
        db: None,
        auth_token: None,
        max_concurrent_total: 4,
        resolve_command: None,
        log_tx,
        orchestration_status: Arc::new(tokio::sync::RwLock::new(OrchestrationStatus::default())),
        terminal_manager: create_terminal_manager(),
        active_commands: create_shared_active_commands(),
        proposal_session_manager: create_proposal_session_manager(proposal_config, None),
    }
}

fn make_state(temp_dir: &TempDir) -> AppState {
    make_state_with_transport_env(temp_dir, std::collections::HashMap::new())
}

#[tokio::test]
async fn proposal_session_create_and_list_use_frontend_contract_shape() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (router, project_id) = create_project(router, remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    assert_eq!(created["status"], "active");
    assert_eq!(created["is_dirty"], false);
    assert!(created["uncommitted_files"].as_array().is_some());
    assert!(created["updated_at"].as_str().is_some());
    assert!(created.get("worktree_path").is_none());

    let session_id = created["id"].as_str().unwrap().to_string();

    let list_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let list_resp = router.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let sessions: Value = serde_json::from_slice(&list_body).unwrap();
    let first = sessions.as_array().unwrap().first().unwrap();
    assert_eq!(first["id"], session_id);
    assert_eq!(first["status"], "active");
    assert!(first["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn proposal_session_prompt_injects_backend_managed_spec_guidance() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());

    let prompt_dump_path = temp_dir.path().join("mock-acp-prompt.json");
    let mut transport_env = std::collections::HashMap::new();
    transport_env.insert(
        "MOCK_ACP_PROMPT_DUMP_OUT".to_string(),
        prompt_dump_path.display().to_string(),
    );

    let state = make_state_with_transport_env(&temp_dir, transport_env);
    let router = build_router(state.clone());

    let (_router, project_id) = create_project(router.clone(), remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, serve_router).await.unwrap();
    });

    use futures_util::{SinkExt, StreamExt};

    let ws_url = format!("ws://{addr}/api/v1/proposal-sessions/{session_id}/ws");
    let (mut socket, _) = connect_async(ws_url).await.unwrap();

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "prompt",
                "content": "spec guidance check"
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let _ = socket.next().await.unwrap().unwrap();
    let _ = socket.next().await.unwrap().unwrap();

    let prompt_dump_raw = fs::read_to_string(&prompt_dump_path).unwrap();
    let prompt_dump_json: Value = serde_json::from_str(&prompt_dump_raw).unwrap();
    let prompt_blocks = prompt_dump_json.as_array().unwrap();

    assert!(
        prompt_blocks.len() >= 2,
        "backend should prepend dedicated guidance before user prompt"
    );

    let system_text = prompt_blocks[0]["text"].as_str().unwrap_or_default();
    assert!(
        system_text.contains("specification-focused assistant"),
        "first prompt block should contain dedicated spec-focused guidance"
    );
    assert!(
        system_text.contains("Do not implement production code"),
        "guidance should include implementation-boundary instruction"
    );

    let user_text = prompt_blocks.last().unwrap()["text"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(user_text, "spec guidance check");

    server_task.abort();
}

#[tokio::test]
async fn proposal_session_create_does_not_inject_default_opencode_config_env() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let mut transport_env = std::collections::HashMap::new();
    transport_env.insert(
        "MOCK_ACP_OPENCODE_CONFIG_OUT".to_string(),
        "mock-acp.out".to_string(),
    );
    let state = make_state_with_transport_env(&temp_dir, transport_env);
    let router = build_router(state.clone());

    let (router, project_id) = create_project(router, remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap();

    let worktree_path = proposal_worktree_path(temp_dir.path(), &project_id, session_id);
    let default_opencode_config = temp_dir.path().join("opencode-proposal.jsonc");

    assert!(!default_opencode_config.exists());
    assert!(!worktree_path.join("mock-acp.out").exists());
}

#[tokio::test]
async fn proposal_session_ws_accepts_frontend_message_aliases() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (_router, project_id) = create_project(router.clone(), remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, serve_router).await.unwrap();
    });

    let ws_url = format!("ws://{addr}/api/v1/proposal-sessions/{session_id}/ws");
    let (mut socket, _) = connect_async(ws_url).await.unwrap();

    use futures_util::SinkExt;

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "prompt",
                "content": "alias-check",
                "client_message_id": "client-1"
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let user_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(user_json["type"], "user_message");
    assert_eq!(user_json["content"], "alias-check");
    assert_eq!(user_json["client_message_id"], "client-1");

    let thought_chunk_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(thought_chunk_json["type"], "agent_thought_chunk");
    assert_eq!(thought_chunk_json["text"], "echo:alias-check");

    let turn_complete_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(turn_complete_json["type"], "turn_complete");
    assert_eq!(turn_complete_json["stop_reason"], "end_turn");

    server_task.abort();
}

#[tokio::test]
async fn proposal_session_close_reports_dirty_worktree_files() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (router, project_id) = create_project(router, remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let worktree_path = proposal_worktree_path(temp_dir.path(), &project_id, &session_id);
    fs::write(worktree_path.join("dirty.txt"), "dirty\n").unwrap();

    let close_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!(
            "/api/v1/projects/{project_id}/proposal-sessions/{session_id}"
        ))
        .header("Content-Type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let close_resp = router.clone().oneshot(close_req).await.unwrap();
    assert_eq!(close_resp.status(), StatusCode::CONFLICT);
    let close_body = axum::body::to_bytes(close_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let close_json: Value = serde_json::from_slice(&close_body).unwrap();
    assert_eq!(close_json["status"], "dirty");
    assert!(close_json["uncommitted_files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("dirty.txt")));

    let force_close_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!(
            "/api/v1/projects/{project_id}/proposal-sessions/{session_id}"
        ))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"force":true}"#))
        .unwrap();
    let force_close_resp = router.clone().oneshot(force_close_req).await.unwrap();
    assert_eq!(force_close_resp.status(), StatusCode::OK);
    assert!(!proposal_worktree_path(temp_dir.path(), &project_id, &session_id).exists());
}

#[tokio::test]
async fn proposal_session_timeout_marks_timed_out_status() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (router, project_id) = create_project(router, remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    state
        .proposal_session_manager
        .write()
        .await
        .scan_timeouts()
        .await;

    let list_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let list_resp = router.clone().oneshot(list_req).await.unwrap();
    let list_body = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let sessions: Value = serde_json::from_slice(&list_body).unwrap();
    let timed_out = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["id"] == session_id)
        .unwrap();
    assert_eq!(timed_out["status"], "timed_out");
}

#[tokio::test]
async fn proposal_session_merge_merges_branch_and_removes_worktree() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (router, project_id) = create_project(router, remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let worktree_path = proposal_worktree_path(temp_dir.path(), &project_id, &session_id);
    let change_dir = worktree_path
        .join("openspec")
        .join("changes")
        .join("merge-check");
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("proposal.md"), "# Change: Merge Check\n").unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "proposal session change"])
        .current_dir(&worktree_path)
        .output()
        .unwrap();

    let changes_req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/projects/{project_id}/proposal-sessions/{session_id}/changes"
        ))
        .body(Body::empty())
        .unwrap();
    let changes_resp = router.clone().oneshot(changes_req).await.unwrap();
    assert_eq!(changes_resp.status(), StatusCode::OK);
    let changes_body = axum::body::to_bytes(changes_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let changes_json: Value = serde_json::from_slice(&changes_body).unwrap();
    let detected_ids: Vec<&str> = changes_json
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert!(detected_ids.contains(&"merge-check"));

    let merge_req = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/api/v1/projects/{project_id}/proposal-sessions/{session_id}/merge"
        ))
        .body(Body::empty())
        .unwrap();
    let merge_resp = router.clone().oneshot(merge_req).await.unwrap();
    assert_eq!(merge_resp.status(), StatusCode::OK);
    assert!(!proposal_worktree_path(temp_dir.path(), &project_id, &session_id).exists());

    let base_worktree = temp_dir
        .path()
        .join("worktrees")
        .join(&project_id)
        .join("main");
    assert!(
        base_worktree
            .join("openspec")
            .join("changes")
            .join("merge-check")
            .join("proposal.md")
            .exists(),
        "merged proposal change should exist in base worktree"
    );
}

#[tokio::test]
async fn proposal_session_ws_cancel_and_reconnect_history_work() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (_router, project_id) = create_project(router.clone(), remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, serve_router).await.unwrap();
    });

    use futures_util::SinkExt;

    let ws_url = format!("ws://{addr}/api/v1/proposal-sessions/{session_id}/ws");
    let (mut socket, _) = connect_async(ws_url.clone()).await.unwrap();

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "prompt",
                "content": "history-check",
                "client_message_id": "client-history"
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let user_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(user_json["type"], "user_message");
    assert_eq!(user_json["content"], "history-check");
    assert_eq!(user_json["client_message_id"], "client-history");

    let thought_chunk_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(thought_chunk_json["type"], "agent_thought_chunk");
    assert_eq!(thought_chunk_json["text"], "echo:history-check");

    let turn_complete_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(turn_complete_json["type"], "turn_complete");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "cancel"}).to_string(),
        ))
        .await
        .unwrap();

    let cancelled_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(cancelled_json["type"], "turn_complete");
    assert_eq!(cancelled_json["stop_reason"], "cancelled");

    drop(socket);

    let (mut reconnect_socket, _) = connect_async(ws_url).await.unwrap();
    let replay_user_json = next_non_heartbeat_json(&mut reconnect_socket).await;
    assert_eq!(replay_user_json["type"], "user_message");
    assert_eq!(replay_user_json["content"], "history-check");
    assert_eq!(replay_user_json["client_message_id"], "client-history");

    let replay_json = next_non_heartbeat_json(&mut reconnect_socket).await;
    assert_eq!(replay_json["type"], "agent_thought_chunk");
    assert_eq!(replay_json["text"], "echo:history-check");

    let replay_turn_complete_json = next_non_heartbeat_json(&mut reconnect_socket).await;
    assert_eq!(replay_turn_complete_json["type"], "turn_complete");

    let replay_recovery_state_json = next_non_heartbeat_json(&mut reconnect_socket).await;
    assert_eq!(replay_recovery_state_json["type"], "recovery_state");
    assert_eq!(replay_recovery_state_json["active"], false);

    server_task.abort();
}

#[tokio::test]
async fn proposal_session_reconnect_does_not_duplicate_acknowledged_prompt() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (_router, project_id) = create_project(router.clone(), remote_url).await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
        .body(Body::empty())
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    let create_body = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, serve_router).await.unwrap();
    });

    use futures_util::SinkExt;

    let ws_url = format!("ws://{addr}/api/v1/proposal-sessions/{session_id}/ws");
    let (mut socket, _) = connect_async(ws_url.clone()).await.unwrap();

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "prompt",
                "content": "dedupe-check",
                "client_message_id": "client-dedupe"
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let ack_json = next_non_heartbeat_json(&mut socket).await;
    assert_eq!(ack_json["type"], "user_message");
    assert_eq!(ack_json["client_message_id"], "client-dedupe");

    drop(socket);

    let (mut reconnect_socket, _) = connect_async(ws_url).await.unwrap();
    let replay_user_json = next_non_heartbeat_json(&mut reconnect_socket).await;
    assert_eq!(replay_user_json["type"], "user_message");
    assert_eq!(replay_user_json["client_message_id"], "client-dedupe");

    loop {
        let replay_json = next_non_heartbeat_json(&mut reconnect_socket).await;
        if replay_json["type"] == "recovery_state" {
            break;
        }
    }

    reconnect_socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "prompt",
                "content": "dedupe-check",
                "client_message_id": "client-dedupe"
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let maybe_duplicate = tokio::time::timeout(
        std::time::Duration::from_millis(300),
        reconnect_socket.next(),
    )
    .await;
    if let Ok(Some(Ok(message))) = maybe_duplicate {
        let payload = message.into_text().unwrap();
        let payload_json: Value = serde_json::from_str(&payload).unwrap();
        assert_ne!(
            payload_json["type"], "user_message",
            "duplicate prompt must not generate a second user_message"
        );
    }

    server_task.abort();
}

#[tokio::test]
async fn proposal_session_multi_session_websockets_stay_independent() {
    let temp_dir = TempDir::new().unwrap();
    let origin = create_local_git_repo(temp_dir.path());
    let remote_url = format!("file://{}", origin.to_string_lossy());
    let state = make_state(&temp_dir);
    let router = build_router(state.clone());

    let (_router, project_id) = create_project(router.clone(), remote_url).await;

    let create_session = |router: axum::Router, project_id: String| async move {
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/projects/{project_id}/proposal-sessions"))
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        json["id"].as_str().unwrap().to_string()
    };

    let session_a = create_session(router.clone(), project_id.clone()).await;
    let session_b = create_session(router.clone(), project_id.clone()).await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, serve_router).await.unwrap();
    });

    let (mut socket_a, _) = connect_async(format!(
        "ws://{addr}/api/v1/proposal-sessions/{session_a}/ws"
    ))
    .await
    .unwrap();
    let (mut socket_b, _) = connect_async(format!(
        "ws://{addr}/api/v1/proposal-sessions/{session_b}/ws"
    ))
    .await
    .unwrap();

    use futures_util::SinkExt;

    socket_a
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "prompt", "content": "alpha"}).to_string(),
        ))
        .await
        .unwrap();
    socket_b
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "prompt", "content": "beta"}).to_string(),
        ))
        .await
        .unwrap();

    let user_json_a = next_non_heartbeat_json(&mut socket_a).await;
    let user_json_b = next_non_heartbeat_json(&mut socket_b).await;
    let json_a = next_non_heartbeat_json(&mut socket_a).await;
    let json_b = next_non_heartbeat_json(&mut socket_b).await;
    assert_eq!(user_json_a["type"], "user_message");
    assert_eq!(user_json_b["type"], "user_message");
    assert_eq!(user_json_a["content"], "alpha");
    assert_eq!(user_json_b["content"], "beta");
    assert!(user_json_a.get("client_message_id").is_none());
    assert!(user_json_b.get("client_message_id").is_none());
    assert_eq!(json_a["type"], "agent_thought_chunk");
    assert_eq!(json_b["type"], "agent_thought_chunk");
    assert_eq!(json_a["text"], "echo:alpha");
    assert_eq!(json_b["text"], "echo:beta");

    server_task.abort();
}
