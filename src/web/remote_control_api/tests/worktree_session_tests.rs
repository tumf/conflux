//! Configured-command worktree sessions on the `/api/v2` wire.
//!
//! Integration evidence, not unit: every case here drives the production router
//! and the real application transaction
//! ([`crate::web::remote_control_api::executor::wired_for_test`]) over a real
//! `WorktreeService`, a real Git backend, a disposable repository, and real
//! `sh` processes spawned by the production runner. They are therefore gated
//! behind the `heavy-tests` feature.
//!
//! Readiness is a FIFO the script writes to, and settlement is the session's own
//! state transition observed through the projection's event channel. Timeouts
//! below are generous hang guards only; no assertion depends on wall-clock time.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};

use crate::config::OrchestratorConfig;
use crate::orchestration::operator_command::{
    ExecutionMarkStore, OperatorCommandService, ParallelRuntime,
};
use crate::orchestration::operator_coordinator::CoreMode;
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::worktree_sessions::{
    ProjectionSessionEvents, RemoteSessionOperations, WorktreeSessionOperations,
};
use crate::web::remote_control_api::worktrees::{
    RemoteWorktreeOperations, WorktreeOperations, WorktreeRegistry,
};
use crate::web::remote_control_api::{router, RemoteControlState};
use crate::web::state::WebState;
use crate::worktree_ops::git_backend::GitWorktreeBackend;
use crate::worktree_ops::service::{NullEventSink, WorktreeService};
use crate::worktree_ops::session::{
    GitSessionProvisionRuntime, SessionProvisionRuntime, SessionState, WorktreeSessionRegistry,
};

use super::{get, post_json, send, status_and_json};

const TOKEN: &str = "session-test-token";
const HANG_GUARD: Duration = Duration::from_secs(60);

// ============================================================================
// Repository fixture
// ============================================================================

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

