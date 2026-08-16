//! End-to-end tests for `cflx client mcp` against a live, long-lived owner.
//!
//! # Why this is heavy
//!
//! Every test here spawns the **compiled binary** as a real stdio MCP server,
//! against a **real owner** with real listeners, and proves completion from a
//! **real Git repository**. That is three process boundaries and a filesystem,
//! which is exactly what the contract is about: an agent runs `cflx client mcp`,
//! an MCP host speaks JSON-RPC to it, and a callback fires while the TUI is
//! still running. None of that is observable from an in-process double, and none
//! of it belongs in the default suite's speed budget — so it lives behind
//! `heavy-tests`.
//!
//! # The property that matters
//!
//! The owner **stays alive** after the work finishes. Process exit was never a
//! completion signal for a resident TUI, so every test that asserts a delivery
//! also asserts the owner is still serving afterwards.

#![cfg(all(unix, feature = "web-monitoring"))]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use conflux::events::{ExecutionEvent, OperatorCommandEffect};
use conflux::orchestration::execution_facts::ExecutionFactsStore;
use conflux::orchestration::state::{OrchestratorState, ReducerCommand};
use conflux::web::remote_control_api::dto::{
    AttentionState, ChangeResource, ChangeTiming, CommandSpec, InstanceSnapshot,
    OwnerExecutionContract, ParallelEligibility, ParallelRuntimeState, QueueIntent, SnapshotTotals,
    TerminalMode,
};
use conflux::web::remote_control_api::executor::{
    CommandFailure, ExecutionSummary, RemoteControlExecutor,
};
use conflux::web::{ListenerPlan, ServerHandle, WebConfig, WebState};

// ============================================================================
// Repository fixture
// ============================================================================

/// A deterministic temporary repository, used as terminal-completion proof.
struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().expect("temp dir"),
        };
        repo.git(&["init", "--initial-branch=main"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "Test"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.write("README.md", "fixture\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-m", "init"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.dir.path())
            .output()
            .expect("git must be available");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn stage_active(&self, change_id: &str) {
        self.write(
            &format!("openspec/changes/{change_id}/proposal.md"),
            "# Proposal\n",
        );
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", "add change"]);
    }

    /// Archive the change: entry present in base, active directory gone. This is
    /// the repository evidence a `merged` owner's terminal mode requires.
    fn archive(&self, change_id: &str) {
        let _ = std::fs::remove_dir_all(
            self.dir
                .path()
                .join(format!("openspec/changes/{change_id}")),
        );
        self.write(
            &format!("openspec/changes/archive/2026-01-01-{change_id}/proposal.md"),
            "# Proposal\n",
        );
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", "archive change"]);
    }
}

// ============================================================================
// Owner
// ============================================================================

/// The reducer state an executor and the tests share.
type SharedReducer = Arc<Mutex<OrchestratorState>>;

/// An executor that actually moves the reducer, so an admission opens a real
/// execution episode instead of a fixture-supplied one.
struct ReducerExecutor {
    reducer: SharedReducer,
    facts: Arc<ExecutionFactsStore>,
    dispatch: Mutex<u64>,
    submitted: Mutex<Vec<CommandSpec>>,
}

impl ReducerExecutor {
    fn new(reducer: SharedReducer, facts: Arc<ExecutionFactsStore>) -> Arc<Self> {
        Arc::new(Self {
            reducer,
            facts,
            dispatch: Mutex::new(1_000),
            submitted: Mutex::new(Vec::new()),
        })
    }

    fn submitted(&self) -> Vec<CommandSpec> {
        self.submitted.lock().unwrap().clone()
    }

    fn publish(&self, change_id: &str, queued: bool) {
        let mut id = self.dispatch.lock().unwrap();
        *id += 1;
        let event = ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::QueueDelta {
                change_id: change_id.to_string(),
                queued,
            },
        };
        let reducer = self.reducer.lock().unwrap();
        self.facts.observe(*id, &event, Some(&reducer), Utc::now());
    }
}

#[async_trait]
impl RemoteControlExecutor for ReducerExecutor {
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        self.submitted.lock().unwrap().push(command.clone());
        match command {
            CommandSpec::SetQueueIntent { change_id, queued } => {
                {
                    let mut reducer = self.reducer.lock().unwrap();
                    if *queued {
                        reducer.apply_command(ReducerCommand::AddToQueue(change_id.clone()));
                    } else {
                        reducer.apply_command(ReducerCommand::RemoveFromQueue(change_id.clone()));
                    }
                }
                self.publish(change_id, *queued);
            }
            CommandSpec::RetryChange { change_id } => {
                {
                    let mut reducer = self.reducer.lock().unwrap();
                    reducer.retry_terminal_error(change_id);
                }
                self.publish(change_id, true);
            }
            _ => {}
        }
        Ok(ExecutionSummary::changed("applied by the test reducer"))
    }

    async fn is_command_capable(&self) -> bool {
        true
    }
}

/// A live owner: real listeners, a real registry, and a command-capable
/// executor that moves the same reducer the facts store reads.
struct Owner {
    socket: PathBuf,
    state: Arc<WebState>,
    facts: Arc<ExecutionFactsStore>,
    reducer: SharedReducer,
    executor: Arc<ReducerExecutor>,
    handle: Option<ServerHandle>,
    dispatch: Mutex<u64>,
}

