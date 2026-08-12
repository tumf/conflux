//! Process-boundary tests for the `cflx client` namespace.
//!
//! Everything here drives the **compiled binary** against a **real `/api/v2`
//! router on a real Unix socket**. That combination is the point: the contract
//! this change ships is "an agent runs a command and reads stdout", so a test
//! that called the client's Rust functions directly would prove nothing about
//! argv parsing, exit statuses, stdout/stderr separation, or the socket
//! transport — the four things a caller actually depends on.
//!
//! The router is assembled the way production assembles it, with the shared
//! late-bound executor, gate, execution-facts, and execution-contract handles.
//! Where a test needs to know exactly what the client submitted, the executor is
//! a spy *behind* the real endpoint, so admission, revision checking, gating,
//! idempotency, and settlement are all still the production code paths.
//!
//! Group names match the verification commands in the change's tasks:
//! `cli_surface`, `transport`, `output_contract`, `owner_contract`, `status`,
//! `enqueue`, `wait`, `production_owner_smoke`, `documentation`, and
//! `feature_disabled`.

use std::path::Path;
use std::process::Output;

/// Run the compiled CLI and capture its streams.
///
/// Deliberately the real binary rather than an in-process call: exit status and
/// stream separation are part of the contract under test.
fn run_cli(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"));
    command.args(args).current_dir(cwd);
    // A stray token in the ambient environment must not silently authenticate a
    // test that is asserting the anonymous path.
    command.env_remove("CFLX_CLIENT_TEST_TOKEN");
    for (name, value) in env {
        command.env(name, value);
    }
    command
        .output()
        .expect("the compiled cflx binary must be runnable")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ============================================================================
// Feature-disabled build
// ============================================================================
//
// Compiled in both configurations. Without `web-monitoring` there is no local
// API to speak, so every client command must refuse before it touches anything.

#[test]
fn feature_disabled_or_enabled_client_never_takes_the_repository_lock() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let output = run_cli(tmp.path(), &["client", "status", "--json"], &[]);

    // Whatever the build, a client invocation must leave no owner artifact: no
    // lock file, no socket, no log directory of its own.
    for artifact in ["cflx-owner.json", "cflx-api.sock", ".cflx"] {
        assert!(
            !tmp.path().join(artifact).exists(),
            "client must not create {artifact}"
        );
    }
    assert!(!output.status.success());
}

#[cfg(not(feature = "web-monitoring"))]
mod feature_disabled {
    use super::*;

    #[test]
    fn feature_disabled_client_refuses_before_any_side_effect() {
        let tmp = tempfile::tempdir().expect("temp dir");
        for action in [
            vec!["client", "status", "--json"],
            vec!["client", "enqueue", "alpha", "--json"],
            vec!["client", "wait", "alpha", "--json"],
        ] {
            let output = run_cli(tmp.path(), &action, &[]);
            let stdout = stdout_of(&output);
            let parsed: serde_json::Value =
                serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
                    panic!("stdout must be one JSON envelope, got {stdout:?}: {e}")
                });
            assert_eq!(parsed["ok"], false);
            assert_eq!(parsed["outcome"], "feature_unavailable");
            assert_eq!(output.status.code(), Some(20));
            assert!(
                stderr_of(&output).contains("web-monitoring"),
                "the refusal must name the missing feature"
            );
            assert!(
                std::fs::read_dir(tmp.path())
                    .expect("temp dir readable")
                    .next()
                    .is_none(),
                "a refused client command must write nothing"
            );
        }
    }
}

#[cfg(feature = "web-monitoring")]
mod enabled {
    use super::*;

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;

    use conflux::orchestration::execution_facts::ExecutionFactsStore;
    use conflux::orchestration::operator_command::RunBoundaryLiveness;
    use conflux::web::remote_control_api::auth::RemoteControlAuth;
    use conflux::web::remote_control_api::dto::{
        CommandSpec, ErrorCode, OwnerExecutionContract, TerminalMode,
    };
    use conflux::web::remote_control_api::executor::{
        CommandFailure, ExecutionSummary, RemoteControlExecutor,
    };
    use conflux::web::remote_control_api::projection::Projection;
    use conflux::web::remote_control_api::{router, RemoteControlRuntime, RemoteControlState};

    // ────────────────────────────────────────────────────────────────────────
    // Doubles
    // ────────────────────────────────────────────────────────────────────────

    /// Records every command the endpoint delegated, and answers on cue.
    ///
    /// It sits *behind* the real command endpoint, so a test asserting "zero
    /// commands were submitted" is asserting about the production admission
    /// path, not about a stub the client talked to directly.
    #[derive(Default)]
    struct SpyExecutor {
        calls: Mutex<Vec<CommandSpec>>,
        /// Outcome for the next call, popped in order; the last one repeats.
        script: Mutex<Vec<Result<ExecutionSummary, CommandFailure>>>,
    }

    impl SpyExecutor {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn script(&self, outcomes: Vec<Result<ExecutionSummary, CommandFailure>>) {
            *self.script.lock().unwrap() = outcomes;
        }

        fn calls(&self) -> Vec<CommandSpec> {
            self.calls.lock().unwrap().clone()
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl RemoteControlExecutor for SpyExecutor {
        async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
            self.calls.lock().unwrap().push(command.clone());
            let mut script = self.script.lock().unwrap();
            if script.len() > 1 {
                script.remove(0)
            } else {
                script
                    .first()
                    .cloned()
                    .unwrap_or_else(|| Ok(ExecutionSummary::changed("applied")))
            }
        }
    }

    /// Scheduler-liveness double: separate from execution facts on purpose, so a
    /// live-but-idle owner and a dead one stay distinguishable.
    #[derive(Default)]
    struct Boundary {
        running: AtomicBool,
    }

    impl Boundary {
        fn set_running(&self, running: bool) {
            self.running.store(running, Ordering::SeqCst);
        }
    }

    impl RunBoundaryLiveness for Boundary {
        fn boundary_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }
    }

    /// What one `POST /api/v2/commands` asked for and what the owner answered.
    #[derive(Debug, Clone)]
    struct CommandExchange {
        command_type: String,
        expected_revision: u64,
        idempotency_key: String,
        status: u16,
        /// `Some` when the owner answered with a command record — its own proof
        /// that the command was admitted for execution, which is exactly what
        /// the client's audit claims to list.
        record_id: Option<String>,
        /// `Some` when the owner refused with a typed error instead.
        error_code: Option<String>,
    }

    /// A recorder and injection point wrapped *around* the real `/api/v2` router.
    ///
    /// It replaces no part of the production path: every request it sees is one
    /// the compiled client actually sent, every response it records is one the
    /// production handlers produced, and an advance it injects lands strictly
    /// between the client's observation and the endpoint's own revision check.
    /// That window is where a stale revision is born, so a test can force one by
    /// request ordering alone — no sleeps, no wall-clock races.
    #[derive(Default)]
    struct ApiSpy {
        /// `METHOD path` for every request, in arrival order.
        requests: Mutex<Vec<String>>,
        /// One entry per `POST /api/v2/commands`, in submission order.
        exchanges: Mutex<Vec<CommandExchange>>,
        /// Advances applied just before each command POST reaches the handler,
        /// popped in order; an exhausted script injects nothing.
        before_command: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
    }

    impl ApiSpy {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }

        fn inject_before_commands(&self, script: Vec<Box<dyn Fn() + Send + Sync>>) {
            *self.before_command.lock().unwrap() = script;
        }

        fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }

        fn exchanges(&self) -> Vec<CommandExchange> {
            self.exchanges.lock().unwrap().clone()
        }

        async fn intercept(
            self: Arc<Self>,
            request: axum::extract::Request,
            next: axum::middleware::Next,
        ) -> axum::response::Response {
            let method = request.method().to_string();
            let path = request.uri().path().to_string();
            self.requests
                .lock()
                .unwrap()
                .push(format!("{method} {path}"));

            // Everything else is forwarded untouched: buffering a body here
            // would break the streaming endpoints this router also serves.
            if method != "POST" || path != "/api/v2/commands" {
                return next.run(request).await;
            }

            let (parts, body) = request.into_parts();
            let submitted_bytes = axum::body::to_bytes(body, usize::MAX)
                .await
                .expect("the command request body is readable");
            let submitted: serde_json::Value =
                serde_json::from_slice(&submitted_bytes).expect("the command request body is JSON");

            let advance = {
                let mut script = self.before_command.lock().unwrap();
                (!script.is_empty()).then(|| script.remove(0))
            };
            if let Some(advance) = advance {
                advance();
            }

            let response = next
                .run(axum::extract::Request::from_parts(
                    parts,
                    axum::body::Body::from(submitted_bytes),
                ))
                .await;
            let status = response.status().as_u16();
            let (parts, body) = response.into_parts();
            let answered_bytes = axum::body::to_bytes(body, usize::MAX)
                .await
                .expect("the command response body is readable");
            let answered: serde_json::Value =
                serde_json::from_slice(&answered_bytes).unwrap_or(serde_json::Value::Null);

            self.exchanges.lock().unwrap().push(CommandExchange {
                // `CommandRequest` flattens its `CommandSpec`, so the
                // discriminant is a top-level `type` on the wire.
                command_type: submitted["type"].as_str().unwrap_or_default().to_string(),
                expected_revision: submitted["expected_revision"].as_u64().unwrap_or_default(),
                idempotency_key: submitted["idempotency_key"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                status,
                record_id: answered["command_id"].as_str().map(str::to_string),
                error_code: answered["error_code"].as_str().map(str::to_string),
            });

            axum::response::Response::from_parts(parts, axum::body::Body::from(answered_bytes))
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Owner fixture
    // ────────────────────────────────────────────────────────────────────────

    /// A real `/api/v2` router on a real Unix socket, plus the handles a test
    /// needs to drive it.
    struct Owner {
        socket: PathBuf,
        projection: Arc<Projection>,
        runtime: Arc<RemoteControlRuntime>,
        boundary: Arc<Boundary>,
        shutdown: tokio_util::sync::CancellationToken,
        task: tokio::task::JoinHandle<()>,
        _dir: Option<tempfile::TempDir>,
    }

    impl Owner {
        /// Start an owner. `executor` is `None` for a read-only process, which is
        /// exactly the shape of a headless `cflx run`.
        async fn start(
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
        ) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let socket = dir.path().join("cflx-api.sock");
            let mut owner = Self::start_on(socket, executor, token).await;
            owner._dir = Some(dir);
            owner
        }

        /// Start an owner whose real router is wrapped by a recording layer.
        ///
        /// Used where a test has to know what the client put on the wire, or has
        /// to make the owner advance at one exact point in the exchange.
        async fn start_intercepted(
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
            api: Arc<ApiSpy>,
        ) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let socket = dir.path().join("cflx-api.sock");
            let mut owner = Self::start_layered(socket, executor, token, Some(api)).await;
            owner._dir = Some(dir);
            owner
        }

        /// Start an owner on a caller-owned socket path.
        ///
        /// Used where a test has to replace the process serving one endpoint, so
        /// the path has to outlive the first incarnation.
        async fn start_on(
            socket: PathBuf,
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
        ) -> Self {
            Self::start_layered(socket, executor, token, None).await
        }

        async fn start_layered(
            socket: PathBuf,
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
            api: Option<Arc<ApiSpy>>,
        ) -> Self {
            let runtime = Arc::new(RemoteControlRuntime::new());
            if let Some(executor) = executor {
                runtime.bind(executor).await;
            }
            let facts = Arc::new(ExecutionFactsStore::new());
            runtime.bind_execution_facts(facts);
            let boundary = Arc::new(Boundary::default());
            runtime.bind_run_boundary(boundary.clone());

            let auth = RemoteControlAuth::new(token.map(str::to_string), &[])
                .expect("test auth policy is valid");
            let app = router(
                RemoteControlState::new(runtime.projection(), Arc::new(auth), runtime.clone())
                    .with_gate(runtime.gate())
                    .with_execution_facts(runtime.execution_facts())
                    .with_execution_contract(runtime.execution_contract()),
            );
            let app = match api {
                Some(api) => app.layer(axum::middleware::from_fn(
                    move |request: axum::extract::Request, next: axum::middleware::Next| {
                        let api = api.clone();
                        async move { api.intercept(request, next).await }
                    },
                )),
                None => app,
            };

            let listener = tokio::net::UnixListener::bind(&socket).expect("binds the test socket");
            let shutdown = tokio_util::sync::CancellationToken::new();
            let token_for_task = shutdown.clone();
            let task = tokio::spawn(async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move { token_for_task.cancelled().await })
                    .await;
            });

            Self {
                socket,
                projection: runtime.projection(),
                runtime,
                boundary,
                shutdown,
                task,
                _dir: None,
            }
        }

        fn socket(&self) -> String {
            self.socket.display().to_string()
        }

        /// Publish a snapshot, advancing the revision exactly as the owner does.
        fn publish(&self, snapshot: conflux::web::remote_control_api::dto::InstanceSnapshot) {
            self.projection
                .apply_state("test_snapshot", None, serde_json::json!({}), snapshot);
        }

        fn contract(&self, contract: OwnerExecutionContract) {
            self.runtime.bind_execution_contract(contract);
        }

        async fn stop(self) {
            self.shutdown.cancel();
            let _ = self.task.await;
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Snapshot builders
    // ────────────────────────────────────────────────────────────────────────

    use conflux::web::remote_control_api::dto::{
        AttentionState, ChangeResource, ChangeTiming, InstanceSnapshot, ParallelEligibility,
        ParallelRuntimeState, QueueIntent, SnapshotTotals,
    };

    /// One projected change with real action eligibility for `app_mode`.
    ///
    /// Built through the same classifier the server publishes with, so a fixture
    /// can never advertise a route production would refuse.
    fn change(id: &str, app_mode: &str, display_status: &str) -> ChangeResource {
        ChangeResource {
            id: id.to_string(),
            display_status: display_status.to_string(),
            progress_status: "pending".to_string(),
            completed_tasks: 0,
            total_tasks: 2,
            progress_percent: 0.0,
            dependencies: Vec::new(),
            iteration_number: None,
            execution_marked: false,
            queue_intent: QueueIntent::NotQueued,
            attention: AttentionState::None,
            blocker: None,
            error_detail: None,
            actions: conflux::web::remote_control_api::projection::change_actions_for_test(
                app_mode,
                display_status,
                None,
            ),
            parallel: ParallelEligibility::default(),
            timing: ChangeTiming::default(),
            latest_activity: None,
            worktree: None,
        }
    }

    fn snapshot(app_mode: &str, changes: Vec<ChangeResource>) -> InstanceSnapshot {
        let total = changes.len();
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
        }
    }

    fn merged_contract(base: &str) -> OwnerExecutionContract {
        OwnerExecutionContract {
            base_branch: base.to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        }
    }

    /// Parse the single JSON envelope a `--json` invocation must emit.
    fn envelope(output: &Output) -> serde_json::Value {
        let stdout = stdout_of(output);
        let trimmed = stdout.trim();
        assert!(
            !trimmed.contains('\n'),
            "JSON stdout must be exactly one object, got:\n{stdout}"
        );
        serde_json::from_str(trimmed)
            .unwrap_or_else(|e| panic!("stdout must be one JSON envelope, got {stdout:?}: {e}"))
    }

    /// Where the CLI runs from when the test does not care about a repository.
    fn neutral_cwd() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    // ========================================================================
    // cli_surface
    // ========================================================================

    #[test]
    fn cli_surface_exposes_only_status_enqueue_and_wait() {
        let tmp = neutral_cwd();
        let output = run_cli(tmp.path(), &["client", "--help"], &[]);
        assert!(output.status.success(), "{}", stderr_of(&output));
        let help = stdout_of(&output);
        for expected in [
            "status",
            "enqueue",
            "wait",
            "--unix-socket",
            "--auth-token-env",
        ] {
            assert!(
                help.contains(expected),
                "help must mention {expected}:\n{help}"
            );
        }
        // The protocol must not leak into the public surface: these are the
        // exact spellings a caller would otherwise be tempted to construct.
        for forbidden in [
            "--expected-revision",
            "--idempotency-key",
            "--command-type",
            "--queue-intent",
            "--execution-mark",
            "--auth-token ",
        ] {
            assert!(
                !help.contains(forbidden),
                "help must not expose {forbidden}:\n{help}"
            );
        }
        for absent in ["stop", "retry", "resolve", "worktree"] {
            assert!(
                !help.contains(&format!("  {absent} ")),
                "client must not offer a {absent} subcommand:\n{help}"
            );
        }
    }

    #[test]
    fn cli_surface_rejects_a_malformed_change_id_as_usage() {
        let tmp = neutral_cwd();
        for bad in ["../escape", ".hidden", "-leading", "has space", ""] {
            let output = run_cli(tmp.path(), &["client", "enqueue", bad, "--json"], &[]);
            assert_eq!(
                output.status.code(),
                Some(2),
                "'{bad}' must be a usage error, stderr={}",
                stderr_of(&output)
            );
            // A rejected `--json` invocation still owes the caller its one
            // envelope: `usage_error` is an outcome, not an escape from the
            // machine contract.
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "usage_error");
            assert_eq!(parsed["operation"], "enqueue");
        }
    }

    #[test]
    fn cli_surface_rejects_a_malformed_timeout_as_usage() {
        let tmp = neutral_cwd();
        for bad in [
            "0s",
            "0ms",
            "50ms",
            "abc",
            "5x",
            "-1",
            "99999999999999h",
            "",
        ] {
            let output = run_cli(
                tmp.path(),
                &["client", "wait", "alpha", "--timeout", bad, "--json"],
                &[],
            );
            assert_eq!(
                output.status.code(),
                Some(2),
                "'{bad}' must be a usage error, stderr={}",
                stderr_of(&output)
            );
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "usage_error");
            assert_eq!(parsed["operation"], "wait");
        }
    }

    #[test]
    fn cli_surface_accepts_the_documented_duration_spellings() {
        // The parser is exercised through the binary, so an accepted spelling is
        // one a caller can actually type. `owner_not_running` proves parsing got
        // all the way past argv.
        let tmp = neutral_cwd();
        let socket = tmp.path().join("absent.sock");
        for good in ["30", "500ms", "30s", "45m", "2h"] {
            let output = run_cli(
                tmp.path(),
                &[
                    "client",
                    "--unix-socket",
                    &socket.display().to_string(),
                    "wait",
                    "alpha",
                    "--timeout",
                    good,
                    "--json",
                ],
                &[],
            );
            assert_eq!(
                envelope(&output)["outcome"],
                "owner_not_running",
                "'{good}' must parse"
            );
        }
    }

    #[test]
    fn cli_surface_outside_a_repository_names_the_socket_option() {
        let tmp = neutral_cwd();
        // A temp dir that happens to sit inside a repository would resolve a
        // default socket, so this only asserts the refusal when there is truly no
        // repository identity to derive one from.
        let output = run_cli(tmp.path(), &["client", "status", "--json"], &[]);
        let parsed = envelope(&output);
        if parsed["outcome"] == "not_in_repository" {
            assert_eq!(output.status.code(), Some(3));
            assert!(stderr_of(&output).contains("--unix-socket"));
        }
    }

    // ========================================================================
    // json_usage_errors
    // ========================================================================
    //
    // The machine contract has to hold on the path a caller hits most often
    // while wiring an agent up: a typo. Before this, `cflx client ... --json`
    // exited through Clap with an empty stdout, so the one thing the contract
    // promised — exactly one parseable envelope — was false precisely when the
    // caller had nothing else to branch on.

    #[test]
    fn json_usage_errors_emit_one_envelope_for_every_rejected_client_invocation() {
        let tmp = neutral_cwd();
        for (args, operation, what) in [
            (
                vec!["client", "enqueue", "../escape", "--json"],
                "enqueue",
                "an invalid change ID",
            ),
            (
                vec!["client", "enqueue", "", "--json"],
                "enqueue",
                "an empty change ID",
            ),
            (
                vec!["client", "wait", "alpha", "--timeout", "abc", "--json"],
                "wait",
                "an unparseable timeout",
            ),
            (
                vec!["client", "wait", "alpha", "--timeout", "0s", "--json"],
                "wait",
                "a zero timeout",
            ),
            (
                vec!["client", "enqueue", "--json"],
                "enqueue",
                "a missing required argument",
            ),
            (
                vec!["client", "status", "--json", "--not-an-option"],
                "status",
                "an unknown client option",
            ),
            (
                vec!["client", "--json"],
                "status",
                "the namespace with no operation",
            ),
        ] {
            let output = run_cli(tmp.path(), &args, &[]);
            // `envelope` itself asserts stdout is exactly one JSON object.
            let parsed = envelope(&output);
            assert_eq!(parsed["schema_version"], 1, "{what}");
            assert_eq!(parsed["ok"], false, "{what}");
            assert_eq!(parsed["outcome"], "usage_error", "{what}");
            assert_eq!(parsed["operation"], operation, "{what}");
            assert!(parsed["detail"].is_object(), "{what}");
            assert_eq!(output.status.code(), Some(2), "{what}");
            // The reason is a diagnostic, so it belongs on stderr as well.
            assert!(
                stderr_of(&output).contains("usage_error"),
                "{what}: stderr={}",
                stderr_of(&output)
            );
            // Nothing was initialized: no lock file, no socket, no log directory.
            assert!(
                std::fs::read_dir(tmp.path())
                    .expect("temp dir readable")
                    .next()
                    .is_none(),
                "{what}: a rejected invocation must write nothing"
            );
        }
    }

    #[test]
    fn json_usage_errors_leave_human_and_non_client_parse_failures_alone() {
        let tmp = neutral_cwd();
        // Human mode: Clap's own diagnostics, and no envelope on stdout.
        for args in [
            vec!["client", "enqueue", "../escape"],
            vec!["client", "wait", "alpha", "--timeout", "abc"],
            vec!["client", "status", "--not-an-option"],
        ] {
            let output = run_cli(tmp.path(), &args, &[]);
            assert!(
                stdout_of(&output).trim().is_empty(),
                "a human parse failure must not print an envelope: {}",
                stdout_of(&output)
            );
            assert!(!output.status.success());
            assert!(
                stderr_of(&output).contains("error:"),
                "Clap's human diagnostic must survive: {}",
                stderr_of(&output)
            );
        }

        // Another namespace's parse failure is not a client result, even though
        // `--json` appears in argv.
        let unrelated = run_cli(
            tmp.path(),
            &["openspec", "show", "alpha", "--not-an-option", "--json"],
            &[],
        );
        assert!(
            stdout_of(&unrelated).trim().is_empty(),
            "an unrelated top-level failure must not be rewritten as a client envelope: {}",
            stdout_of(&unrelated)
        );
        assert!(!unrelated.status.success());

        // A *value* that merely contains the spelling is data, not intent.
        let substring = run_cli(
            tmp.path(),
            &[
                "client",
                "--unix-socket",
                "/tmp/holds--json-in-its-name.sock",
                "enqueue",
                "../escape",
            ],
            &[],
        );
        assert!(
            stdout_of(&substring).trim().is_empty(),
            "'--json' inside a value must not select JSON mode: {}",
            stdout_of(&substring)
        );
        assert_eq!(substring.status.code(), Some(2));
    }

    #[test]
    fn json_usage_errors_never_rewrite_help_or_version() {
        let tmp = neutral_cwd();
        // Help is an answer, not a usage failure, even alongside `--json`.
        let help = run_cli(tmp.path(), &["client", "status", "--help", "--json"], &[]);
        assert!(help.status.success(), "{}", stderr_of(&help));
        assert!(
            stdout_of(&help).contains("Usage:"),
            "help must still be help: {}",
            stdout_of(&help)
        );

        let version = run_cli(tmp.path(), &["--version"], &[]);
        assert!(version.status.success());
        assert!(!stdout_of(&version).contains("usage_error"));
    }

    // ========================================================================
    // transport
    // ========================================================================

    #[tokio::test]
    async fn transport_reaches_a_real_owner_over_an_explicit_unix_socket() {
        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();

        let socket = owner.socket();
        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();

        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "observed");
        assert_eq!(parsed["ok"], true);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(parsed["detail"]["changes"][0]["id"], "alpha");
        owner.stop().await;
    }

    #[test]
    fn transport_reports_owner_not_running_for_an_absent_socket() {
        let tmp = neutral_cwd();
        let socket = tmp.path().join("nobody.sock");
        let output = run_cli(
            tmp.path(),
            &[
                "client",
                "--unix-socket",
                &socket.display().to_string(),
                "status",
                "--json",
            ],
            &[],
        );
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "owner_not_running");
        assert_eq!(output.status.code(), Some(4));
        assert!(!socket.exists(), "a client must never create the socket");
    }

    #[tokio::test]
    async fn transport_presents_the_environment_token_and_never_prints_it() {
        let owner = Owner::start(Some(SpyExecutor::new()), Some("s3cret-token")).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let cwd = tmp.path().to_path_buf();
        let authorized = tokio::task::spawn_blocking({
            let socket = socket.clone();
            let cwd = cwd.clone();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "--auth-token-env",
                        "CFLX_CLIENT_TEST_TOKEN",
                        "status",
                        "--json",
                    ],
                    &[("CFLX_CLIENT_TEST_TOKEN", "s3cret-token")],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&authorized)["outcome"], "observed");
        assert!(
            !stdout_of(&authorized).contains("s3cret-token")
                && !stderr_of(&authorized).contains("s3cret-token"),
            "the token must never appear in either stream"
        );

        let anonymous = tokio::task::spawn_blocking({
            let socket = socket.clone();
            let cwd = cwd.clone();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&anonymous)["outcome"], "authentication_failed");
        assert_eq!(anonymous.status.code(), Some(5));

        let wrong = tokio::task::spawn_blocking({
            let socket = socket.clone();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "--auth-token-env",
                        "CFLX_CLIENT_TEST_TOKEN",
                        "status",
                        "--json",
                    ],
                    &[("CFLX_CLIENT_TEST_TOKEN", "wrong-token")],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&wrong)["outcome"], "authentication_failed");
        assert!(!stderr_of(&wrong).contains("wrong-token"));
        owner.stop().await;
    }

    #[test]
    fn transport_refuses_an_unset_token_variable_without_connecting() {
        let tmp = neutral_cwd();
        let socket = tmp.path().join("nobody.sock");
        let output = run_cli(
            tmp.path(),
            &[
                "client",
                "--unix-socket",
                &socket.display().to_string(),
                "--auth-token-env",
                "CFLX_CLIENT_TEST_TOKEN",
                "status",
                "--json",
            ],
            &[],
        );
        // Fails closed on the *client's* misconfiguration rather than reporting
        // the owner's eventual 401 as if the owner were at fault.
        assert_eq!(envelope(&output)["outcome"], "authentication_failed");
    }

    #[tokio::test]
    async fn transport_reports_an_incompatible_owner_for_a_non_v2_endpoint() {
        // Something is listening and answering HTTP, but it is not this API.
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("impostor.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let app = axum::Router::new().route(
            "/api/v2/capabilities",
            axum::routing::get(|| async { "not a capabilities document" }),
        );
        let shutdown = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move { shutdown.cancelled().await })
                    .await;
            }
        });

        let tmp = neutral_cwd();
        let path = socket.display().to_string();
        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &path, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&output)["outcome"], "incompatible_owner");
        assert_eq!(output.status.code(), Some(6));
        shutdown.cancel();
        let _ = task.await;
    }

    // ------------------------------------------------------------------------
    // auth_header_validation
    // ------------------------------------------------------------------------
    //
    // The token is opaque to this client but not to HTTP. A CR or LF inside it
    // ends the `Authorization` header early and lets the rest be read as another
    // header, so the check has to happen before a connection exists — "no bytes
    // were written" is the property, and a counting listener is how it is
    // proven rather than asserted.

    #[tokio::test]
    async fn auth_header_validation_refuses_a_malformed_token_before_connecting() {
        let dir = tempfile::tempdir().expect("temp dir");
        let socket = dir.path().join("counting.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("binds the counting socket");
        let accepted = Arc::new(AtomicUsize::new(0));
        let shutdown = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn({
            let accepted = accepted.clone();
            let stop = shutdown.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        incoming = listener.accept() => {
                            if incoming.is_ok() {
                                accepted.fetch_add(1, Ordering::SeqCst);
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
        });

        let path = socket.display().to_string();
        for (token, what) in [
            ("s3cret\r\nX-Injected: yes", "a CRLF header injection"),
            ("s3cret\n", "a trailing line feed"),
            ("s3cret\rmore", "a bare carriage return"),
            ("s3cret\u{7f}", "DEL"),
            ("s3\u{1}cret", "another C0 control"),
            ("s3\tcret", "a horizontal tab"),
        ] {
            let output = tokio::task::spawn_blocking({
                let path = path.clone();
                let dir = dir.path().to_path_buf();
                let token = token.to_string();
                move || {
                    run_cli(
                        &dir,
                        &[
                            "client",
                            "--unix-socket",
                            &path,
                            "--auth-token-env",
                            "CFLX_CLIENT_TEST_TOKEN",
                            "status",
                            "--json",
                        ],
                        &[("CFLX_CLIENT_TEST_TOKEN", &token)],
                    )
                }
            })
            .await
            .unwrap();

            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "authentication_failed", "{what}");
            assert_eq!(output.status.code(), Some(5), "{what}");
            let stdout = stdout_of(&output);
            let stderr = stderr_of(&output);
            for stream in [&stdout, &stderr] {
                assert!(
                    !stream.contains("s3"),
                    "{what}: no fragment of the token value may be shown: {stream}"
                );
                assert!(
                    !stream.contains("X-Injected"),
                    "{what}: an injection attempt must not be echoed: {stream}"
                );
            }
            // The variable is named so an operator can fix it; the value is not.
            assert!(
                stderr.contains("CFLX_CLIENT_TEST_TOKEN"),
                "{what}: {stderr}"
            );
        }

        assert_eq!(
            accepted.load(Ordering::SeqCst),
            0,
            "a malformed token must be refused before any connection is opened"
        );
        shutdown.cancel();
        let _ = task.await;
    }

    #[tokio::test]
    async fn auth_header_validation_keeps_a_valid_token_authenticating() {
        // Punctuation-heavy but entirely legal as a header value, so the check
        // cannot have been implemented as an alphanumeric allow-list.
        const TOKEN: &str = "tok.en-plus~/+=:_9";
        let owner = Owner::start(Some(SpyExecutor::new()), Some(TOKEN)).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "--auth-token-env",
                        "CFLX_CLIENT_TEST_TOKEN",
                        "status",
                        "--json",
                    ],
                    &[("CFLX_CLIENT_TEST_TOKEN", TOKEN)],
                )
            }
        })
        .await
        .unwrap();

        assert_eq!(envelope(&output)["outcome"], "observed");
        assert_eq!(output.status.code(), Some(0));
        assert!(
            !stdout_of(&output).contains(TOKEN) && !stderr_of(&output).contains(TOKEN),
            "a valid token still must never be printed"
        );
        owner.stop().await;
    }

    // ========================================================================
    // output_contract
    // ========================================================================

    #[tokio::test]
    async fn output_contract_keeps_json_stdout_clean_and_diagnostics_on_stderr() {
        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        // Success: stdout is one object, stderr is empty.
        let success = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            let socket = socket.clone();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let parsed = envelope(&success);
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["operation"], "status");
        assert!(parsed["detail"].is_object());
        assert!(
            stderr_of(&success).trim().is_empty(),
            "a successful run must print no diagnostics: {}",
            stderr_of(&success)
        );

        // Failure: the envelope is still the only thing on stdout.
        let failure = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            let socket = socket.clone();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "enqueue",
                        "ghost",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let parsed = envelope(&failure);
        assert_eq!(parsed["ok"], false);
        assert_eq!(parsed["outcome"], "change_not_found");
        assert_eq!(parsed["change_id"], "ghost");
        assert_eq!(failure.status.code(), Some(9));
        assert!(
            stderr_of(&failure).contains("change_not_found"),
            "the human diagnostic belongs on stderr"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn output_contract_human_mode_is_one_concise_line() {
        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || run_cli(&cwd, &["client", "--unix-socket", &socket, "status"], &[])
        })
        .await
        .unwrap();
        let stdout = stdout_of(&output);
        assert_eq!(stdout.lines().count(), 1, "human output must be one line");
        assert!(stdout.starts_with("status: observed"), "{stdout}");
        // Human mode must not accidentally emit the machine contract.
        assert!(!stdout.contains("schema_version"));
        owner.stop().await;
    }

    #[tokio::test]
    async fn output_contract_maps_every_reached_outcome_to_its_documented_exit_code() {
        let tmp = neutral_cwd();
        let cwd = tmp.path().to_path_buf();

        // (bind an executor, args, outcome, exit code) — every pair asserted
        // through the binary, so the mapping is executable behavior rather than
        // a table in a comment. The command-capability case needs an owner with
        // no executor, which is the shape of a headless `cflx run`.
        let cases: Vec<(bool, Vec<String>, &str, i32)> = vec![
            (
                false,
                vec!["enqueue".into(), "alpha".into()],
                "owner_not_command_capable",
                7,
            ),
            (
                true,
                vec!["enqueue".into(), "ghost".into()],
                "change_not_found",
                9,
            ),
            (
                true,
                vec![
                    "wait".into(),
                    "alpha".into(),
                    "--timeout".into(),
                    "300ms".into(),
                ],
                "unsupported_terminal_mode",
                16,
            ),
        ];

        for (bound, action, outcome, code) in cases {
            let executor: Option<Arc<dyn RemoteControlExecutor>> = if bound {
                Some(SpyExecutor::new())
            } else {
                None
            };
            let owner = Owner::start(executor, None).await;
            owner.publish(snapshot(
                "select",
                vec![change("alpha", "select", "not queued")],
            ));
            let socket = owner.socket();
            let cwd = cwd.clone();
            let output = tokio::task::spawn_blocking(move || {
                let mut args = vec!["client".to_string(), "--unix-socket".to_string(), socket];
                args.extend(action);
                args.push("--json".to_string());
                let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
                run_cli(&cwd, &borrowed, &[])
            })
            .await
            .unwrap();
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], outcome);
            assert_eq!(output.status.code(), Some(code), "outcome={outcome}");
            owner.stop().await;
        }
    }

    // ========================================================================
    // owner_contract
    // ========================================================================

    #[tokio::test]
    async fn owner_contract_publishes_each_terminal_mode_and_omits_inapplicable_fields() {
        for (contract, expected_mode, expects_remote, expects_branch) in [
            (
                OwnerExecutionContract::resolve("main", None, None),
                "merged",
                false,
                false,
            ),
            (
                OwnerExecutionContract::resolve("main", None, Some("upstream")),
                "base_published",
                true,
                false,
            ),
            (
                OwnerExecutionContract::resolve("main", Some("origin"), None),
                "branch_pushed",
                true,
                true,
            ),
        ] {
            let owner = Owner::start(Some(SpyExecutor::new()), None).await;
            owner.publish(snapshot(
                "select",
                vec![change("alpha", "select", "not queued")],
            ));
            owner.contract(contract);
            let tmp = neutral_cwd();
            let socket = owner.socket();

            let output = tokio::task::spawn_blocking({
                let cwd = tmp.path().to_path_buf();
                move || {
                    run_cli(
                        &cwd,
                        &["client", "--unix-socket", &socket, "status", "--json"],
                        &[],
                    )
                }
            })
            .await
            .unwrap();

            let published = envelope(&output)["detail"]["execution_contract"].clone();
            assert_eq!(published["terminal_mode"], expected_mode);
            assert_eq!(published["base_branch"], "main");
            assert_eq!(
                published.get("remote").is_some(),
                expects_remote,
                "mode {expected_mode} remote presence"
            );
            // `status` names no change, so even `branch_pushed` publishes no
            // branch: the derivation is change-scoped and stays server-side.
            assert!(
                published.get("pushed_branch").is_none(),
                "an unscoped read must not invent a branch"
            );
            let _ = expects_branch;
            owner.stop().await;
        }
    }

    #[tokio::test]
    async fn owner_contract_joins_the_revision_and_incarnation_of_the_snapshot() {
        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        owner.contract(merged_contract("main"));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let parsed = envelope(&output);
        assert_eq!(
            parsed["instance_id"].as_str().unwrap(),
            owner.projection.instance_id(),
            "the envelope must name the incarnation it observed"
        );
        assert_eq!(
            parsed["detail"]["state_revision"].as_u64().unwrap(),
            owner.projection.revision(),
            "the contract must be joined at the snapshot's revision"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn owner_contract_reports_command_capability_and_a_distinct_unbound_error() {
        let unbound = Owner::start(None, None).await;
        unbound.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = unbound.socket();

        let status = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            let socket = socket.clone();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(
            envelope(&status)["detail"]["command_execution_available"],
            false
        );

        let enqueue = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "enqueue",
                        "alpha",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&enqueue)["outcome"], "owner_not_command_capable");
        assert_eq!(enqueue.status.code(), Some(7));
        unbound.stop().await;
    }

    #[test]
    fn owner_contract_error_code_is_its_own_wire_token() {
        // The distinction the client depends on: an unbound executor never
        // clears within an incarnation, while a lifecycle conflict can.
        assert_eq!(
            ErrorCode::CommandExecutorUnbound.as_str(),
            "command_executor_unbound"
        );
        assert_ne!(
            ErrorCode::CommandExecutorUnbound.as_str(),
            ErrorCode::LifecycleConflict.as_str()
        );
        assert!(conflux::web::remote_control_api::dto::ALL_ERROR_CODES
            .contains(&ErrorCode::CommandExecutorUnbound));
    }

    // ========================================================================
    // status
    // ========================================================================

    #[tokio::test]
    async fn status_is_read_only_and_submits_no_command() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![
                change("alpha", "running", "applying"),
                change("beta", "running", "not queued"),
            ],
        ));
        owner.boundary.set_running(true);
        owner.contract(merged_contract("main"));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();

        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "observed");
        assert_eq!(parsed["detail"]["process"]["app_mode"], "running");
        assert_eq!(parsed["detail"]["process"]["scheduler_running"], true);
        assert_eq!(parsed["detail"]["changes"].as_array().unwrap().len(), 2);
        assert_eq!(spy.call_count(), 0, "status must submit no command");
        owner.stop().await;
    }

    #[tokio::test]
    async fn status_reconciles_a_snapshot_that_advances_between_reads() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        // Advance the owner while the client is mid-read. The reads are separate
        // resources, so this is exactly the race the reconciliation exists for:
        // the client must either reread coherently or say `observation_conflict`,
        // never stitch two revisions together.
        let churn_projection = owner.projection.clone();
        let stop_churn = Arc::new(AtomicBool::new(false));
        let churn_flag = stop_churn.clone();
        let churn = tokio::spawn(async move {
            let mut n = 0usize;
            while !churn_flag.load(Ordering::SeqCst) {
                n += 1;
                churn_projection.apply_state(
                    "test_snapshot",
                    None,
                    serde_json::json!({}),
                    snapshot(
                        "select",
                        vec![change(&format!("alpha{}", n % 2), "select", "not queued")],
                    ),
                );
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        });

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        stop_churn.store(true, Ordering::SeqCst);
        let _ = churn.await;

        let parsed = envelope(&output);
        let outcome = parsed["outcome"].as_str().unwrap();
        assert!(
            outcome == "observed" || outcome == "observation_conflict",
            "a racing read must reconcile or report a typed conflict, got {outcome}"
        );
        if outcome == "observation_conflict" {
            assert_eq!(output.status.code(), Some(13));
        }
        assert_eq!(
            spy.call_count(),
            0,
            "even a conflicted status submits nothing"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn status_reports_a_missing_owner_contract_without_failing() {
        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let output = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "status", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "observed");
        assert!(parsed["detail"]["execution_contract"].is_null());
        owner.stop().await;
    }

    // ========================================================================
    // enqueue
    // ========================================================================

    /// One endpoint that starts serving a different owner after the first POST.
    ///
    /// A byte relay in front of two *real* routers rather than a scripted JSON
    /// server: the point is that the client sees genuine v2 responses from two
    /// genuine incarnations, which is the only way "the socket is now a
    /// different process" is reproducible in-process.
    async fn switch_after_first_post(
        front: PathBuf,
        first: PathBuf,
        second: PathBuf,
    ) -> tokio_util::sync::CancellationToken {
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::UnixListener::bind(&front).expect("binds the relay socket");
        let cancel = tokio_util::sync::CancellationToken::new();
        let stop = cancel.clone();
        tokio::spawn(async move {
            let switched = Arc::new(AtomicBool::new(false));
            loop {
                let accepted = tokio::select! {
                    _ = stop.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                let Ok((mut client, _)) = accepted else { break };
                let switched = switched.clone();
                let first = first.clone();
                let second = second.clone();
                tokio::spawn(async move {
                    let mut head = vec![0u8; 8 * 1024];
                    let Ok(read) = client.read(&mut head).await else {
                        return;
                    };
                    let head = &head[..read];
                    // The switch applies to *later* connections, so the POST
                    // itself still reaches the incarnation the client observed.
                    let target = if switched.load(Ordering::SeqCst) {
                        second
                    } else {
                        first
                    };
                    if head.starts_with(b"POST ") {
                        switched.store(true, Ordering::SeqCst);
                    }
                    let Ok(mut upstream) = tokio::net::UnixStream::connect(&target).await else {
                        return;
                    };
                    if upstream.write_all(head).await.is_err() {
                        return;
                    }
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                });
            }
        });
        cancel
    }

    /// Run `cflx client enqueue` against an owner, off the async runtime thread.
    async fn enqueue(owner: &Owner, change_id: &str) -> Output {
        let socket = owner.socket();
        let change_id = change_id.to_string();
        let cwd = neutral_cwd();
        let path = cwd.path().to_path_buf();
        let output = tokio::task::spawn_blocking(move || {
            run_cli(
                &path,
                &[
                    "client",
                    "--unix-socket",
                    &socket,
                    "enqueue",
                    &change_id,
                    "--json",
                ],
                &[],
            )
        })
        .await
        .unwrap();
        drop(cwd);
        output
    }

    #[tokio::test]
    async fn enqueue_admits_an_idle_owner_with_an_isolated_mark_and_start() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "admitted");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(parsed["detail"]["route"], "mark_and_start");

        // Exactly the two commands the idle route needs, in order, and nothing
        // else: no bulk mark, no queue intent, no retry.
        let calls = spy.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            calls[0],
            CommandSpec::SetExecutionMark {
                change_id: "alpha".to_string(),
                marked: true
            }
        );
        assert_eq!(calls[1], CommandSpec::Start);
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_adds_live_owner_work_through_queue_intent() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![change("alpha", "running", "not queued")],
        ));
        owner.boundary.set_running(true);

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "admitted");
        assert_eq!(parsed["detail"]["route"], "set_queue_intent");
        assert_eq!(
            spy.calls(),
            vec![CommandSpec::SetQueueIntent {
                change_id: "alpha".to_string(),
                queued: true
            }],
            "a live owner must not be started a second time"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_routes_retryable_evidence_through_retry() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![change("alpha", "running", "error")],
        ));
        owner.boundary.set_running(true);

        let output = enqueue(&owner, "alpha").await;
        assert_eq!(envelope(&output)["outcome"], "admitted");
        assert_eq!(
            spy.calls(),
            vec![CommandSpec::RetryChange {
                change_id: "alpha".to_string()
            }]
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_is_an_idempotent_no_op_for_already_admitted_work() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut queued = change("alpha", "running", "not queued");
        queued.queue_intent = QueueIntent::Queued;
        owner.publish(snapshot("running", vec![queued]));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "already_admitted");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(parsed["detail"]["commands_submitted"], 0);
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_refuses_rather_than_consuming_an_unrelated_mark() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut beta = change("beta", "select", "not queued");
        beta.execution_marked = true;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued"), beta],
        ));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "operator_intent_conflict");
        assert_eq!(output.status.code(), Some(11));
        assert_eq!(parsed["detail"]["unrelated_marks"][0], "beta");
        // The whole point: `beta`'s mark is another operator's intent, and it is
        // neither consumed by a Start nor cleared to manufacture isolation.
        assert_eq!(spy.call_count(), 0, "no command may touch beta's mark");
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_refuses_unsafe_targets_without_any_hidden_mutation() {
        for (app_mode, status, expected) in [
            ("select", "archived", "target_ineligible"),
            ("select", "merged", "target_ineligible"),
            ("select", "rejected", "target_ineligible"),
            ("select", "blocked", "target_ineligible"),
        ] {
            let spy = SpyExecutor::new();
            let owner = Owner::start(Some(spy.clone()), None).await;
            owner.publish(snapshot(app_mode, vec![change("alpha", app_mode, status)]));

            let output = enqueue(&owner, "alpha").await;
            assert_eq!(envelope(&output)["outcome"], expected, "status={status}");
            assert_eq!(output.status.code(), Some(10));
            assert_eq!(spy.call_count(), 0, "status={status} must submit nothing");
            owner.stop().await;
        }
    }

    #[tokio::test]
    async fn enqueue_refuses_a_worktree_ineligible_target() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut alpha = change("alpha", "select", "not queued");
        alpha.parallel = ParallelEligibility {
            eligible: false,
            blocked_reason: Some(
                conflux::web::remote_control_api::dto::ParallelBlockedReason::NotCommitted,
            ),
        };
        owner.publish(snapshot("select", vec![alpha]));

        let output = enqueue(&owner, "alpha").await;
        assert_eq!(envelope(&output)["outcome"], "target_ineligible");
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_refuses_a_target_held_by_an_active_apply_iteration_limit() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut limited = change("alpha", "running", "error");
        // The active run owns this target's exhausted ceiling. It is retryable
        // in principle, which is exactly why a router reading only
        // `retry_change.allowed` would wrongly submit a retry here.
        limited.actions =
            conflux::web::remote_control_api::projection::limited_change_actions_for_test(
                "running", "error", true,
            );
        assert!(!limited.actions.retry_change.allowed);
        owner.publish(snapshot("running", vec![limited]));
        owner.boundary.set_running(true);

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "target_ineligible");
        assert_eq!(output.status.code(), Some(10));
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("Apply-dispatch ceiling"));
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_reports_partial_intent_when_a_mark_races_in_before_start() {
        // The mark settles, and an unrelated mark appears in the window before
        // Start. Starting would consume it; clearing either mark to restore
        // isolation would be mutating an operator's intent to tidy up our own.
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        struct RacingMark {
            inner: Arc<SpyExecutor>,
            projection: Arc<Projection>,
        }
        #[async_trait]
        impl RemoteControlExecutor for RacingMark {
            async fn execute(
                &self,
                command: &CommandSpec,
            ) -> Result<ExecutionSummary, CommandFailure> {
                let result = self.inner.execute(command).await;
                if matches!(command, CommandSpec::SetExecutionMark { .. }) {
                    let mut alpha = change("alpha", "select", "not queued");
                    alpha.execution_marked = true;
                    let mut beta = change("beta", "select", "not queued");
                    beta.execution_marked = true;
                    self.projection.apply_state(
                        "test_snapshot",
                        None,
                        serde_json::json!({}),
                        snapshot("select", vec![alpha, beta]),
                    );
                }
                result
            }
        }
        owner
            .runtime
            .bind(Arc::new(RacingMark {
                inner: spy.clone(),
                projection: owner.projection.clone(),
            }))
            .await;

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(parsed["detail"]["remaining_mark"], "alpha");
        assert_eq!(parsed["detail"]["rolled_back"], false);
        assert!(parsed["message"].as_str().unwrap().contains("beta"));
        // The mark, and only the mark: no Start, and no attempt to clear either
        // mark afterwards.
        assert_eq!(spy.call_count(), 1, "{:?}", spy.calls());
        assert!(matches!(
            spy.calls()[0],
            CommandSpec::SetExecutionMark { .. }
        ));
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_aborts_when_the_socket_starts_serving_a_different_incarnation() {
        // Two real owners behind one endpoint. The client marks against the
        // first, and the reread before Start reaches the second — which cannot
        // prove whether the first one's command settled.
        let first_spy = SpyExecutor::new();
        let second_spy = SpyExecutor::new();
        let first = Owner::start(Some(first_spy.clone()), None).await;
        let second = Owner::start(Some(second_spy.clone()), None).await;
        first.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        second.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let front_dir = tempfile::tempdir().expect("temp dir");
        let front = front_dir.path().join("cflx-api.sock");
        let switch =
            switch_after_first_post(front.clone(), first.socket.clone(), second.socket.clone())
                .await;

        let workspace = neutral_cwd();
        let socket = front.display().to_string();
        let output = tokio::task::spawn_blocking({
            let cwd = workspace.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "enqueue",
                        "alpha",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();

        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "owner_restarted");
        assert_eq!(output.status.code(), Some(8));
        assert_eq!(
            first_spy.call_count(),
            1,
            "only the mark reached the first incarnation: {:?}",
            first_spy.calls()
        );
        assert_eq!(
            second_spy.call_count(),
            0,
            "nothing may be resubmitted to a process that never saw the first command"
        );

        switch.cancel();
        first.stop().await;
        second.stop().await;
    }

    #[tokio::test]
    async fn enqueue_reports_partial_intent_when_start_is_refused_after_the_mark_settles() {
        let spy = SpyExecutor::new();
        // The mark succeeds; Start is refused. The mark is real and cannot be
        // honestly rolled back, so the client must say so rather than claim
        // either success or a clean refusal.
        spy.script(vec![
            Ok(ExecutionSummary::changed("marked")),
            Err(CommandFailure::new(
                ErrorCode::LifecycleConflict,
                "the run cannot start right now",
            )),
        ]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(parsed["detail"]["remaining_mark"], "alpha");
        assert_eq!(parsed["detail"]["rolled_back"], false);
        let message = parsed["message"].as_str().unwrap();
        assert!(message.contains("later operator Start"), "{message}");
        // Two commands, and crucially no third: the client must not try to undo
        // the mark it already settled.
        assert_eq!(spy.call_count(), 2, "{:?}", spy.calls());
        owner.stop().await;
    }

    // ------------------------------------------------------------------------
    // partial_intent_command_audit
    // ------------------------------------------------------------------------
    //
    // `commands_submitted` is an audit list, not a description of the situation.
    // A mark the client *found* and a mark the client *set* leave the repository
    // in the same shape but say different things about who to ask next, and only
    // one of them is this invocation's doing. The mirror error is just as bad: a
    // Start that was submitted and then failed is still an external effect the
    // owner recorded, so the list must carry it. In every case the list has to
    // equal what the executor behind the real endpoint actually saw.

    /// The audit spelling of each command the spy observed, in order.
    fn submitted_names(spy: &SpyExecutor) -> Vec<&'static str> {
        spy.calls()
            .iter()
            .map(|command| match command {
                CommandSpec::SetExecutionMark { .. } => "set_execution_mark",
                CommandSpec::Start => "start",
                CommandSpec::SetQueueIntent { .. } => "set_queue_intent",
                CommandSpec::RetryChange { .. } => "retry_change",
                _ => "other",
            })
            .collect()
    }

    #[tokio::test]
    async fn partial_intent_command_audit_lists_the_mark_this_invocation_submitted() {
        let spy = SpyExecutor::new();
        spy.script(vec![
            Ok(ExecutionSummary::changed("marked")),
            Err(CommandFailure::new(
                ErrorCode::LifecycleConflict,
                "the run cannot start right now",
            )),
        ]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(["set_execution_mark", "start"]),
            "a Start that was submitted and then failed is still an effect the \
             owner recorded"
        );
        assert_eq!(parsed["detail"]["remaining_mark"], "alpha");
        assert_eq!(parsed["detail"]["rolled_back"], false);

        // The audit list and the executor agree on exactly which commands ran.
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(submitted_names(&spy)),
            "the envelope audit must equal the executor's submission sequence"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn partial_intent_command_audit_omits_a_mark_it_only_found() {
        let spy = SpyExecutor::new();
        // Only Start is ever submitted: the target carries an operator's mark
        // already, so the client has no mark command to send.
        spy.script(vec![Err(CommandFailure::new(
            ErrorCode::LifecycleConflict,
            "the run cannot start right now",
        ))]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut premarked = change("alpha", "select", "not queued");
        premarked.execution_marked = true;
        owner.publish(snapshot("select", vec![premarked]));

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(["start"]),
            "a pre-existing mark is state, not a command this invocation sent, \
             but the Start that failed was still submitted"
        );

        // The truthful parts of the report survive: the mark is still there and
        // a later operator Start can still consume it.
        assert_eq!(parsed["detail"]["remaining_mark"], "alpha");
        assert_eq!(parsed["detail"]["rolled_back"], false);
        let message = parsed["message"].as_str().unwrap();
        assert!(message.contains("later operator Start"), "{message}");
        assert!(
            message.contains("already execution-marked"),
            "the message must not claim a mark settled: {message}"
        );

        // Exactly one command reached the executor, and it was not a mark.
        assert_eq!(spy.call_count(), 1, "{:?}", spy.calls());
        assert!(
            matches!(spy.calls().as_slice(), [CommandSpec::Start]),
            "{:?}",
            spy.calls()
        );
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(submitted_names(&spy)),
            "the envelope audit must equal the executor's submission sequence"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn partial_intent_command_audit_omits_a_start_it_never_submitted() {
        // An unrelated mark appearing between our settled mark and Start makes
        // Start unsafe, so it is never POSTed. The audit must stop at the mark:
        // "we intended two commands" is not the same claim as "we sent two".
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![
                change("alpha", "select", "not queued"),
                change("beta", "select", "not queued"),
            ],
        ));

        // The racing mark lands from inside the mark command's own execution, so
        // the client's pre-Start reread is guaranteed to observe it.
        struct Racing {
            inner: Arc<SpyExecutor>,
            projection: Arc<Projection>,
        }
        #[async_trait]
        impl RemoteControlExecutor for Racing {
            async fn execute(
                &self,
                command: &CommandSpec,
            ) -> Result<ExecutionSummary, CommandFailure> {
                let result = self.inner.execute(command).await;
                let mut alpha = change("alpha", "select", "not queued");
                alpha.execution_marked = true;
                let mut beta = change("beta", "select", "not queued");
                beta.execution_marked = true;
                self.projection.apply_state(
                    "test_snapshot",
                    None,
                    serde_json::json!({}),
                    snapshot("select", vec![alpha, beta]),
                );
                result
            }
        }
        owner
            .runtime
            .bind(Arc::new(Racing {
                inner: spy.clone(),
                projection: owner.projection.clone(),
            }))
            .await;

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(["set_execution_mark"]),
            "a Start that was never POSTed must not appear in the audit"
        );
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(submitted_names(&spy)),
            "the envelope audit must equal the executor's submission sequence"
        );
        assert!(
            matches!(
                spy.calls().as_slice(),
                [CommandSpec::SetExecutionMark { .. }]
            ),
            "{:?}",
            spy.calls()
        );
        assert_eq!(parsed["detail"]["remaining_mark"], "alpha");
        assert_eq!(parsed["detail"]["rolled_back"], false);
        owner.stop().await;
    }

    // ------------------------------------------------------------------------
    // stale_revision_command_audit
    // ------------------------------------------------------------------------
    //
    // The audit spans a whole invocation, and a recomputation is the only place
    // where that span covers more than one attempt. Two opposite lies are
    // available there: a submission the owner refused as stale created no
    // command record and must not be counted, while the recomputed submission
    // that *did* produce one must be counted exactly once. Both are invisible to
    // every other test in this file, because no other test makes the endpoint
    // reject a submission and then watches what the next one does to the list.

    #[tokio::test]
    async fn stale_revision_command_audit_matches_the_records_across_a_recomputation() {
        let spy = SpyExecutor::new();
        let api = ApiSpy::new();
        let owner = Owner::start_intercepted(Some(spy.clone()), None, api.clone()).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        // One advance, injected in the only window that can produce a stale
        // submission: after the client took its observation, before the real
        // endpoint compares `expected_revision`. It is deliberately routing-
        // irrelevant progress — the owner moved on with its own work, so the
        // recomputed attempt must reach the same conclusion at a newer revision.
        let projection = owner.projection.clone();
        api.inject_before_commands(vec![Box::new(move || {
            let mut advanced = change("alpha", "select", "not queued");
            advanced.completed_tasks = 1;
            advanced.progress_percent = 50.0;
            projection.apply_state(
                "test_snapshot",
                None,
                serde_json::json!({}),
                snapshot("select", vec![advanced]),
            );
        })]);

        let output = enqueue(&owner, "alpha").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "admitted", "{}", stderr_of(&output));
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(parsed["detail"]["route"], "mark_and_start");

        // The injection fired and the *real* endpoint refused the first mutation
        // this invocation attempted, before any command record existed.
        let exchanges = api.exchanges();
        assert_eq!(
            exchanges.len(),
            3,
            "one refused mark, one recomputed mark, one start: {exchanges:#?}"
        );
        assert_eq!(exchanges[0].command_type, "set_execution_mark");
        assert_eq!(exchanges[0].status, 409);
        assert_eq!(exchanges[0].error_code.as_deref(), Some("stale_revision"));
        assert!(
            exchanges[0].record_id.is_none(),
            "a stale submission is refused before admission, so no record can exist: {:#?}",
            exchanges[0]
        );

        // The reread and recomputation happened inside this one invocation: a
        // fresh authoritative read landed between the refusal and the next
        // submission, and that submission carried a revision the first attempt
        // never observed.
        let trace = api.requests();
        let first_post = trace
            .iter()
            .position(|request| request == "POST /api/v2/commands")
            .expect("the client must submit a command");
        let second_post = first_post
            + 1
            + trace[first_post + 1..]
                .iter()
                .position(|request| request == "POST /api/v2/commands")
                .expect("the client must recompute and submit again");
        assert_eq!(
            trace[first_post + 1..second_post]
                .iter()
                .filter(|request| *request == "GET /api/v2/state")
                .count(),
            1,
            "the client must reread authoritative state between the stale refusal and its \
             recomputed submission: {trace:#?}"
        );
        assert!(
            exchanges[1].expected_revision > exchanges[0].expected_revision,
            "the recomputed submission must carry the advanced revision: {exchanges:#?}"
        );
        assert_eq!(exchanges[1].command_type, "set_execution_mark");
        assert_eq!(exchanges[2].command_type, "start");
        let mut keys: Vec<&str> = exchanges
            .iter()
            .map(|exchange| exchange.idempotency_key.as_str())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(
            keys.len(),
            3,
            "a recomputed submission must mint its own key, or the owner would replay the \
             refused identity: {exchanges:#?}"
        );

        // The audit equals, in order, exactly the submissions the owner answered
        // with a command record. The refused attempt is absent because no record
        // was created for it, and the mark appears once rather than twice.
        let recorded: Vec<&str> = exchanges
            .iter()
            .filter(|exchange| exchange.record_id.is_some())
            .map(|exchange| exchange.command_type.as_str())
            .collect();
        assert_eq!(
            recorded,
            vec!["set_execution_mark", "start"],
            "{exchanges:#?}"
        );
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(recorded),
            "the envelope audit must equal the owner's own command records"
        );
        assert_eq!(
            parsed["detail"]["commands_submitted"],
            serde_json::json!(submitted_names(&spy)),
            "the envelope audit must equal the executor's submission sequence"
        );
        assert_eq!(
            spy.call_count(),
            2,
            "the refused attempt must not have reached the executor: {:?}",
            spy.calls()
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_recomputes_after_a_stale_revision_without_repeating_a_settled_effect() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![change("alpha", "running", "not queued")],
        ));
        owner.boundary.set_running(true);

        // Advance the revision exactly once, from inside the executor, so the
        // client's *next* submission is admitted against a revision it did not
        // observe. The endpoint rejects it as stale and the client recomputes.
        let projection = owner.projection.clone();
        let bumped = Arc::new(AtomicUsize::new(0));
        struct Bumping {
            inner: Arc<SpyExecutor>,
            projection: Arc<Projection>,
            bumped: Arc<AtomicUsize>,
        }
        #[async_trait]
        impl RemoteControlExecutor for Bumping {
            async fn execute(
                &self,
                command: &CommandSpec,
            ) -> Result<ExecutionSummary, CommandFailure> {
                let result = self.inner.execute(command).await;
                if self.bumped.fetch_add(1, Ordering::SeqCst) == 0 {
                    // A second, unrelated advance lands after this command, which
                    // is what makes a retry of the same revision stale.
                    self.projection.apply_state(
                        "test_snapshot",
                        None,
                        serde_json::json!({}),
                        snapshot("running", vec![change("alpha", "running", "not queued")]),
                    );
                }
                result
            }
        }
        owner
            .runtime
            .bind(Arc::new(Bumping {
                inner: spy.clone(),
                projection: projection.clone(),
                bumped: bumped.clone(),
            }))
            .await;

        let first = enqueue(&owner, "alpha").await;
        assert_eq!(envelope(&first)["outcome"], "admitted");
        let after_first = spy.call_count();

        // A second enqueue against the already-advanced owner still settles once.
        let second = enqueue(&owner, "alpha").await;
        assert_eq!(envelope(&second)["outcome"], "admitted");
        assert_eq!(
            spy.call_count(),
            after_first + 1,
            "each intent submits exactly one queue command: {:?}",
            spy.calls()
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_reports_a_command_failure_rather_than_claiming_admission() {
        let spy = SpyExecutor::new();
        spy.script(vec![Err(CommandFailure::new(
            ErrorCode::TargetIneligible,
            "the target is not admissible",
        ))]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![change("alpha", "running", "not queued")],
        ));
        owner.boundary.set_running(true);

        let output = enqueue(&owner, "alpha").await;
        assert_eq!(envelope(&output)["outcome"], "target_ineligible");
        assert_eq!(output.status.code(), Some(10));
        assert_eq!(spy.call_count(), 1);
        owner.stop().await;
    }

    #[tokio::test]
    async fn enqueue_starts_no_second_owner_and_writes_nothing_when_it_refuses() {
        let workspace = tempfile::tempdir().unwrap();
        let owner = Owner::start(None, None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let socket = owner.socket();

        let before: Vec<_> = std::fs::read_dir(workspace.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let output = tokio::task::spawn_blocking({
            let cwd = workspace.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "enqueue",
                        "alpha",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let after: Vec<_> = std::fs::read_dir(workspace.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert_eq!(envelope(&output)["outcome"], "owner_not_command_capable");
        assert_eq!(
            before, after,
            "a refused enqueue must not touch the workspace"
        );
        owner.stop().await;
    }

    // ========================================================================
    // wait
    // ========================================================================

    /// A deterministic temporary repository, used as terminal-completion proof.
    ///
    /// Real Git, because the completion oracle reads real trees — but tiny,
    /// offline, and with no orchestration or AI phase anywhere near it.
    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let fixture = Self { dir };
            fixture.git(&["init", "--initial-branch=main"]);
            fixture.git(&["config", "user.email", "test@example.com"]);
            fixture.git(&["config", "user.name", "Test"]);
            fixture.git(&["config", "commit.gpgsign", "false"]);
            fixture.write("README.md", "fixture\n");
            fixture.git(&["add", "-A"]);
            fixture.git(&["commit", "-m", "init"]);
            fixture
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
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

        /// Put the change in its active (not yet archived) shape.
        fn stage_active(&self, change_id: &str) {
            self.write(
                &format!("openspec/changes/{change_id}/proposal.md"),
                "# Proposal\n",
            );
            self.git(&["add", "-A"]);
            self.git(&["commit", "-m", "add change"]);
        }

        /// Archive the change: entry present, active directory gone.
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

        /// Leave both the archive entry and the active directory in place.
        fn contradict(&self, change_id: &str) {
            self.write(
                &format!("openspec/changes/{change_id}/proposal.md"),
                "# Proposal\n",
            );
            self.write(
                &format!("openspec/changes/archive/2026-01-01-{change_id}/proposal.md"),
                "# Proposal\n",
            );
            self.git(&["add", "-A"]);
            self.git(&["commit", "-m", "contradictory"]);
        }
    }

    async fn wait_for(owner: &Owner, repo: &Path, change_id: &str, timeout: &str) -> Output {
        let socket = owner.socket();
        let repo = repo.to_path_buf();
        let change_id = change_id.to_string();
        let timeout = timeout.to_string();
        tokio::task::spawn_blocking(move || {
            run_cli(
                &repo,
                &[
                    "client",
                    "--unix-socket",
                    &socket,
                    "wait",
                    &change_id,
                    "--timeout",
                    &timeout,
                    "--json",
                ],
                &[],
            )
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn wait_proves_local_integration_from_repository_evidence() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        repo.archive("alpha");

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "completed");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(parsed["detail"]["terminal_mode"], "merged");
        assert_eq!(parsed["detail"]["commands_submitted"], 0);
        assert_eq!(spy.call_count(), 0, "wait must submit no command");
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_proves_branch_publication_without_claiming_base_integration() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        // The change branch carries the archived proposal; base does not.
        repo.git(&["checkout", "-b", "alpha"]);
        repo.archive("alpha");
        repo.git(&["checkout", "main"]);

        // A second repository standing in for the remote, so `ls-remote` is real.
        let remote = tempfile::tempdir().unwrap();
        let remote_path = remote.path().join("origin.git");
        std::process::Command::new("git")
            .args(["init", "--bare", remote_path.to_str().unwrap()])
            .output()
            .unwrap();
        repo.git(&["remote", "add", "origin", remote_path.to_str().unwrap()]);
        repo.git(&["push", "origin", "alpha"]);

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "pushed")],
        ));
        owner.contract(OwnerExecutionContract::resolve(
            "main",
            Some("origin"),
            None,
        ));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "completed");
        assert_eq!(parsed["detail"]["terminal_mode"], "branch_pushed");
        assert_eq!(parsed["detail"]["pushed_branch"], "alpha");
        let evidence = parsed["detail"]["evidence"].as_str().unwrap();
        assert!(
            evidence.contains("not base integration"),
            "publication must not be reported as base integration: {evidence}"
        );
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_proves_base_publication_only_when_the_remote_matches() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        repo.archive("alpha");

        let remote = tempfile::tempdir().unwrap();
        let remote_path = remote.path().join("upstream.git");
        std::process::Command::new("git")
            .args(["init", "--bare", remote_path.to_str().unwrap()])
            .output()
            .unwrap();
        repo.git(&["remote", "add", "upstream", remote_path.to_str().unwrap()]);

        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));
        owner.contract(OwnerExecutionContract::resolve(
            "main",
            None,
            Some("upstream"),
        ));

        // Locally integrated but not published: that is not the owner's terminal
        // success, so the wait must expire instead of succeeding.
        let unpublished = wait_for(&owner, repo.path(), "alpha", "300ms").await;
        let parsed = envelope(&unpublished);
        assert_eq!(parsed["outcome"], "timeout");
        assert_eq!(unpublished.status.code(), Some(19));

        repo.git(&["push", "upstream", "main"]);
        let published = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&published);
        assert_eq!(parsed["outcome"], "completed");
        assert_eq!(parsed["detail"]["terminal_mode"], "base_published");
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_does_not_treat_disappearance_as_completion() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        // The change is simply not in the snapshot, and base holds no archive
        // entry. Nothing here proves anything finished.
        owner.publish(snapshot(
            "select",
            vec![change("beta", "select", "not queued")],
        ));
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "300ms").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "timeout");
        assert!(!parsed["ok"].as_bool().unwrap());
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_reports_contradictory_repository_evidence_as_its_own_outcome() {
        let repo = Fixture::new();
        repo.contradict("alpha");

        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "evidence_error");
        assert_eq!(output.status.code(), Some(22));
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_reports_a_missing_base_branch_as_an_evidence_error() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        repo.archive("alpha");

        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));
        owner.contract(merged_contract("a-branch-that-does-not-exist"));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        assert_eq!(envelope(&output)["outcome"], "evidence_error");
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_reports_rejection_without_repairing_it() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut rejected = change("alpha", "select", "rejected");
        rejected.error_detail = Some("the proposal was rejected in review".to_string());
        owner.publish(snapshot("select", vec![rejected]));
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "change_rejected");
        assert_eq!(output.status.code(), Some(17));
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("rejected in review"));
        assert_eq!(spy.call_count(), 0, "wait must never retry a rejection");
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_reports_a_fatal_process_error() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        let mut failed = snapshot("select", vec![change("alpha", "select", "applying")]);
        failed.process_error = Some("the orchestration run died".to_string());
        owner.publish(failed);
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "process_failed");
        assert_eq!(output.status.code(), Some(18));
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_refuses_to_wait_on_an_owner_that_published_no_terminal_mode() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let owner = Owner::start(Some(SpyExecutor::new()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));

        let output = wait_for(&owner, repo.path(), "alpha", "30s").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "unsupported_terminal_mode");
        assert_eq!(output.status.code(), Some(16));
        let message = parsed["message"].as_str().unwrap();
        assert!(message.contains("could only end in a timeout"), "{message}");
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_times_out_without_mutating_anything() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        let head_before = repo.git(&["rev-parse", "HEAD"]);

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));
        owner.contract(merged_contract("main"));

        let output = wait_for(&owner, repo.path(), "alpha", "300ms").await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "timeout");
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(parsed["detail"]["commands_submitted"], 0);
        assert_eq!(spy.call_count(), 0);
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            repo.git(&["status", "--porcelain"]),
            "",
            "wait must leave the worktree clean"
        );
        owner.stop().await;
    }

    // ------------------------------------------------------------------------
    // wait_deadline
    // ------------------------------------------------------------------------
    //
    // `--timeout D` is a promise about the operation, so the oracle here is the
    // *outcome*, never elapsed time: an unbounded read reports `transport_error`
    // after the transport's own 30s valve, and an unbounded `git ls-remote`
    // never reports anything at all. Both fixtures synchronize on the client
    // actually being inside the blocking step — a connection accepted — so a
    // client that failed to connect could not pass by accident. The wall-clock
    // guards below exist only so a regression fails instead of hanging.

    /// Longest a bounded `wait` invocation may take before the test gives up.
    ///
    /// Far above the sub-second deadlines under test and above the transport's
    /// own 30s valve, so it can only trip on a genuine loss of bounding.
    const DEADLINE_TEST_GUARD: Duration = Duration::from_secs(60);

    #[tokio::test]
    async fn wait_deadline_bounds_a_stalled_owner_read_rather_than_the_transport_valve() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        let head_before = repo.git(&["rev-parse", "HEAD"]);

        // Accepts, then never answers: the client gets past `connect` and blocks
        // inside the exchange, which is the only place the deadline can be
        // observed doing its job.
        let dir = tempfile::tempdir().expect("temp dir");
        let socket = dir.path().join("stalled.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("binds the stalled socket");
        let accepted = Arc::new(AtomicUsize::new(0));
        let shutdown = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn({
            let accepted = accepted.clone();
            let stop = shutdown.clone();
            async move {
                // The accepted streams are held, not dropped: closing them would
                // hand the client an EOF and let it fail for the wrong reason.
                let mut held = Vec::new();
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        incoming = listener.accept() => {
                            let Ok((stream, _)) = incoming else { break };
                            accepted.fetch_add(1, Ordering::SeqCst);
                            held.push(stream);
                        }
                    }
                }
            }
        });

        let path = socket.display().to_string();
        let output = tokio::time::timeout(
            DEADLINE_TEST_GUARD,
            tokio::task::spawn_blocking({
                let cwd = repo.path().to_path_buf();
                move || {
                    run_cli(
                        &cwd,
                        &[
                            "client",
                            "--unix-socket",
                            &path,
                            "wait",
                            "alpha",
                            "--timeout",
                            "500ms",
                            "--json",
                        ],
                        &[],
                    )
                }
            }),
        )
        .await
        .expect("a bounded wait must not hang")
        .unwrap();

        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"], "timeout",
            "a stalled owner must expire the operation, not the transport"
        );
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(parsed["detail"]["commands_submitted"], 0);
        assert!(
            accepted.load(Ordering::SeqCst) >= 1,
            "the client must have been inside a request, not failing to connect"
        );
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            repo.git(&["status", "--porcelain"]),
            "",
            "a timed-out wait must leave the worktree clean"
        );
        shutdown.cancel();
        let _ = task.await;
    }

    #[tokio::test]
    async fn wait_deadline_terminates_a_stalled_remote_lookup_and_reaps_it() {
        let repo = Fixture::new();
        repo.stage_active("alpha");
        repo.archive("alpha");
        let head_before = repo.git(&["rev-parse", "HEAD"]);

        // A `git://` endpoint that accepts and then says nothing. Real Git, real
        // TCP, and no network beyond loopback: `git ls-remote` connects, sends
        // its request, and waits forever for a ref advertisement.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("binds a loopback port");
        let port = listener.local_addr().unwrap().port();
        let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();
        let shutdown = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn({
            let stop = shutdown.clone();
            async move {
                let mut connected_tx = Some(connected_tx);
                let mut held = Vec::new();
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        incoming = listener.accept() => {
                            let Ok((stream, _)) = incoming else { break };
                            if let Some(tx) = connected_tx.take() {
                                let _ = tx.send(());
                            }
                            held.push(stream);
                        }
                    }
                }
                held
            }
        });

        repo.git(&[
            "remote",
            "add",
            "upstream",
            &format!("git://127.0.0.1:{port}/stalled.git"),
        ]);

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));
        // Base publication, so local integration alone is not enough and the
        // remote lookup is genuinely required to decide the outcome.
        owner.contract(OwnerExecutionContract::resolve(
            "main",
            None,
            Some("upstream"),
        ));

        let output = tokio::time::timeout(
            DEADLINE_TEST_GUARD,
            wait_for(&owner, repo.path(), "alpha", "2s"),
        )
        .await
        .expect("an unbounded git ls-remote would hang here");

        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"], "timeout",
            "the deadline owns the outcome; no later evidence error may replace it"
        );
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(parsed["detail"]["commands_submitted"], 0);
        assert_eq!(spy.call_count(), 0, "wait must submit no command");

        // The lookup really happened, so the deadline cancelled work in flight
        // rather than skipping it.
        tokio::time::timeout(DEADLINE_TEST_GUARD, connected_rx)
            .await
            .expect("git must have reached the stalled remote")
            .expect("the fixture must report the connection");

        // The child is gone: its socket reached EOF. A `git` left running would
        // hold the connection open and this read would never return.
        let held = tokio::time::timeout(DEADLINE_TEST_GUARD, async {
            shutdown.cancel();
            task.await.unwrap()
        })
        .await
        .expect("the fixture must stop");
        let mut stream = held.into_iter().next().expect("one accepted connection");
        let mut sink = [0u8; 64];
        loop {
            let read = tokio::time::timeout(
                DEADLINE_TEST_GUARD,
                tokio::io::AsyncReadExt::read(&mut stream, &mut sink),
            )
            .await
            .expect("the git child must have been terminated and reaped")
            .expect("reading the fixture connection");
            if read == 0 {
                break;
            }
        }

        assert_eq!(repo.git(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            repo.git(&["status", "--porcelain"]),
            "",
            "a timed-out wait must leave the worktree clean"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn wait_reports_owner_replacement_when_repository_evidence_is_absent() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        // The socket path outlives both incarnations, because "the same endpoint
        // now serves a different process" is the situation under test.
        let endpoint = tempfile::tempdir().expect("temp dir");
        let socket_path = endpoint.path().join("cflx-api.sock");

        let spy = SpyExecutor::new();
        let first = Owner::start_on(socket_path.clone(), Some(spy.clone()), None).await;
        first.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));
        first.contract(merged_contract("main"));

        let repo_path = repo.path().to_path_buf();
        let socket = socket_path.display().to_string();
        let waiting = tokio::task::spawn_blocking(move || {
            run_cli(
                &repo_path,
                &[
                    "client",
                    "--unix-socket",
                    &socket,
                    "wait",
                    "alpha",
                    "--timeout",
                    "20s",
                    "--json",
                ],
                &[],
            )
        });

        // Let the wait capture the first incarnation, then replace it.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let first_instance = first.projection.instance_id().to_string();
        first.stop().await;
        std::fs::remove_file(&socket_path).ok();

        let second = Owner::start_on(socket_path.clone(), Some(SpyExecutor::new()), None).await;
        assert_ne!(
            second.projection.instance_id(),
            first_instance,
            "the replacement must be a different incarnation"
        );
        second.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));
        second.contract(merged_contract("main"));

        let output = waiting.await.unwrap();
        let parsed = envelope(&output);
        let outcome = parsed["outcome"].as_str().unwrap();
        // Either the client noticed the replacement, or it noticed the gap while
        // the socket was down. Neither may be reported as completion: base holds
        // no archive entry, so repository evidence alone proves nothing.
        assert!(
            outcome == "owner_restarted" || outcome == "owner_not_running",
            "a replaced owner must never read as completion, got {outcome}"
        );
        assert_eq!(spy.call_count(), 0);
        second.stop().await;
    }

    #[tokio::test]
    async fn wait_succeeds_across_owner_replacement_on_repository_evidence_alone() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let endpoint = tempfile::tempdir().expect("temp dir");
        let socket_path = endpoint.path().join("cflx-api.sock");

        let spy = SpyExecutor::new();
        let first = Owner::start_on(socket_path.clone(), Some(spy.clone()), None).await;
        first.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));
        first.contract(merged_contract("main"));

        let repo_path = repo.path().to_path_buf();
        let socket = socket_path.display().to_string();
        let waiting = tokio::task::spawn_blocking(move || {
            run_cli(
                &repo_path,
                &[
                    "client",
                    "--unix-socket",
                    &socket,
                    "wait",
                    "alpha",
                    "--timeout",
                    "20s",
                    "--json",
                ],
                &[],
            )
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        // The archive lands in the repository, and *then* the owner is replaced.
        // The new incarnation knows nothing about the work, so the only thing
        // that can carry the success is the repository itself.
        repo.archive("alpha");
        first.stop().await;
        std::fs::remove_file(&socket_path).ok();

        let second = Owner::start_on(socket_path.clone(), Some(SpyExecutor::new()), None).await;
        second.publish(snapshot("select", vec![]));
        second.contract(merged_contract("main"));

        let output = waiting.await.unwrap();
        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"],
            "completed",
            "repository evidence alone must certify the success, stderr={}",
            stderr_of(&output)
        );
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(spy.call_count(), 0);
        second.stop().await;
    }

    #[tokio::test]
    async fn wait_recovers_from_an_event_gap_by_rehydrating_every_resource() {
        let repo = Fixture::new();
        repo.stage_active("alpha");

        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "applying")],
        ));
        owner.contract(merged_contract("main"));

        let repo_path = repo.path().to_path_buf();
        let socket = owner.socket();
        let waiting = tokio::task::spawn_blocking(move || {
            run_cli(
                &repo_path,
                &[
                    "client",
                    "--unix-socket",
                    &socket,
                    "wait",
                    "alpha",
                    "--timeout",
                    "20s",
                    "--json",
                ],
                &[],
            )
        });

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        // Blow past the retained event ring, so any cursor the client is holding
        // is no longer replayable. Completion still has to be noticed, because
        // every wake rehydrates the authoritative resources rather than trusting
        // the stream.
        for iteration in 0..1200 {
            owner.publish(snapshot(
                "select",
                vec![change(
                    "alpha",
                    "select",
                    if iteration % 2 == 0 {
                        "applying"
                    } else {
                        "accepting"
                    },
                )],
            ));
        }
        repo.archive("alpha");
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));

        let output = waiting.await.unwrap();
        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"],
            "completed",
            "a replay gap must not cost the wait its answer, stderr={}",
            stderr_of(&output)
        );
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    // ========================================================================
    // production_owner_smoke
    // ========================================================================

    /// A scheduler port that records launches instead of spawning them.
    ///
    /// The one boundary this smoke test cannot cross: activating a real launch
    /// would start AI phases. Everything below it — the operator command
    /// service, the run-control service, the coordinator, the shared
    /// application transaction, and the v2 executor — is the production
    /// assembly, so a recorded launch is proof that admission really happened.
    #[derive(Default)]
    struct SmokeScheduler {
        launches: Arc<Mutex<Vec<Vec<String>>>>,
        running: AtomicBool,
    }

    impl SmokeScheduler {
        fn launches(&self) -> Vec<Vec<String>> {
            self.launches.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl conflux::orchestration::run_control::RunSchedulerPort for SmokeScheduler {
        fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }

        async fn prepare_run(
            &self,
            targets: Vec<String>,
            _explicit_retry: bool,
        ) -> Result<conflux::orchestration::run_control::RunPermit, String> {
            // Recorded only on activation, which is the production ordering: a
            // prepared-but-unactivated launch is a rollback, not a run.
            let recorded = self.launches.clone();
            Ok(conflux::orchestration::run_control::RunPermit::new(
                move || recorded.lock().unwrap().push(targets),
            ))
        }

        async fn notify_scheduler(&self) {}

        async fn cancel_run(&self) {}

        fn set_graceful_stop(&self, _requested: bool) {}

        async fn stop_activity(&self) -> conflux::tui::stop_classification::StopActivitySnapshot {
            use conflux::tui::stop_classification::{
                ExecutionEvidence, ShutdownWorkEvidence, StopActivitySnapshot,
            };
            // Nothing is executing and nothing is draining: this owner admits
            // work and never stops any.
            StopActivitySnapshot {
                execution_handles: ExecutionEvidence::Known { registered: 0 },
                reducer_agent_execution_active: false,
                shutdown_work: ShutdownWorkEvidence::Known { pending: false },
            }
        }
    }

    /// A queue port with nothing to cancel: this smoke test admits work, and
    /// never stops any.
    #[derive(Default)]
    struct SmokeQueue {
        added: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl conflux::orchestration::operator_command::QueuePort for SmokeQueue {
        async fn add(&self, change_id: &str) -> bool {
            self.added.lock().unwrap().push(change_id.to_string());
            true
        }

        async fn remove(&self, _change_id: &str) -> bool {
            true
        }

        async fn request_cancellation(
            &self,
            _change_id: &str,
        ) -> Result<Option<conflux::orchestration::operator_command::TerminationWaiter>, String>
        {
            Ok(None)
        }

        async fn notify_scheduler(&self) {}
    }

    #[tokio::test]
    async fn production_owner_smoke_admits_one_change_through_the_shared_coordinator() {
        use conflux::orchestration::operator_command::{
            ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, ParallelRuntime,
        };
        use conflux::orchestration::operator_coordinator::CoreMode;
        use conflux::orchestration::run_control::{ResolveReservations, RunControlService};
        use conflux::orchestration::state::OrchestratorState;
        use conflux::web::state::WebState;

        // ── The production assembly, minus the launch itself ────────────────
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["alpha".to_string()],
            4,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let queue = Arc::new(SmokeQueue::default());
        let scheduler = Arc::new(SmokeScheduler::default());
        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                queue.clone(),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service,
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel,
        ));

        let listing = conflux::openspec::Change {
            id: "alpha".to_string(),
            completed_tasks: 0,
            total_tasks: 2,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: Default::default(),
        };
        let web_state = Arc::new(WebState::new(std::slice::from_ref(&listing)));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        web_state.update_with_mode(&[listing], "select").await;
        web_state.sync_remote_control_projection().await;

        let core_mode = Arc::new(CoreMode::new());
        let (executor, application) = conflux::web::remote_control_api::executor::wired_for_test(
            reducer.clone(),
            run_control,
            web_state.clone(),
            core_mode,
        );

        let runtime = web_state.remote_control();
        runtime.bind(Arc::new(executor)).await;
        runtime.bind_gate(application.gate()).await;
        runtime.bind_run_boundary(scheduler.clone());

        let dir = tempfile::tempdir().expect("temp dir");
        let socket_path = dir.path().join("cflx-api.sock");
        let auth = RemoteControlAuth::new(None, &[]).expect("auth policy is valid");
        let app = router(
            RemoteControlState::new(runtime.projection(), Arc::new(auth), runtime.clone())
                .with_gate(runtime.gate())
                .with_execution_facts(runtime.execution_facts())
                .with_execution_contract(runtime.execution_contract()),
        );
        let listener = tokio::net::UnixListener::bind(&socket_path).expect("binds");
        let shutdown = tokio_util::sync::CancellationToken::new();
        let serve = tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async move { shutdown.cancelled().await })
                    .await;
            }
        });

        // ── One real admission round trip through the compiled CLI ──────────
        let workspace = tempfile::tempdir().expect("temp dir");
        let socket = socket_path.display().to_string();
        let output = tokio::task::spawn_blocking({
            let cwd = workspace.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "enqueue",
                        "alpha",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();

        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"],
            "admitted",
            "stderr={}",
            stderr_of(&output)
        );
        assert_eq!(output.status.code(), Some(0));

        // The proof this test exists for: the shared coordinator really admitted
        // the change. The mark is in the process-local store a keypress writes,
        // and the scheduler saw a launch naming exactly this target.
        assert!(
            marks.is_marked("alpha"),
            "the shared execution-mark store must hold the admitted change"
        );
        assert_eq!(
            scheduler.launches(),
            vec![vec!["alpha".to_string()]],
            "exactly one launch, for exactly the requested change"
        );

        shutdown.cancel();
        let _ = serve.await;
    }

    // ========================================================================
    // documentation
    // ========================================================================

    #[test]
    fn documentation_recommends_the_client_for_existing_owner_delegation() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let agents = std::fs::read_to_string(repo_root.join("AGENTS.md")).expect("AGENTS.md");

        assert!(
            agents.contains("cflx client"),
            "AGENTS.md must document the client namespace"
        );
        for example in [
            "cflx client status",
            "cflx client enqueue",
            "cflx client wait",
        ] {
            assert!(agents.contains(example), "AGENTS.md must show `{example}`");
        }
        assert!(
            agents.contains("/api/v2"),
            "AGENTS.md must retain the API as the low-level contract"
        );
        // The distinction that actually prevents misuse.
        assert!(
            agents.contains("cflx run") && agents.contains("owner"),
            "AGENTS.md must keep the run/client ownership distinction"
        );
        // And it must not teach anyone to build the envelopes the client hides.
        for forbidden in ["expected_revision", "idempotency_key"] {
            let recommended = agents
                .split("## Delegating to an existing owner")
                .nth(1)
                .unwrap_or("");
            assert!(
                !recommended.contains(forbidden),
                "the delegation guidance must not tell a caller to construct {forbidden}"
            );
        }
    }
}