struct Fixture {
    temp: tempfile::TempDir,
    repo: PathBuf,
    base: PathBuf,
    scripts: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::TempDir::new().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let repo = root.join("repo");
        let base = root.join("ws");
        let scripts = root.join("scripts");
        for dir in [&repo, &base, &scripts] {
            std::fs::create_dir_all(dir).unwrap();
        }
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "Test User"]);
        std::fs::write(repo.join("README.md"), "test\n").unwrap();
        // Keep the operator's `.wt/` out of the tracked tree.
        std::fs::write(repo.join(".git/info/exclude"), ".wt/\n").unwrap();
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "-m", "init"]);
        Self {
            temp,
            repo,
            base,
            scripts,
        }
    }

    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.temp.path()).unwrap()
    }

    fn file(&self, name: &str) -> PathBuf {
        self.scripts.join(name)
    }

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.file(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    fn fifo(&self, name: &str) -> PathBuf {
        let path = self.file(name);
        let status = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs");
        assert!(status.success());
        path
    }

    fn add_worktree(&self, name: &str) -> PathBuf {
        let path = self.base.join(name);
        git(
            &self.repo,
            &[
                "worktree",
                "add",
                "-b",
                name,
                path.to_str().unwrap(),
                "HEAD",
            ],
        );
        std::fs::canonicalize(path).unwrap()
    }

    fn setup_script(&self, body: &str) {
        let dir = self.repo.join(".wt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("setup"), body).unwrap();
    }

    fn worktree_list(&self) -> String {
        git(&self.repo, &["worktree", "list", "--porcelain"])
    }

    fn session_branches(&self) -> String {
        git(&self.repo, &["branch", "--list", "ws-session-*"])
    }
}

/// Block until the script writes its readiness line, without polling.
async fn read_fifo(path: PathBuf) -> String {
    tokio::time::timeout(
        HANG_GUARD,
        tokio::task::spawn_blocking(move || std::fs::read_to_string(path).unwrap()),
    )
    .await
    .expect("the script never signalled readiness")
    .unwrap()
}

/// True while `pid` names a live, non-zombie process.
fn alive(pid: i32) -> bool {
    if nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err() {
        return false;
    }
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

fn pids(path: &Path) -> Vec<i32> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

// ============================================================================
// Production wiring
// ============================================================================

struct Wired {
    fixture: Fixture,
    router: axum::Router,
    projection: Arc<Projection>,
    registry: Arc<WorktreeSessionRegistry>,
    keys: AtomicUsize,
}

/// Builds a replacement provisioning seam over the wired service.
type ProvisionFactory = Box<dyn FnOnce(Arc<WorktreeService>) -> Arc<dyn SessionProvisionRuntime>>;

struct Options {
    template: Option<String>,
    retries: u32,
    provision: Option<ProvisionFactory>,
}

impl Options {
    fn template(template: impl Into<String>) -> Self {
        Self {
            template: Some(template.into()),
            retries: 1,
            provision: None,
        }
    }

    fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}

impl Wired {
    async fn new(fixture: Fixture, options: Options) -> Self {
        let config = Arc::new(OrchestratorConfig {
            worktree_command: options.template.clone(),
            workspace_base_dir: Some(fixture.base.to_string_lossy().to_string()),
            command_queue_stagger_delay_ms: Some(0),
            command_queue_max_retries: Some(options.retries),
            command_queue_retry_delay_ms: Some(0),
            ..Default::default()
        });

        let registry = WorktreeSessionRegistry::new(
            crate::ai_command_runner::AiCommandRunner::from_orchestrator_config(
                &config,
                Arc::new(tokio::sync::Mutex::new(None)),
            ),
        );
        let service = Arc::new(
            WorktreeService::new(
                Arc::new(GitWorktreeBackend::new(
                    fixture.repo.clone(),
                    config.clone(),
                )),
                Arc::new(NullEventSink),
                fixture.base.clone(),
            )
            .with_session_probe(registry.clone()),
        );
        let worktrees = Arc::new(RemoteWorktreeOperations::new(
            service.clone(),
            Arc::new(WorktreeRegistry::new()),
            fixture.repo.clone(),
        ));
        let mut sessions = RemoteSessionOperations::new(
            registry.clone(),
            worktrees.clone(),
            service.clone(),
            fixture.repo.clone(),
            options.template.clone(),
        )
        .await;
        if let Some(provision) = options.provision {
            sessions = sessions.with_provision_runtime(provision(service.clone()));
        }
        let sessions: Arc<dyn WorktreeSessionOperations> = Arc::new(sessions);
        let worktree_port: Arc<dyn WorktreeOperations> = worktrees;

        // The real application transaction, exactly as the TUI owner binds it.
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let operator = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                Arc::new(crate::tui::queue::DynamicQueue::new()),
                Arc::new(crate::orchestration::operator_command::NoopQueueHooks),
                marks,
            )
            .with_parallel(parallel.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            operator,
            Arc::new(RecordingScheduler::new()),
            Arc::new(ResolveReservations::new()),
            parallel,
        ));
        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        let (executor, application) = crate::web::remote_control_api::executor::wired_for_test(
            reducer,
            run_control,
            web_state.clone(),
            Arc::new(CoreMode::new()),
        );
        let executor = executor
            .with_worktrees(worktree_port.clone())
            .with_worktree_sessions(sessions.clone());

        let runtime = web_state.remote_control();
        registry.set_event_sink(Arc::new(ProjectionSessionEvents::new(runtime.projection())));
        runtime.bind(Arc::new(executor)).await;
        runtime.bind_gate(application.gate()).await;
        runtime.bind_worktrees(worktree_port).await;
        runtime.bind_worktree_sessions(sessions);

        let projection = runtime.projection();
        let auth = RemoteControlAuth::new(Some(TOKEN.to_string()), &[]).unwrap();
        let router = router(
            RemoteControlState::new(projection.clone(), Arc::new(auth), runtime.clone())
                .with_gate(runtime.gate())
                .with_worktrees(runtime.clone())
                .with_worktree_sessions(runtime.worktree_sessions()),
        );

        Self {
            fixture,
            router,
            projection,
            registry,
            keys: AtomicUsize::new(0),
        }
    }

    fn next_key(&self) -> String {
        format!("key-{}", self.keys.fetch_add(1, Ordering::SeqCst))
    }

    async fn submit(&self, command: Value) -> (StatusCode, Value) {
        let key = self.next_key();
        self.submit_keyed(command, &key).await
    }

    async fn submit_keyed(&self, mut command: Value, key: &str) -> (StatusCode, Value) {
        command["expected_revision"] = json!(self.projection.revision());
        command["idempotency_key"] = json!(key);
        status_and_json(
            send(
                &self.router,
                post_json("/api/v2/commands", Some(TOKEN), &command.to_string()),
            )
            .await,
        )
        .await
    }

    async fn read(&self, uri: &str) -> Value {
        let (status, body) = status_and_json(send(&self.router, get(uri, Some(TOKEN))).await).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        body
    }

    async fn worktree_id(&self, path: &Path) -> String {
        let body = self.read("/api/v2/worktrees").await;
        let display = crate::web::remote_control_api::worktrees::repository_relative_display(
            &self.fixture.repo,
            path,
        );
        body["worktrees"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["path"] == display.as_str())
            .unwrap_or_else(|| panic!("worktree {display} not listed: {body}"))["worktree_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn run(&self, worktree_id: &str) -> (StatusCode, Value) {
        self.submit(json!({
            "type": "run_worktree_command",
            "target": {"worktree_id": worktree_id},
            "params": {}
        }))
        .await
    }

    async fn launched(&self, worktree_id: &str) -> String {
        let (status, record) = self.run(worktree_id).await;
        assert_eq!(status, StatusCode::OK, "{record}");
        assert_eq!(record["state"], "succeeded", "settled at launch: {record}");
        assert_eq!(record["result"]["kind"], "worktree_session", "{record}");
        record["result"]["session_id"].as_str().unwrap().to_string()
    }

    async fn session(&self, session_id: &str) -> Value {
        self.read(&format!("/api/v2/worktree-sessions/{session_id}"))
            .await["session"]
            .clone()
    }

    /// Wait for the session's own state transition, driven by the events the
    /// registry publishes. No sleep.
    async fn settled(&self, session_id: &str) -> Value {
        let mut events = self.projection.subscribe();
        loop {
            if self
                .registry
                .get(session_id)
                .is_some_and(|s| s.state != SessionState::Running)
            {
                return self.session(session_id).await;
            }
            let _ = tokio::time::timeout(HANG_GUARD, events.recv())
                .await
                .expect("the session never settled");
        }
    }

    /// Wait for a command record to settle (a stop can outlive the 250 ms
    /// synchronous grace and answer 202 first).
    async fn command_settled(&self, record: Value) -> Value {
        let id = record["command_id"].as_str().unwrap().to_string();
        let deadline = tokio::time::Instant::now() + HANG_GUARD;
        loop {
            let current = self.read(&format!("/api/v2/commands/{id}")).await;
            if current["state"] != "running" {
                return current;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "command never settled"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn stop(&self, session_id: &str) -> Value {
        let (status, record) = self
            .submit(json!({
                "type": "stop_worktree_session",
                "target": {"session_id": session_id},
                "params": {}
            }))
            .await;
        assert!(
            status == StatusCode::OK || status == StatusCode::ACCEPTED,
            "{record}"
        );
        self.command_settled(record).await
    }

    fn session_events(&self) -> Vec<Value> {
        let crate::web::remote_control_api::projection::EventsSince::Replay(events) =
            self.projection.events_after(0)
        else {
            panic!("the test ring never overflows");
        };
        events
            .into_iter()
            .filter(|e| e.event_type.starts_with("worktree_session_"))
            .map(|e| serde_json::to_value(e).unwrap())
            .collect()
    }

    async fn shutdown(&self) {
        let cleanup = self.registry.shutdown(Duration::from_secs(10)).await;
        assert!(cleanup.is_quiescent(), "{}", cleanup.diagnostics());
    }
}

/// A panicking test never reaches `shutdown()`, and dropping the runtime does
/// not kill children. Kill whatever process group the scope still owns so a
/// failure cannot leak `sleep 300` processes.
impl Drop for Wired {
    fn drop(&mut self) {
        for pgid in self.registry.scope().retained_process_ids() {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pgid as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

/// A long-running script: one PID line per attempt, a background child in the
/// same process group, a FIFO readiness signal, then a foreground sleep.
fn sleeper(fixture: &Fixture, name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let pids = fixture.file(&format!("{name}.pids"));
    let background = fixture.file(&format!("{name}.bg"));
    let ready = fixture.fifo(&format!("{name}.ready"));
    let script = fixture.script(
        &format!("{name}.sh"),
        &format!(
            "echo $$ >> '{pids}'\nsleep 300 &\necho $! > '{bg}'\necho ready > '{ready}'\nsleep 300\n",
            pids = pids.display(),
            bg = background.display(),
            ready = ready.display(),
        ),
    );
    (script, pids, ready)
}

// ============================================================================
// Launch
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_run_executes_in_the_worktree_with_null_stdin() {
    let fixture = Fixture::new();
    let out = fixture.file("run.out");
    let script = fixture.script(
        "run.sh",
        &format!(
            "{{ pwd; echo \"ws=$1\"; echo \"root=$2\"; if read x; then echo stdin:data; else echo stdin:eof; fi; }} | tee '{}'\n",
            out.display()
        ),
    );
    let worktree = fixture.add_worktree("feature-a");
    let template = format!("sh {} {{workspace_dir}} {{repo_root}}", script.display());
    let wired = Wired::new(fixture, Options::template(template)).await;
    let revision_before = wired.projection.revision();

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;

    let written = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines[0], worktree.to_str().unwrap(), "cwd is the worktree");
    assert_eq!(lines[1], format!("ws={}", worktree.display()));
    assert_eq!(lines[2], format!("root={}", wired.fixture.repo.display()));
    assert_eq!(lines[3], "stdin:eof", "stdin is /dev/null");

    assert_eq!(session["state"], "succeeded");
    assert_eq!(session["exit_code"], 0);
    assert_eq!(session["attempt"], 1);
    assert_eq!(session["kind"], "existing");
    assert_eq!(session["worktree_id"], id.as_str());
    assert!(!session["path"].as_str().unwrap().starts_with('/'));
    let texts: Vec<String> = session["output"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["text"].as_str().unwrap().to_string())
        .collect();
    assert!(texts.contains(&"stdin:eof".to_string()), "{texts:?}");

    // This is a raw-output fixture: the absolute cwd it printed lands in
    // `output.lines` and in no owner-generated field.
    let finished: Vec<Value> = wired
        .session_events()
        .into_iter()
        .filter(|e| e["event_type"] == "worktree_session_finished")
        .collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0]["payload"]["session_id"], session_id.as_str());
    assert_eq!(finished[0]["payload"]["exit_code"], 0);
    assert!(finished[0]["change_id"].is_null());
    let abs = worktree.to_str().unwrap();
    let mut owner_fields = session.clone();
    owner_fields["output"] = Value::Null;
    for (what, value) in [
        ("session", owner_fields.to_string()),
        ("events", json!(wired.session_events()).to_string()),
        ("logs", wired.read("/api/v2/logs").await.to_string()),
    ] {
        assert!(!value.contains(abs), "{what} discloses the cwd: {value}");
    }
    assert_eq!(
        wired.projection.revision(),
        revision_before,
        "sessions never move the revision"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_create_provisions_setup_and_launches() {
    let fixture = Fixture::new();
    fixture.setup_script("#!/bin/sh\necho setup > setup-marker\n");
    let script = fixture.script("create.sh", "cat setup-marker\n");
    let template = format!("sh {} {{workspace_dir}}", script.display());
    let wired = Wired::new(fixture, Options::template(template)).await;

    let (status, record) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    let result = &record["result"];
    assert_eq!(result["kind"], "worktree_session");
    let branch = result["branch"].as_str().unwrap();
    assert!(branch.starts_with("ws-session-"), "{branch}");
    assert_eq!(result["setup"], json!({"ran": true, "succeeded": true}));
    assert!(result["cleanup"].is_null());
    assert!(result["path"]
        .as_str()
        .unwrap()
        .starts_with("../ws/proposal-"));

    let session_id = result["session_id"].as_str().unwrap();
    let session = wired.settled(session_id).await;
    assert_eq!(session["state"], "succeeded");
    assert_eq!(session["kind"], "session");
    assert_eq!(
        session["output"]["lines"][0]["text"], "setup",
        "setup ran first"
    );

    // The opaque ID names a real, listed, uninspected session worktree.
    let worktree_id = result["worktree_id"].as_str().unwrap();
    let listed = wired
        .read(&format!("/api/v2/worktrees/{worktree_id}"))
        .await["worktree"]
        .clone();
    assert_eq!(listed["branch"], branch);
    assert_eq!(listed["path"], result["path"]);
    assert!(wired.fixture.worktree_list().contains(branch));
    assert!(wired.fixture.session_branches().contains(branch));
    assert_eq!(
        listed["inspection"], "not_inspected",
        "periodic inspection treats a session worktree exactly as a TUI `+` one"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_setup_failure_cleans_up_and_launches_nothing() {
    let fixture = Fixture::new();
    fixture.setup_script("#!/bin/sh\necho setup-broke >&2\nexit 7\n");
    let script = fixture.script("never.sh", "echo should-not-run\n");
    let template = format!("sh {}", script.display());
    let worktrees_before = fixture.worktree_list();
    let wired = Wired::new(fixture, Options::template(template.clone())).await;

    let (status, record) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["state"], "failed");
    assert_eq!(record["error_code"], "worktree_session_setup_failed");
    let result = &record["result"];
    assert!(result["session_id"].is_null(), "nothing was launched");
    assert_eq!(result["cleanup"]["removed"], true);
    assert_eq!(result["cleanup"]["outcome"], "removed");
    assert_eq!(result["setup"]["succeeded"], false);
    let branch = result["branch"].as_str().unwrap();

    // Worktree directory and registration are gone; the branch is retained,
    // exactly as the TUI's forced removal without teardown leaves it.
    assert_eq!(wired.fixture.worktree_list(), worktrees_before);
    assert!(wired.fixture.session_branches().contains(branch));
    assert!(!wired
        .fixture
        .repo
        .join(result["path"].as_str().unwrap())
        .exists());
    assert_eq!(wired.registry.launch_count(), 0);
    assert!(wired.registry.list().is_empty());
    let detail = record["detail"].as_str().unwrap();
    assert!(
        !detail.contains(&wired.fixture.root().to_string_lossy().to_string()),
        "{detail}"
    );

    // A cleanup that itself fails reports `retained` and its reason.
    struct FailingCleanup(GitSessionProvisionRuntime);
    #[async_trait]
    impl SessionProvisionRuntime for FailingCleanup {
        async fn check_git_repo(&self, r: &Path) -> Result<bool, String> {
            self.0.check_git_repo(r).await
        }
        async fn generate_unique_branch_name(
            &self,
            r: &Path,
            p: &str,
            n: u32,
        ) -> Result<String, String> {
            self.0.generate_unique_branch_name(r, p, n).await
        }
        async fn worktree_add(&self, r: &Path, w: &str, b: &str, c: &str) -> Result<(), String> {
            self.0.worktree_add(r, w, b, c).await
        }
        async fn validate_worktree_command_cwd(&self, r: &Path, w: &Path) -> Result<(), String> {
            self.0.validate_worktree_command_cwd(r, w).await
        }
        async fn run_worktree_setup(&self, r: &Path, w: &Path) -> Result<bool, String> {
            self.0.run_worktree_setup(r, w).await
        }
        async fn worktree_remove_after_setup_failure(
            &self,
            _r: &Path,
            w: &str,
        ) -> Result<(), String> {
            Err(format!("cannot remove {w}: device busy"))
        }
    }
    let fixture = Fixture::new();
    fixture.setup_script("#!/bin/sh\nexit 1\n");
    let wired = Wired::new(
        fixture,
        Options {
            provision: Some(Box::new(|service: Arc<WorktreeService>| {
                Arc::new(FailingCleanup(GitSessionProvisionRuntime::new(
                    service.backend(),
                ))) as Arc<dyn SessionProvisionRuntime>
            })),
            ..Options::template(template)
        },
    )
    .await;
    let (status, record) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    let cleanup = &record["result"]["cleanup"];
    assert_eq!(cleanup["outcome"], "retained", "{record}");
    assert_eq!(cleanup["removed"], false);
    let reason = cleanup["reason"].as_str().unwrap();
    assert!(reason.contains("device busy"), "{reason}");
    assert!(
        !reason.contains(&wired.fixture.root().to_string_lossy().to_string()),
        "the reason is redacted: {reason}"
    );
    assert!(
        record["result"]["worktree_id"].is_string(),
        "the retained worktree is addressable"
    );
    assert_eq!(wired.registry.launch_count(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_invalid_cwd_before_launch_is_refused() {
    /// Removes the worktree between setup and the final validation.
    struct VanishingWorktree {
        inner: GitSessionProvisionRuntime,
        validations: AtomicUsize,
    }
    #[async_trait]
    impl SessionProvisionRuntime for VanishingWorktree {
        async fn check_git_repo(&self, r: &Path) -> Result<bool, String> {
            self.inner.check_git_repo(r).await
        }
        async fn generate_unique_branch_name(
            &self,
            r: &Path,
            p: &str,
            n: u32,
        ) -> Result<String, String> {
            self.inner.generate_unique_branch_name(r, p, n).await
        }
        async fn worktree_add(&self, r: &Path, w: &str, b: &str, c: &str) -> Result<(), String> {
            self.inner.worktree_add(r, w, b, c).await
        }
        async fn validate_worktree_command_cwd(&self, r: &Path, w: &Path) -> Result<(), String> {
            if self.validations.fetch_add(1, Ordering::SeqCst) == 2 {
                std::fs::remove_dir_all(w).unwrap();
            }
            self.inner.validate_worktree_command_cwd(r, w).await
        }
        async fn run_worktree_setup(&self, r: &Path, w: &Path) -> Result<bool, String> {
            self.inner.run_worktree_setup(r, w).await
        }
        async fn worktree_remove_after_setup_failure(
            &self,
            r: &Path,
            w: &str,
        ) -> Result<(), String> {
            self.inner.worktree_remove_after_setup_failure(r, w).await
        }
    }

    let fixture = Fixture::new();
    let script = fixture.script("never.sh", "echo should-not-run\n");
    let wired = Wired::new(
        fixture,
        Options {
            provision: Some(Box::new(|service: Arc<WorktreeService>| {
                Arc::new(VanishingWorktree {
                    inner: GitSessionProvisionRuntime::new(service.backend()),
                    validations: AtomicUsize::new(0),
                }) as Arc<dyn SessionProvisionRuntime>
            })),
            ..Options::template(format!("sh {}", script.display()))
        },
    )
    .await;

    let (status, record) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["error_code"], "target_ineligible");
    let detail = record["detail"].as_str().unwrap();
    assert!(
        detail.starts_with("reason=cwd_invalid phase=before_launch"),
        "{detail}"
    );
    assert!(
        !detail.contains(&wired.fixture.root().to_string_lossy().to_string()),
        "{detail}"
    );
    assert!(record["result"]["session_id"].is_null());
    assert_eq!(wired.registry.launch_count(), 0, "no process started");
}

// ============================================================================
// Single flight, capacity, retries
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_replay_and_concurrency_launch_once() {
    let fixture = Fixture::new();
    let (script, pids_file, ready) = sleeper(&fixture, "busy");
    let worktrees: Vec<PathBuf> = (0..5)
        .map(|n| fixture.add_worktree(&format!("wt-{n}")))
        .collect();
    // Only the first session waits on the FIFO; the others just sleep.
    let quiet = fixture.script("quiet.sh", "sleep 300\n");
    let template = format!(
        "if [ \"$(basename {{workspace_dir}})\" = wt-0 ]; then sh {}; else sh {}; fi",
        script.display(),
        quiet.display()
    );
    let wired = Wired::new(fixture, Options::template(template)).await;
    let ids = {
        let mut ids = Vec::new();
        for path in &worktrees {
            ids.push(wired.worktree_id(path).await);
        }
        ids
    };

    let command = json!({
        "type": "run_worktree_command",
        "target": {"worktree_id": ids[0]},
        "params": {}
    });
    let (status, first) = wired.submit_keyed(command.clone(), "replayed").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    read_fifo(ready).await;
    let (status, replay) = wired.submit_keyed(command.clone(), "replayed").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        replay["command_id"], first["command_id"],
        "replay returns the record"
    );
    assert_eq!(replay["result"], first["result"]);
    assert_eq!(
        wired.registry.launch_count(),
        1,
        "the replay launched nothing"
    );
    assert_eq!(pids(&pids_file).len(), 1);

    let (status, busy) = wired.run(&ids[0]).await;
    assert_eq!(status, StatusCode::CONFLICT, "{busy}");
    assert_eq!(busy["error_code"], "worktree_session_busy");
    assert!(busy["detail"]
        .as_str()
        .unwrap()
        .starts_with("reason=worktree_busy"));

    let mut running = vec![first["result"]["session_id"].as_str().unwrap().to_string()];
    for id in &ids[1..4] {
        running.push(wired.launched(id).await);
    }
    let (status, capacity) = wired.run(&ids[4]).await;
    assert_eq!(status, StatusCode::CONFLICT, "{capacity}");
    assert_eq!(capacity["error_code"], "worktree_session_busy");
    assert!(capacity["detail"]
        .as_str()
        .unwrap()
        .starts_with("reason=capacity"));
    assert_eq!(wired.registry.launch_count(), 4);

    let status = wired.read("/api/v2/execution-status").await;
    assert_eq!(status["process"]["worktree_sessions_running"], 4);

    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_configured_retry_stays_inside_one_session() {
    let fixture = Fixture::new();
    let marker = fixture.file("attempted");
    let script = fixture.script(
        "flaky.sh",
        &format!(
            "if [ -e '{m}' ]; then echo second; exit 0; fi\ntouch '{m}'\necho first\nexit 3\n",
            m = marker.display()
        ),
    );
    let worktree = fixture.add_worktree("flaky");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())).retries(3),
    )
    .await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;
    assert_eq!(session["state"], "succeeded");
    assert_eq!(session["attempt"], 2, "the retry is inside the session");
    assert_eq!(session["session_id"], session_id.as_str());
    assert_eq!(wired.registry.list().len(), 1, "one session, not two");
    assert_eq!(wired.registry.launch_count(), 1);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_create_capacity_refusal_creates_nothing() {
    /// Counts provisioning calls so "nothing was attempted" is observable.
    struct Counting {
        inner: GitSessionProvisionRuntime,
        calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl SessionProvisionRuntime for Counting {
        async fn check_git_repo(&self, r: &Path) -> Result<bool, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.check_git_repo(r).await
        }
        async fn generate_unique_branch_name(
            &self,
            r: &Path,
            p: &str,
            n: u32,
        ) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.generate_unique_branch_name(r, p, n).await
        }
        async fn worktree_add(&self, r: &Path, w: &str, b: &str, c: &str) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.worktree_add(r, w, b, c).await
        }
        async fn validate_worktree_command_cwd(&self, r: &Path, w: &Path) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.validate_worktree_command_cwd(r, w).await
        }
        async fn run_worktree_setup(&self, r: &Path, w: &Path) -> Result<bool, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.run_worktree_setup(r, w).await
        }
        async fn worktree_remove_after_setup_failure(
            &self,
            r: &Path,
            w: &str,
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.worktree_remove_after_setup_failure(r, w).await
        }
    }

    let fixture = Fixture::new();
    let worktrees: Vec<PathBuf> = (0..4)
        .map(|n| fixture.add_worktree(&format!("full-{n}")))
        .collect();
    let quiet = fixture.script("quiet.sh", "sleep 300\n");
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let wired = Wired::new(
        fixture,
        Options {
            provision: Some(Box::new(move |service: Arc<WorktreeService>| {
                Arc::new(Counting {
                    inner: GitSessionProvisionRuntime::new(service.backend()),
                    calls: counted,
                }) as Arc<dyn SessionProvisionRuntime>
            })),
            ..Options::template(format!("sh {}", quiet.display()))
        },
    )
    .await;

    let mut sessions = Vec::new();
    for path in &worktrees {
        let id = wired.worktree_id(path).await;
        sessions.push(wired.launched(&id).await);
    }
    let worktrees_before = wired.fixture.worktree_list();
    let branches_before = wired.fixture.session_branches();

    let (status, refused) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "worktree_session_busy");
    assert!(refused["detail"]
        .as_str()
        .unwrap()
        .starts_with("reason=capacity"));
    assert!(
        refused.get("result").is_none(),
        "no worktree_session result"
    );
    assert_eq!(wired.fixture.worktree_list(), worktrees_before);
    assert_eq!(wired.fixture.session_branches(), branches_before);
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no provisioning step ran");

    // The reservation was released: one session finishing frees the slot.
    let stopped = wired.stop(&sessions[0]).await;
    assert_eq!(stopped["result"]["state_after"], "stopped");
    let (status, created) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert!(created["result"]["session_id"].is_string());

    wired.shutdown().await;
}

// ============================================================================
// Stop and shutdown
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_stop_kills_the_process_group_and_reaps() {
    let fixture = Fixture::new();
    let (script, pids_file, ready) = sleeper(&fixture, "stop");
    let background_file = fixture.file("stop.bg");
    let worktree = fixture.add_worktree("stoppable");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())).retries(3),
    )
    .await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    read_fifo(ready).await;
    let leader = pids(&pids_file)[0];
    let background: i32 = std::fs::read_to_string(&background_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(leader) && alive(background));

    let record = wired.stop(&session_id).await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["kind"], "worktree_session_stop");
    assert_eq!(record["result"]["terminated"], true);
    assert_eq!(record["result"]["state_after"], "stopped");

    // Settled only after the reap: both group members are already gone.
    assert!(!alive(leader), "leader {leader} survived");
    assert!(!alive(background), "background child {background} survived");

    let session = wired.session(&session_id).await;
    assert_eq!(session["state"], "stopped");
    assert_eq!(session["signal"], "SIGTERM");
    assert_eq!(session["stop_requested"], true);
    assert_eq!(
        session["attempt"], 1,
        "the configured retry did not relaunch"
    );
    assert_eq!(
        pids(&pids_file),
        vec![leader],
        "no new PID after settlement"
    );

    // A second stop is an explicit no-op.
    let again = wired.stop(&session_id).await;
    assert_eq!(again["state"], "no_op");
    assert_eq!(again["result"]["terminated"], false);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_shutdown_terminates_running_sessions() {
    let fixture = Fixture::new();
    let (script, pids_file, ready) = sleeper(&fixture, "shutdown");
    let worktree = fixture.add_worktree("shutdown");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())).retries(3),
    )
    .await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    read_fifo(ready).await;
    let leader = pids(&pids_file)[0];

    let cleanup = wired.registry.shutdown(Duration::from_secs(10)).await;
    assert!(cleanup.is_quiescent(), "{}", cleanup.diagnostics());
    assert!(!alive(leader));
    let session = wired.session(&session_id).await;
    assert_eq!(session["state"], "stopped");
    assert_eq!(session["attempt"], 1);
    assert_eq!(pids(&pids_file), vec![leader], "no new PID");

    // Scheduler stop never closes the registry; owner shutdown does, for good.
    let (status, refused) = wired.run(&id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "worktree_session_unavailable");
    assert_eq!(wired.registry.launch_count(), 1, "no spawn after shutdown");
    let capabilities = wired.read("/api/v2/capabilities").await;
    assert_eq!(capabilities["worktree_sessions"]["available"], false);
}