impl Owner {
    /// An owner that never binds an orchestration runtime, so it holds no
    /// execution-sink registry at all — the shape of a build or a process that
    /// predates the surface.
    async fn start_without_sinks(repo: &Repo, socket: PathBuf) -> ServerHandle {
        let state = Arc::new(WebState::new(&[]));
        state.set_repo_root(repo.path().to_path_buf()).await;
        state.set_execution_contract(OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        });
        conflux::web::start_listeners(
            WebConfig {
                enabled: false,
                refresh_interval_secs: 0,
                ..WebConfig::default()
            },
            ListenerPlan {
                unix_socket: Some(socket),
                tcp: false,
            },
            state,
        )
        .await
        .expect("the owner socket must bind")
    }

    async fn start(repo: &Repo, socket: PathBuf, change_ids: &[&str]) -> Self {
        let state = Arc::new(WebState::new(&[]));
        let facts = Arc::new(ExecutionFactsStore::new());
        let reducer: SharedReducer = Arc::new(Mutex::new(OrchestratorState::new(
            change_ids.iter().map(|id| (*id).to_string()).collect(),
            0,
        )));
        let executor = ReducerExecutor::new(reducer.clone(), facts.clone());

        state.set_execution_facts(facts.clone()).await;
        state.set_repo_root(repo.path().to_path_buf()).await;
        state.set_execution_contract(OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        });
        let runtime = state.remote_control();
        runtime.bind(executor.clone()).await;

        let handle = conflux::web::start_listeners(
            WebConfig {
                enabled: false,
                refresh_interval_secs: 0,
                ..WebConfig::default()
            },
            ListenerPlan {
                unix_socket: Some(socket.clone()),
                tcp: false,
            },
            state.clone(),
        )
        .await
        .expect("the owner socket must bind");

        let owner = Self {
            socket,
            state,
            facts,
            reducer,
            executor,
            handle: Some(handle),
            dispatch: Mutex::new(0),
        };
        owner.publish(change_ids, "running");
        owner
    }

    fn instance_id(&self) -> String {
        self.state
            .remote_control()
            .projection()
            .instance_id()
            .to_string()
    }

    /// Publish the owner's authoritative snapshot with real action eligibility.
    fn publish(&self, change_ids: &[&str], app_mode: &str) {
        let changes: Vec<ChangeResource> = change_ids
            .iter()
            .map(|id| {
                let reducer = self.reducer.lock().unwrap();
                let queued = reducer
                    .change_runtime(id)
                    .map(|runtime| {
                        matches!(
                            runtime.queue_intent,
                            conflux::orchestration::state::QueueIntent::Queued
                        )
                    })
                    .unwrap_or(false);
                ChangeResource {
                    id: (*id).to_string(),
                    display_status: "not queued".to_string(),
                    progress_status: "pending".to_string(),
                    completed_tasks: 0,
                    total_tasks: 2,
                    progress_percent: 0.0,
                    dependencies: Vec::new(),
                    iteration_number: None,
                    execution_marked: false,
                    queue_intent: if queued {
                        QueueIntent::Queued
                    } else {
                        QueueIntent::NotQueued
                    },
                    attention: AttentionState::None,
                    blocker: None,
                    error_detail: None,
                    actions: conflux::web::remote_control_api::projection::change_actions_for_test(
                        app_mode,
                        "not queued",
                        None,
                    ),
                    parallel: ParallelEligibility::default(),
                    timing: ChangeTiming::default(),
                    latest_activity: None,
                    worktree: None,
                }
            })
            .collect();
        let total = changes.len();
        self.state.remote_control().projection().apply_state(
            "test_snapshot",
            None,
            serde_json::json!({}),
            InstanceSnapshot {
                app_mode: app_mode.to_string(),
                persistent_scheduler_idle: false,
                is_resolving: false,
                process_error: None,
                parallel: ParallelRuntimeState::default(),
                changes,
                totals: SnapshotTotals {
                    total,
                    completed: 0,
                    in_progress: 0,
                    pending: total,
                },
            },
        );
    }

    /// Drive one typed event through the reducer and the facts store.
    fn dispatch(&self, event: ExecutionEvent) {
        let id = {
            let mut id = self.dispatch.lock().unwrap();
            *id += 1;
            *id
        };
        let mut reducer = self.reducer.lock().unwrap();
        reducer.apply_execution_event(&event);
        self.facts.observe(id, &event, Some(&reducer), Utc::now());
    }

    fn execution_id(&self, change_id: &str) -> Option<String> {
        self.facts.change(change_id).execution_id
    }

    /// True while the owner is still serving its socket.
    async fn is_alive(&self) -> bool {
        tokio::net::UnixStream::connect(&self.socket).await.is_ok()
    }

    async fn stop(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown().await;
        }
    }

    /// Drop every listener without the graceful path, standing in for a crash.
    ///
    /// A crashed owner cannot deliver from a process-local registry, which is
    /// exactly the limitation an external adapter has to cover.
    fn crash(mut self) {
        drop(self.handle.take());
        let _ = std::fs::remove_file(&self.socket);
    }
}

// ============================================================================
// MCP host
// ============================================================================

/// The compiled binary, driven as a real stdio MCP server.
struct McpHost {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpHost {
    fn start(cwd: &Path, socket: &Path) -> Self {
        Self::spawn(
            cwd,
            &["client", "--unix-socket", socket.to_str().unwrap(), "mcp"],
        )
    }

    /// The registration the contract actually recommends: one server, no route
    /// option at all, every call naming its own project.
    fn start_unrouted(cwd: &Path) -> Self {
        Self::spawn(cwd, &["client", "mcp"])
    }

    fn spawn(cwd: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cflx"))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the compiled cflx binary must be runnable");
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut host = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        let initialize = host.request(
            "initialize",
            serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
        );
        assert_eq!(initialize["result"]["serverInfo"]["name"], "cflx-client");
        host.notify("notifications/initialized", serde_json::json!({}));
        host
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        self.next_id += 1;
        let id = self.next_id;
        let frame = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        });
        writeln!(self.stdin, "{frame}").expect("write a request frame");
        self.stdin.flush().expect("flush");

        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read a response frame");
        assert!(!line.trim().is_empty(), "the server closed the stream");
        let response: serde_json::Value =
            serde_json::from_str(line.trim()).unwrap_or_else(|error| {
                panic!("stdout must carry only JSON-RPC frames, got {line:?}: {error}")
            });
        assert_eq!(response["id"], id, "responses arrive in order");
        response
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) {
        let frame = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        writeln!(self.stdin, "{frame}").expect("write a notification frame");
        self.stdin.flush().expect("flush");
    }

    /// Call one tool and return its structured envelope.
    fn call(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let result = self.call_raw(name, arguments);
        assert!(
            result.get("structuredContent").is_some(),
            "the call must have reached an owner: {result}"
        );
        result["structuredContent"].clone()
    }

    /// Call one tool and return the whole tool result, including a refusal that
    /// produced no envelope because nothing was ever contacted.
    fn call_raw(&mut self, name: &str, arguments: serde_json::Value) -> serde_json::Value {
        let response = self.request(
            "tools/call",
            serde_json::json!({"name": name, "arguments": arguments}),
        );
        assert!(
            response.get("error").is_none(),
            "a tool call is not a protocol error: {response}"
        );
        response["result"].clone()
    }

    fn stop(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

// ============================================================================
// Callback recorder
// ============================================================================

/// A callback that appends its payload and then its event marker to a log.
///
/// Order matters: the marker is written *last*, so a test that waits for the
/// marker is guaranteed the payload it then parses is already on disk. Waiting
/// on the marker first would be a race dressed up as an assertion.
fn recorder(dir: &Path, name: &str) -> Vec<String> {
    let script = dir.join(format!("{name}.sh"));
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         { cat \"$CFLX_EVENT_PATH\"; echo; \
         echo \"event=$CFLX_EVENT_TYPE execution=$CFLX_EXECUTION_ID\"; } >> \"$1\"\n",
    )
    .expect("write the recorder script");
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o700);
    }
    std::fs::set_permissions(&script, permissions).unwrap();
    let log = dir.join(format!("{name}.log"));
    vec![
        "/bin/sh".to_string(),
        script.display().to_string(),
        log.display().to_string(),
    ]
}

/// The owner socket a project route derives: `<git-common-dir>/cflx-api.sock`.
///
/// Asked of Git rather than assembled by hand, so a linked worktree and its
/// main repository are proven to agree on one path rather than assumed to.
fn project_socket(repo: &Repo) -> PathBuf {
    common_dir_of(repo.path()).join("cflx-api.sock")
}

fn common_dir_of(dir: &Path) -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(dir)
        .output()
        .expect("git must be available");
    assert!(output.status.success(), "git rev-parse must succeed");
    let raw = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
    let absolute = if raw.is_absolute() {
        raw
    } else {
        dir.join(raw)
    };
    std::fs::canonicalize(&absolute).unwrap_or(absolute)
}

fn log_of(command: &[String]) -> PathBuf {
    PathBuf::from(&command[2])
}

/// Wait until the log contains `needle`, or give up.
///
/// The ceiling is a hang guard, not a latency assertion.
async fn await_event(path: &Path, needle: &str) -> String {
    for _ in 0..600 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if text.contains(needle) {
                return text;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "no '{needle}' event appeared at {}: {:?}",
        path.display(),
        std::fs::read_to_string(path).ok()
    );
}

/// Bounded chance for the dispatcher to act, for assertions about absence.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(500)).await;
}

fn count(path: &Path, needle: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .matches(needle)
        .count()
}

// ============================================================================
// Tests
// ============================================================================