// ============================================================================
// Output and disclosure
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_output_is_bounded_and_sanitized() {
    let fixture = Fixture::new();
    let script = fixture.script(
        "chatty.sh",
        "i=0\nwhile [ $i -lt 2000 ]; do printf '\\033[31m%0100d\\033[0m\\n' $i; i=$((i+1)); done\n",
    );
    let worktree = fixture.add_worktree("chatty");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())),
    )
    .await;
    let revision_before = wired.projection.revision();
    let logs_before = wired.projection.logs().0.len();

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;

    let output = &session["output"];
    assert!(output["retained_bytes"].as_u64().unwrap() <= 65_536);
    assert!(output["truncated_bytes"].as_u64().unwrap() > 0);
    let serialized = output.to_string();
    assert!(!serialized.contains('\u{1b}') && !serialized.contains("\\u001b"));
    let last = output["lines"].as_array().unwrap().last().unwrap();
    assert_eq!(
        last["text"],
        format!("{:0100}", 1999),
        "the newest lines are kept"
    );

    // Output is never a log entry, an event, or a revision bump.
    assert_eq!(wired.projection.logs().0.len(), logs_before);
    assert_eq!(wired.session_events().len(), 2);
    assert_eq!(wired.projection.revision(), revision_before);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_discloses_no_command_path_or_secret() {
    let fixture = Fixture::new();
    // Metadata fixture: the script prints a fixed literal and never echoes its
    // arguments, environment, or cwd.
    let script = fixture.script("quiet.sh", "echo fixed-literal\n");
    let worktree = fixture.add_worktree("secretive");
    let template = format!(
        "sh {} --token SECRET {{workspace_dir}} {{repo_root}}",
        script.display()
    );
    let wired = Wired::new(fixture, Options::template(template.clone())).await;
    let root = wired.fixture.root().to_string_lossy().to_string();

    let id = wired.worktree_id(&worktree).await;
    let (status, record) = wired.run(&id).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    let session_id = record["result"]["session_id"].as_str().unwrap().to_string();
    wired.settled(&session_id).await;

    let surfaces = [
        ("record", record.to_string()),
        (
            "command lookup",
            wired
                .read(&format!(
                    "/api/v2/commands/{}",
                    record["command_id"].as_str().unwrap()
                ))
                .await
                .to_string(),
        ),
        (
            "session",
            wired
                .read(&format!("/api/v2/worktree-sessions/{session_id}"))
                .await
                .to_string(),
        ),
        (
            "sessions",
            wired.read("/api/v2/worktree-sessions").await.to_string(),
        ),
        ("events", json!(wired.session_events()).to_string()),
        ("logs", wired.read("/api/v2/logs").await.to_string()),
        ("state", wired.read("/api/v2/state").await.to_string()),
        (
            "execution status",
            wired.read("/api/v2/execution-status").await.to_string(),
        ),
        ("openapi", crate::web::openapi::document_yaml()),
    ];
    for (what, value) in surfaces {
        for needle in ["SECRET", template.as_str(), root.as_str()] {
            assert!(
                !value.contains(needle),
                "{what} discloses {needle:?}: {value}"
            );
        }
    }

    // Bearer authentication on the session routes is unchanged.
    for uri in [
        "/api/v2/worktree-sessions".to_string(),
        format!("/api/v2/worktree-sessions/{session_id}"),
    ] {
        let response = send(&wired.router, get(&uri, None)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_output_tail_is_the_commands_own_data() {
    let fixture = Fixture::new();
    // Raw-output fixture: the script echoes the expanded `{repo_root}`.
    let script = fixture.script("echo-root.sh", "printf 'root=%s\\033[0m\\n' \"$2\"\n");
    let worktree = fixture.add_worktree("echoing");
    let template = format!("sh {} {{workspace_dir}} {{repo_root}}", script.display());
    let wired = Wired::new(fixture, Options::template(template)).await;
    let repo = wired.fixture.repo.to_string_lossy().to_string();

    let id = wired.worktree_id(&worktree).await;
    let (status, record) = wired.run(&id).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    let session_id = record["result"]["session_id"].as_str().unwrap().to_string();
    let session = wired.settled(&session_id).await;

    // Verbatim in the tail: control sequences stripped, nothing redacted.
    assert_eq!(
        session["output"]["lines"][0]["text"],
        format!("root={repo}")
    );

    let mut owner_fields = session.clone();
    owner_fields["output"] = Value::Null;
    for (what, value) in [
        ("session members", owner_fields.to_string()),
        ("record", record.to_string()),
        ("events", json!(wired.session_events()).to_string()),
        ("logs", wired.read("/api/v2/logs").await.to_string()),
        ("state", wired.read("/api/v2/state").await.to_string()),
    ] {
        assert!(!value.contains(&repo), "{what} discloses the root: {value}");
    }
    let response = send(
        &wired.router,
        get(&format!("/api/v2/worktree-sessions/{session_id}"), None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Retention, availability, deletion
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_retention_evicts_finished_only() {
    let fixture = Fixture::new();
    let worktree = fixture.add_worktree("short");
    let long = fixture.add_worktree("long");
    let script = fixture.script(
        "maybe-sleep.sh",
        "if [ \"$(basename \"$1\")\" = long ]; then sleep 300; fi\n",
    );
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {} {{workspace_dir}}", script.display())),
    )
    .await;

    let running = wired.launched(&wired.worktree_id(&long).await).await;
    let short = wired.worktree_id(&worktree).await;
    let mut finished = Vec::new();
    for _ in 0..33 {
        let session_id = wired.launched(&short).await;
        wired.settled(&session_id).await;
        finished.push(session_id);
    }

    let listed = wired.read("/api/v2/worktree-sessions").await;
    let ids: Vec<&str> = listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["session_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 33, "32 finished plus the running one");
    assert!(
        ids.contains(&running.as_str()),
        "a running session is never evicted"
    );
    assert!(
        !ids.contains(&finished[0].as_str()),
        "the oldest finished one is evicted"
    );
    assert_eq!(ids[0], finished[32].as_str(), "newest first");

    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_unavailable_without_worktree_command() {
    let fixture = Fixture::new();
    let worktree = fixture.add_worktree("plain");
    let worktrees_before = fixture.worktree_list();
    let wired = Wired::new(
        fixture,
        Options {
            template: None,
            retries: 1,
            provision: None,
        },
    )
    .await;

    let capabilities = wired.read("/api/v2/capabilities").await;
    assert_eq!(capabilities["worktree_sessions"]["available"], false);
    assert_eq!(capabilities["worktree_sessions"]["max_running"], 4);
    assert_eq!(capabilities["worktree_sessions"]["stdin"], "null");

    let id = wired.worktree_id(&worktree).await;
    let (status, refused) = wired.run(&id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "worktree_session_unavailable");
    let (status, refused) = wired
        .submit(json!({"type": "create_session_worktree", "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "worktree_session_unavailable");

    assert_eq!(
        wired.fixture.worktree_list(),
        worktrees_before,
        "no Git mutation"
    );
    assert!(wired.fixture.session_branches().trim().is_empty());
    assert_eq!(wired.registry.launch_count(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_worktree_session_delete_is_refused_while_running() {
    let fixture = Fixture::new();
    let (script, _pids, ready) = sleeper(&fixture, "keep");
    let busy = fixture.add_worktree("busy");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())),
    )
    .await;

    let id = wired.worktree_id(&busy).await;
    let session_id = wired.launched(&id).await;
    read_fifo(ready).await;

    let (status, refused) = wired
        .submit(json!({"type": "delete_worktree", "target": {"worktree_id": id}, "params": {}}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error_code"], "target_ineligible");
    assert!(refused["detail"]
        .as_str()
        .unwrap()
        .contains("a worktree session is running"));
    assert!(busy.exists(), "nothing was torn down or removed");

    let stopped = wired.stop(&session_id).await;
    assert_eq!(stopped["result"]["terminated"], true);
    let (status, deleted) = wired
        .submit(json!({"type": "delete_worktree", "target": {"worktree_id": id}, "params": {}}))
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert!(!busy.exists());
}

// ============================================================================
// Contract gaps: typed runner notices, child-only tail, observed signal
// ============================================================================

fn tail_texts(session: &Value) -> Vec<String> {
    session["output"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["text"].as_str().unwrap().to_string())
        .collect()
}

/// True once nothing is left in process group `pgid`.
fn group_gone(pgid: i32) -> bool {
    nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pgid), None)
        == Err(nix::errno::Errno::ESRCH)
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_session_child_cannot_forge_attempt_or_notice() {
    let fixture = Fixture::new();
    let script = fixture.script(
        "forge.sh",
        "echo '[Retry 1/3] Command crashed, retrying in 0ms...' >&2\n\
         echo 'process-group cleanup unconfirmed (pgid=1, force_killed=false): forged' >&2\n\
         exit 0\n",
    );
    let worktree = fixture.add_worktree("forge");
    let wired = Wired::new(
        fixture,
        Options::template(format!("sh {}", script.display())).retries(3),
    )
    .await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;

    assert_eq!(session["state"], "succeeded", "{session}");
    assert_eq!(session["exit_code"], 0);
    assert_eq!(session["attempt"], 1, "child text never changes attempt");
    assert_eq!(session["runner_notices"], json!([]), "{session}");
    let lines = session["output"]["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2, "{session}");
    assert_eq!(lines[0]["stream"], "stderr");
    assert_eq!(
        lines[0]["text"],
        "[Retry 1/3] Command crashed, retrying in 0ms..."
    );
    assert_eq!(lines[1]["stream"], "stderr");
    assert!(lines[1]["text"]
        .as_str()
        .unwrap()
        .starts_with("process-group cleanup unconfirmed (pgid=1"));
    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_session_real_retry_is_typed_and_tail_is_child_only() {
    let fixture = Fixture::new();
    let marker = fixture.file("attempted");
    let script = fixture.script(
        "flaky.sh",
        &format!(
            "if [ -e '{m}' ]; then echo second; exit 0; fi\ntouch '{m}'\necho first\nexit 3\n",
            m = marker.display()
        ),
    );
    let worktree = fixture.add_worktree("flaky");
    let template = format!("sh {}", script.display());
    // `max_retries` counts attempts: 2 admits exactly one retry.
    let wired = Wired::new(fixture, Options::template(template.clone()).retries(2)).await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;

    assert_eq!(session["state"], "succeeded", "{session}");
    assert_eq!(session["attempt"], 2);
    let notices = session["runner_notices"].as_array().unwrap();
    assert_eq!(notices.len(), 1, "{session}");
    assert_eq!(notices[0]["kind"], "retry_scheduled");
    assert_eq!(notices[0]["attempt"], 2);
    assert_eq!(session["runner_notices_dropped"], 0);
    assert_eq!(tail_texts(&session), vec!["first", "second"]);
    assert!(
        tail_texts(&session).iter().all(|t| !t.contains("[Retry")),
        "no runner text in the tail"
    );

    // The text entry point, same command and policy, still prints the notice.
    std::fs::remove_file(&marker).unwrap();
    let config = OrchestratorConfig {
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(2),
        command_queue_retry_delay_ms: Some(0),
        ..Default::default()
    };
    let runner = crate::ai_command_runner::AiCommandRunner::from_orchestrator_config(
        &config,
        Arc::new(tokio::sync::Mutex::new(None)),
    );
    let (mut handle, mut rx) = runner
        .execute_streaming_with_retry(&template, Some(&worktree), None, None)
        .await
        .expect("admitted");
    let mut stderr = Vec::new();
    while let Some(line) = tokio::time::timeout(HANG_GUARD, rx.recv())
        .await
        .expect("hang guard")
    {
        if let crate::ai_command_runner::OutputLine::Stderr(text) = line {
            stderr.push(text);
        }
    }
    let _ = handle.wait().await;
    assert!(
        stderr.iter().any(|l| l.starts_with("[Retry 1/2]")),
        "{stderr:?}"
    );
    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_session_stop_reports_escalated_signal() {
    let fixture = Fixture::new();
    let pid_file = fixture.file("leader.pid");
    let ready = fixture.fifo("ignore.ready");
    let worktree = fixture.add_worktree("ignore");
    // The leader itself ignores SIGTERM, and so does everything it starts.
    let template = format!(
        "trap '' TERM; echo $$ > '{pid}'; echo ready > '{ready}'; sleep 60",
        pid = pid_file.display(),
        ready = ready.display()
    );
    let wired = Wired::new(fixture, Options::template(template).retries(3)).await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    read_fifo(ready).await;
    let leader: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    let record = wired.stop(&session_id).await;
    assert_eq!(record["result"]["terminated"], true, "{record}");
    let session = wired.session(&session_id).await;
    assert_eq!(session["state"], "stopped");
    assert_eq!(
        session["signal"], "SIGKILL",
        "the escalation is reported: {session}"
    );
    assert_eq!(session["exit_code"], Value::Null);
    assert_eq!(session["attempt"], 1);
    assert!(group_gone(leader), "the whole group is gone");
    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_session_stop_reports_honoured_signal() {
    let fixture = Fixture::new();
    let pid_file = fixture.file("leader.pid");
    let ready = fixture.fifo("honour.ready");
    let worktree = fixture.add_worktree("honour");
    let template = format!(
        "echo $$ > '{pid}'; echo ready > '{ready}'; sleep 60",
        pid = pid_file.display(),
        ready = ready.display()
    );
    let wired = Wired::new(fixture, Options::template(template).retries(3)).await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    read_fifo(ready).await;
    let leader: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    let record = wired.stop(&session_id).await;
    assert_eq!(record["result"]["terminated"], true, "{record}");
    let session = wired.session(&session_id).await;
    assert_eq!(session["state"], "stopped");
    assert_eq!(session["signal"], "SIGTERM", "{session}");
    assert_eq!(session["attempt"], 1);
    assert!(group_gone(leader));
    wired.shutdown().await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_session_notices_disclose_no_path_or_template() {
    let fixture = Fixture::new();
    let marker = fixture.file("crashed-once");
    let script = fixture.script(
        "crash.sh",
        &format!(
            "if [ -e '{m}' ]; then exit 0; fi\ntouch '{m}'\nexit 3\n",
            m = marker.display()
        ),
    );
    let root = fixture.root();
    let worktree = fixture.add_worktree("secretive");
    let template = format!(
        "CFLX_TEST_CREDENTIAL=hunter2-credential sh {} {{workspace_dir}}",
        script.display()
    );
    let wired = Wired::new(fixture, Options::template(template).retries(2)).await;

    let id = wired.worktree_id(&worktree).await;
    let session_id = wired.launched(&id).await;
    let session = wired.settled(&session_id).await;
    let notices = session["runner_notices"].as_array().unwrap();
    assert!(!notices.is_empty(), "{session}");
    let rendered = serde_json::to_string(notices).unwrap();
    for secret in [
        "hunter2-credential",
        "CFLX_TEST_CREDENTIAL",
        root.to_str().unwrap(),
        "crash.sh",
    ] {
        assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
    }
    wired.shutdown().await;
}