/// The whole point of the change: an agent admits work into a resident TUI over
/// MCP and is told exactly once when that execution finishes — while the TUI is
/// still running, and only because the repository proves it.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_admits_into_a_live_owner_and_notifies_once_on_verified_completion() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let command = recorder(callbacks.path(), "done");

    let mut host = McpHost::start(repo.path(), &socket);

    // The tool surface is closed: no raw command construction is reachable.
    let listed = host.request("tools/list", serde_json::json!({}));
    let names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "cflx_status",
            "cflx_enqueue",
            "cflx_wait",
            "cflx_notify_set",
            "cflx_notify_get",
            "cflx_notify_clear"
        ]
    );

    let status = host.call("cflx_status", serde_json::json!({}));
    assert_eq!(status["outcome"], "observed");
    assert_eq!(status["instance_id"], owner.instance_id());

    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    assert_eq!(admitted["outcome"], "admitted");
    let execution_id = admitted["execution_id"]
        .as_str()
        .expect("an admitted execution carries its episode identity")
        .to_string();
    assert_eq!(admitted["instance_id"], owner.instance_id());
    assert_eq!(
        owner.executor.submitted(),
        vec![CommandSpec::SetQueueIntent {
            change_id: "alpha".to_string(),
            queued: true
        }],
        "a live owner is queued, never started a second time"
    );

    let subscribed = host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "instance_id": owner.instance_id(),
            "command": command,
        }),
    );
    assert_eq!(subscribed["outcome"], "subscribed");
    assert_eq!(subscribed["detail"]["terminal_dispatched"], false);

    // The work finishes: the repository carries the archive entry, and the
    // owner reaches its typed terminal success.
    repo.archive("alpha");
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });

    let record = await_event(&log_of(&command), "event=completed").await;
    assert!(record.contains(&execution_id), "{record}");
    assert!(
        record.contains("\"terminal\": true"),
        "the payload is typed: {record}"
    );

    // Exactly once, and the owner is still serving.
    settle().await;
    assert_eq!(count(&log_of(&command), "event="), 1);
    assert!(
        owner.is_alive().await,
        "the owner must remain alive: process exit was never the completion signal"
    );

    let read = host.call(
        "cflx_notify_get",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "instance_id": owner.instance_id(),
        }),
    );
    assert_eq!(read["outcome"], "subscribed");
    assert_eq!(read["detail"]["terminal_dispatched"], true);
    assert_eq!(read["detail"]["delivered_events"][0], "completed");

    host.stop();
    owner.stop().await;
}

/// The race an agent is genuinely in: the execution settles between the enqueue
/// returning and the notify landing. Losing that notification would make the
/// whole contract unusable.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_delivers_a_terminal_that_settled_before_registration() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let command = recorder(callbacks.path(), "late");

    let mut host = McpHost::start(repo.path(), &socket);
    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    let execution_id = admitted["execution_id"].as_str().unwrap().to_string();

    // Settle first, with nothing registered.
    repo.archive("alpha");
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });
    settle().await;
    assert!(!log_of(&command).exists());

    let subscribed = host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "command": command,
        }),
    );
    assert_eq!(subscribed["outcome"], "subscribed");

    let record = await_event(&log_of(&command), "event=completed").await;
    assert!(record.contains(&execution_id));
    settle().await;
    assert_eq!(count(&log_of(&command), "event="), 1);

    host.stop();
    owner.stop().await;
}

/// Attention is edge-triggered: an unchanged blocked state does not redeliver,
/// and recovery arms the next edge. The terminal still arrives afterwards.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_blocked_attention_repeats_only_after_recovery() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let command = recorder(callbacks.path(), "attention");

    let mut host = McpHost::start(repo.path(), &socket);
    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    let execution_id = admitted["execution_id"].as_str().unwrap().to_string();
    host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "command": command,
            "notify_blocked": true,
        }),
    );

    // Manual deferral: the reducer parks the row idle in an explicit merge
    // wait, which is the typed blocked condition a sink reports on.
    owner.dispatch(ExecutionEvent::MergeDeferred {
        change_id: "alpha".to_string(),
        reason: "waiting on the base lane".to_string(),
        auto_resumable: false,
    });
    await_event(&log_of(&command), "event=blocked").await;
    settle().await;
    assert_eq!(
        count(&log_of(&command), "event=blocked"),
        1,
        "an unchanged blocked state must not redeliver"
    );

    // Recover, then block again: a new attention edge.
    owner.dispatch(ExecutionEvent::ApplyStarted {
        change_id: "alpha".to_string(),
        command: "agent apply".to_string(),
    });
    owner.dispatch(ExecutionEvent::MergeDeferred {
        change_id: "alpha".to_string(),
        reason: "waiting again".to_string(),
        auto_resumable: false,
    });
    for _ in 0..600 {
        if count(&log_of(&command), "event=blocked") == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        count(&log_of(&command), "event=blocked"),
        2,
        "leaving and re-entering blocked arms a new edge"
    );

    // And the terminal is never opt-out.
    repo.archive("alpha");
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });
    await_event(&log_of(&command), "event=completed").await;

    host.stop();
    owner.stop().await;
}

/// A graceful stop tells live registrations the owner is leaving — and says
/// nothing about whether the work finished.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_graceful_owner_shutdown_reports_owner_stopping_not_completion() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let command = recorder(callbacks.path(), "stopping");

    let mut host = McpHost::start(repo.path(), &socket);
    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    let execution_id = admitted["execution_id"].as_str().unwrap().to_string();
    host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "command": command,
        }),
    );

    owner.stop().await;

    let record = await_event(&log_of(&command), "event=owner_stopping").await;
    assert!(
        record.contains("\"terminal\": false"),
        "the owner leaving is not a terminal classification: {record}"
    );
    assert!(
        !record.contains("event=completed"),
        "and it is certainly not completion: {record}"
    );

    host.stop();
}

/// A crashed owner cannot deliver from a process-local registry. The typed
/// answer is `owner_restarted`, and it is never read as success — which is
/// precisely why an external adapter keeps its own continuity observer.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_reports_owner_restarted_after_a_crash_rather_than_completion() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let command = recorder(callbacks.path(), "lost");

    let mut host = McpHost::start(repo.path(), &socket);
    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    let execution_id = admitted["execution_id"].as_str().unwrap().to_string();
    let first_instance = owner.instance_id();
    host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "instance_id": first_instance,
            "command": command,
        }),
    );

    owner.crash();
    let replacement = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    assert_ne!(replacement.instance_id(), first_instance);

    let read = host.call(
        "cflx_notify_get",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": execution_id,
            "instance_id": first_instance,
        }),
    );
    assert_eq!(read["outcome"], "owner_restarted");
    assert_eq!(read["ok"], false);
    assert_eq!(read["detail"]["expected_instance_id"], first_instance);

    // A crash delivers nothing, and nothing invents a completion for it.
    assert!(!log_of(&command).exists());

    host.stop();
    replacement.stop().await;
}

/// A retry is a distinct execution episode, so a sink bound to the first one is
/// never consulted for the second. Two runs of one proposal are two answers.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owner it is talking to is a task in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_retry_is_a_distinct_execution_identity() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;
    let callbacks = tempfile::tempdir().expect("temp dir");
    let first_command = recorder(callbacks.path(), "first");

    let mut host = McpHost::start(repo.path(), &socket);
    let admitted = host.call("cflx_enqueue", serde_json::json!({"change_id": "alpha"}));
    let first_execution = admitted["execution_id"].as_str().unwrap().to_string();
    host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": first_execution,
            "command": first_command,
        }),
    );

    // The first episode fails.
    owner.dispatch(ExecutionEvent::ApplyFailed {
        change_id: "alpha".to_string(),
        error: "boom".to_string(),
    });
    await_event(&log_of(&first_command), "event=failed").await;

    // A retry opens a new episode with its own identity.
    {
        let mut reducer = owner.reducer.lock().unwrap();
        reducer.retry_terminal_error("alpha");
    }
    owner.dispatch(ExecutionEvent::OperatorCommandApplied {
        effect: OperatorCommandEffect::QueueDelta {
            change_id: "alpha".to_string(),
            queued: true,
        },
    });
    let second_execution = owner
        .execution_id("alpha")
        .expect("the retry opens an episode");
    assert_ne!(second_execution, first_execution);

    // The first sink is not consulted for the second episode.
    repo.archive("alpha");
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });
    settle().await;
    assert_eq!(
        count(&log_of(&first_command), "event="),
        1,
        "one terminal per execution, and the retry is a different execution"
    );
    assert_eq!(count(&log_of(&first_command), "event=completed"), 0);

    // And the second episode has no sink of its own until one is registered.
    let read = host.call(
        "cflx_notify_get",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": second_execution,
        }),
    );
    assert_eq!(read["outcome"], "subscribed");
    assert!(read["detail"]["sink"].is_null());

    host.stop();
    owner.stop().await;
}

/// An owner with no execution-sink surface is an *owner-compatibility* fact, not
/// a lost execution and not a protocol error. A client that confused the three
/// would either retry forever or fail the whole MCP session.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_reports_an_owner_without_execution_sinks_as_incompatible() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let handle = Owner::start_without_sinks(&repo, socket.clone()).await;

    let mut host = McpHost::start(repo.path(), &socket);
    let envelope = host.call(
        "cflx_notify_set",
        serde_json::json!({
            "change_id": "alpha",
            "execution_id": "0123456789abcdef0123456789abcdef",
            "command": ["/bin/true"],
        }),
    );
    assert_eq!(envelope["outcome"], "incompatible_owner");
    assert_eq!(envelope["ok"], false);
    // It is a tool result the model can read, not a JSON-RPC failure that would
    // hide the reason from it.
    assert!(envelope["message"]
        .as_str()
        .unwrap()
        .contains("completion sinks"));

    // Status still works against the same owner: only the sink surface is absent.
    let status = host.call("cflx_status", serde_json::json!({}));
    assert_eq!(status["outcome"], "observed");

    host.stop();
    handle.shutdown().await;
}

/// A peer that never sends a newline must not be able to make the adapter hold
/// its input. The compiled binary is the subject on purpose: the bound has to
/// hold on real stdio, where `read_line` would have buffered every byte while
/// waiting for a terminator that never arrives.
///
/// The session ends unread rather than resynchronizing — the remaining bytes
/// belong to a frame this server already refused to hold, and guessing where the
/// next one starts is how a desynchronized stream turns into a dispatched tool
/// call nobody sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_terminates_on_a_newline_free_oversized_frame_without_dispatching() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;

    let mut child = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "--unix-socket", socket.to_str().unwrap(), "mcp"])
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the compiled cflx binary must be runnable");
    let mut stdin = child.stdin.take().expect("piped stdin");

    // Well past the frame limit, and not one newline in it. The writer stops as
    // soon as the adapter drops its end, which is the behaviour under test.
    let writer = std::thread::spawn(move || {
        let mut opening = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":"#.to_vec();
        opening.resize(64 * 1024, b'x');
        for _ in 0..48 {
            if stdin.write_all(&opening).is_err() {
                break;
            }
        }
        let _ = stdin.flush();
    });

    let output = child.wait_with_output().expect("the adapter must exit");
    let _ = writer.join();

    assert_eq!(
        output.status.code(),
        Some(21),
        "an unreadable stream is the transport-error exit status"
    );
    assert!(
        output.stdout.is_empty(),
        "an unread frame is answered with nothing, not with a guess: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        owner.executor.submitted().is_empty(),
        "no owner request may be dispatched from a frame that was never interpreted"
    );

    owner.stop().await;
}

/// The handshake is a gate and the envelope is checked. A host that skips
/// `initialize`, or sends something that is not a JSON-RPC 2.0 request, gets a
/// machine-readable protocol error — and the owner never hears about it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_refuses_tool_traffic_before_initialization_and_off_protocol_frames() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let socket = repo.path().join("cflx-api.sock");
    let owner = Owner::start(&repo, socket.clone(), &["alpha"]).await;

    let mut child = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "--unix-socket", socket.to_str().unwrap(), "mcp"])
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the compiled cflx binary must be runnable");
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

    let mut exchange = |frame: serde_json::Value| -> serde_json::Value {
        writeln!(stdin, "{frame}").expect("write a frame");
        stdin.flush().expect("flush");
        let mut line = String::new();
        stdout.read_line(&mut line).expect("read a response frame");
        serde_json::from_str(line.trim())
            .unwrap_or_else(|error| panic!("stdout must carry only JSON-RPC frames: {error}"))
    };

    // Before initialization, the tools are not reachable.
    for method in ["tools/list", "tools/call"] {
        let response = exchange(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": method,
            "params": {"name": "cflx_enqueue", "arguments": {"change_id": "alpha"}}
        }));
        assert_eq!(
            response["error"]["code"], -32002,
            "{method} must be refused before the handshake: {response}"
        );
    }

    // `ping` is allowed before it, and the handshake itself works afterwards.
    let response = exchange(serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    assert_eq!(response["result"], serde_json::json!({}));
    let response = exchange(serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}}
    }));
    assert_eq!(response["result"]["serverInfo"]["name"], "cflx-client");

    // The envelope is still checked afterwards: no version, wrong version, and a
    // batch array are all invalid requests.
    let response = exchange(serde_json::json!({"id": 4, "method": "tools/list"}));
    assert_eq!(response["error"]["code"], -32600, "{response}");
    let response = exchange(serde_json::json!({
        "jsonrpc": "1.0", "id": 5, "method": "tools/list"
    }));
    assert_eq!(response["error"]["code"], -32600, "{response}");
    let response = exchange(serde_json::json!([
        {"jsonrpc": "2.0", "id": 6, "method": "tools/list"}
    ]));
    assert_eq!(response["error"]["code"], -32600, "{response}");
    assert!(response["id"].is_null(), "a batch has no id to echo");

    // And a properly initialized, properly enveloped call still works.
    let response = exchange(serde_json::json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/list"
    }));
    assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 6);

    assert!(
        owner.executor.submitted().is_empty(),
        "not one refused frame reached the owner"
    );

    drop(stdin);
    let _ = child.wait();
    owner.stop().await;
}

// ============================================================================
// Project-directory routing
// ============================================================================

/// The reason the selector is a directory: one server process, two independent
/// Conflux projects, two owners. Each call reaches only the owner its own
/// `project_dir` names, and neither call changes anything the other reads.
// Multi-threaded on purpose: the MCP host is driven with blocking stdio
// reads, and the owners it is talking to are tasks in this same runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_routes_two_projects_by_their_own_project_directories() {
    let project_a = Repo::new();
    project_a.stage_active("alpha");
    let project_b = Repo::new();
    project_b.stage_active("beta");
    let owner_a = Owner::start(&project_a, project_socket(&project_a), &["alpha"]).await;
    let owner_b = Owner::start(&project_b, project_socket(&project_b), &["beta"]).await;
    assert_ne!(owner_a.instance_id(), owner_b.instance_id());

    // Deliberately outside both projects and with no namespace route, so
    // nothing but the call itself can decide where a request goes.
    let elsewhere = tempfile::tempdir().expect("temp dir");
    let mut host = McpHost::start_unrouted(elsewhere.path());

    for (repo, expected) in [(&project_a, &owner_a), (&project_b, &owner_b)] {
        let status = host.call(
            "cflx_status",
            serde_json::json!({"project_dir": repo.path().to_str().unwrap()}),
        );
        assert_eq!(status["outcome"], "observed", "{status}");
        assert_eq!(status["instance_id"], expected.instance_id(), "{status}");
    }

    // And an intent, not just a read: each admission lands in its own reducer.
    let admitted_a = host.call(
        "cflx_enqueue",
        serde_json::json!({
            "change_id": "alpha",
            "project_dir": project_a.path().to_str().unwrap(),
        }),
    );
    assert_eq!(admitted_a["outcome"], "admitted", "{admitted_a}");
    assert_eq!(admitted_a["instance_id"], owner_a.instance_id());

    let admitted_b = host.call(
        "cflx_enqueue",
        serde_json::json!({
            "change_id": "beta",
            "project_dir": project_b.path().to_str().unwrap(),
        }),
    );
    assert_eq!(admitted_b["outcome"], "admitted", "{admitted_b}");
    assert_eq!(admitted_b["instance_id"], owner_b.instance_id());

    assert_eq!(
        owner_a.executor.submitted(),
        vec![CommandSpec::SetQueueIntent {
            change_id: "alpha".to_string(),
            queued: true
        }],
        "project A saw only its own admission"
    );
    assert_eq!(
        owner_b.executor.submitted(),
        vec![CommandSpec::SetQueueIntent {
            change_id: "beta".to_string(),
            queued: true
        }],
        "project B saw only its own admission"
    );

    host.stop();
    owner_a.stop().await;
    owner_b.stop().await;
}

/// A linked worktree is a different working tree of the *same* repository, and
/// the repository lock lets that repository have exactly one default owner. So
/// naming the worktree has to reach the owner under the shared Git common
/// directory — the same one the main working tree resolves.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_resolves_a_linked_worktree_to_the_common_owner_socket() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let worktrees = tempfile::tempdir().expect("temp dir");
    let linked = worktrees.path().join("feature");
    repo.git(&["worktree", "add", "-b", "feature", linked.to_str().unwrap()]);
    assert_eq!(
        common_dir_of(&linked),
        common_dir_of(repo.path()),
        "a linked worktree shares the repository's common directory"
    );

    let owner = Owner::start(&repo, project_socket(&repo), &["alpha"]).await;
    let elsewhere = tempfile::tempdir().expect("temp dir");
    let mut host = McpHost::start_unrouted(elsewhere.path());

    // The worktree root, a directory below it, and a symlink pointing at it:
    // all three resolve the way Git itself resolves them, so none of them needs
    // the caller to know where the socket is.
    let nested = linked.join("openspec");
    std::fs::create_dir_all(&nested).unwrap();
    let symlinked = worktrees.path().join("by-symlink");
    std::os::unix::fs::symlink(&linked, &symlinked).expect("a symlink must be creatable");
    for directory in [&linked, &nested, &symlinked] {
        let status = host.call(
            "cflx_status",
            serde_json::json!({"project_dir": directory.to_str().unwrap()}),
        );
        assert_eq!(
            status["instance_id"],
            owner.instance_id(),
            "{}: {status}",
            directory.display()
        );
    }

    host.stop();
    owner.stop().await;
}

/// Two selectors in one call is ambiguous, and the only honest answers are
/// "refuse" or "silently pick one" — so it is refused, through the ordinary
/// validation channel, before a byte reaches either owner.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_refuses_two_call_selectors_before_contacting_any_owner() {
    let project_a = Repo::new();
    project_a.stage_active("alpha");
    let project_b = Repo::new();
    project_b.stage_active("alpha");
    let owner_a = Owner::start(&project_a, project_socket(&project_a), &["alpha"]).await;
    let owner_b = Owner::start(&project_b, project_socket(&project_b), &["alpha"]).await;

    let elsewhere = tempfile::tempdir().expect("temp dir");
    let mut host = McpHost::start_unrouted(elsewhere.path());

    let refused = host.call_raw(
        "cflx_enqueue",
        serde_json::json!({
            "change_id": "alpha",
            "project_dir": project_b.path().to_str().unwrap(),
            "unix_socket": project_socket(&project_a).to_str().unwrap(),
        }),
    );
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        refused.get("structuredContent").is_none(),
        "a refusal that contacted nobody has no owner envelope to report: {refused}"
    );
    let text = refused["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains("project_dir") && text.contains("unix_socket"),
        "the refusal must name both selectors: {text}"
    );

    // Nothing was admitted anywhere: the refusal is not a routed failure.
    assert!(owner_a.executor.submitted().is_empty());
    assert!(owner_b.executor.submitted().is_empty());

    // Every unusable project path is refused the same bounded way, and none of
    // them falls through to an owner.
    let not_a_repository = tempfile::tempdir().expect("temp dir");
    let bare = tempfile::tempdir().expect("temp dir");
    let bare_path = bare.path().join("bare.git");
    Command::new("git")
        .args(["init", "--bare", bare_path.to_str().unwrap()])
        .output()
        .expect("git must be available");
    for (label, project_dir) in [
        ("relative", "relative/project".to_string()),
        (
            "missing",
            project_a.path().join("no-such-dir").display().to_string(),
        ),
        (
            "a file rather than a directory",
            project_a.path().join("README.md").display().to_string(),
        ),
        (
            "outside any repository",
            not_a_repository.path().display().to_string(),
        ),
        ("bare", bare_path.display().to_string()),
    ] {
        let refused = host.call_raw(
            "cflx_status",
            serde_json::json!({"project_dir": project_dir}),
        );
        assert_eq!(refused["isError"], true, "{label}: {refused}");
        assert!(
            refused.get("structuredContent").is_none(),
            "{label} must not report an owner observation: {refused}"
        );
    }
    assert!(owner_a.executor.submitted().is_empty());
    assert!(owner_b.executor.submitted().is_empty());

    host.stop();
    owner_a.stop().await;
    owner_b.stop().await;
}

/// The namespace default is a default, not a pin: a call that names a project
/// overrides it, a call that names nothing still gets it, and the override
/// leaves it exactly where it was for the next call.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_lets_a_call_selector_override_the_namespace_default_without_moving_it() {
    let project_a = Repo::new();
    project_a.stage_active("alpha");
    let project_b = Repo::new();
    project_b.stage_active("beta");
    let default_socket = project_a.path().join("cflx-api.sock");
    let owner_a = Owner::start(&project_a, default_socket.clone(), &["alpha"]).await;
    let owner_b = Owner::start(&project_b, project_socket(&project_b), &["beta"]).await;

    let elsewhere = tempfile::tempdir().expect("temp dir");
    let mut host = McpHost::start(elsewhere.path(), &default_socket);

    // The namespace default answers a call that names nothing.
    let default_first = host.call("cflx_status", serde_json::json!({}));
    assert_eq!(default_first["instance_id"], owner_a.instance_id());

    // A call-scoped project overrides it.
    let overridden = host.call(
        "cflx_status",
        serde_json::json!({"project_dir": project_b.path().to_str().unwrap()}),
    );
    assert_eq!(overridden["instance_id"], owner_b.instance_id());

    // And the override moved nothing: the default is still the default.
    let default_again = host.call("cflx_status", serde_json::json!({}));
    assert_eq!(
        default_again["instance_id"],
        owner_a.instance_id(),
        "one call's selector must not become the server's route"
    );

    // The low-level override still works on its own, unchanged.
    let by_socket = host.call(
        "cflx_status",
        serde_json::json!({"unix_socket": project_socket(&project_b).to_str().unwrap()}),
    );
    assert_eq!(by_socket["instance_id"], owner_b.instance_id());

    host.stop();
    owner_a.stop().await;
    owner_b.stop().await;
}

/// The truthfulness half of the route. Completion is proven from a repository,
/// so a call that selected project B must read project B's repository — even
/// when the server is standing in project A and both projects contain the same
/// change ID. Reading the server's own repository here would certify one
/// project's archive as another project's success.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_wait_certifies_evidence_from_the_selected_project_only() {
    // Project A: 'alpha' is still active — its repository proves nothing.
    let project_a = Repo::new();
    project_a.stage_active("alpha");
    // Project B: the same change ID, archived — the evidence `merged` needs.
    let project_b = Repo::new();
    project_b.stage_active("alpha");
    project_b.archive("alpha");

    // Neither owner tracks the change, so nothing but the repository can settle
    // the wait: a tracked change would let a snapshot answer instead of Git.
    let owner_a = Owner::start(&project_a, project_socket(&project_a), &[]).await;
    let owner_b = Owner::start(&project_b, project_socket(&project_b), &[]).await;

    // The server stands inside project A, which is also its default route.
    let mut host = McpHost::start_unrouted(project_a.path());

    let completed = host.call(
        "cflx_wait",
        serde_json::json!({
            "change_id": "alpha",
            "project_dir": project_b.path().to_str().unwrap(),
            "timeout": "30s",
        }),
    );
    assert_eq!(
        completed["outcome"], "completed",
        "project B's archive is the only evidence that could have settled this: {completed}"
    );
    assert_eq!(completed["instance_id"], owner_b.instance_id());

    // The mirror image, with the same server and the same change ID: project A
    // has no archive entry, so it cannot be completed and must time out rather
    // than borrow project B's proof.
    let unproven = host.call(
        "cflx_wait",
        serde_json::json!({
            "change_id": "alpha",
            "project_dir": project_a.path().to_str().unwrap(),
            "timeout": "2s",
        }),
    );
    assert_eq!(
        unproven["outcome"], "timeout",
        "an unarchived change is never completion: {unproven}"
    );

    host.stop();
    owner_a.stop().await;
    owner_b.stop().await;
}

/// Nothing above may cost the old behavior: a server started inside a
/// repository with no route option at all still reaches that repository's
/// owner, and certifies from that repository.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn mcp_without_any_selector_still_uses_the_current_repository() {
    let repo = Repo::new();
    repo.stage_active("alpha");
    let owner = Owner::start(&repo, project_socket(&repo), &["alpha"]).await;

    let mut host = McpHost::start_unrouted(repo.path());
    let status = host.call("cflx_status", serde_json::json!({}));
    assert_eq!(status["outcome"], "observed", "{status}");
    assert_eq!(status["instance_id"], owner.instance_id());

    host.stop();
    owner.stop().await;
}
