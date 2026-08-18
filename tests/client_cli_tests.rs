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

// ============================================================================
// client_help_and_usage
// ============================================================================
//
// Help text and argv parsing are the same in both builds — neither needs a
// local API — so these run wherever the binary does. A caller reaches this
// surface before it ever reaches an owner, and everything asserted here is
// refused *before* any request is sent.

/// The `subscribe` group is documented as an explicit, proposal-scoped callback
/// registration whose argv is data. The two things a reader must not come away
/// believing are that the callback is shell source, and that "the TUI is still
/// running" was ever a statement about a proposal.
#[test]
fn client_subscribe_help_and_usage_documents_an_argv_callback_and_proposal_scope() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let namespace = run_cli(tmp.path(), &["client", "--help"], &[]);
    assert!(namespace.status.success(), "{}", stderr_of(&namespace));
    let namespace_help = stdout_of(&namespace);
    assert!(
        namespace_help.contains("subscribe"),
        "the namespace must offer the group:\n{namespace_help}"
    );

    let output = run_cli(tmp.path(), &["client", "subscribe", "--help"], &[]);
    assert!(output.status.success(), "{}", stderr_of(&output));
    let help = stdout_of(&output);
    for expected in [
        "set",
        "get",
        "clear",
        "never shell source",
        "execution completion, not process completion",
        "transport_not_permitted",
        "CFLX_EVENT_PATH",
        "--unix-socket",
        "--auth-token-env",
    ] {
        assert!(
            help.contains(expected),
            "subscribe help must mention {expected}:\n{help}"
        );
    }
    // Delivery is observability. A reader must not be told it resumes anything.
    assert!(
        help.contains("never resumes") || help.contains("resumes no agent"),
        "subscribe help must say delivery does not resume an agent:\n{help}"
    );
    // The protocol stays out of the public surface here too: a callback
    // registration is not a command, and nothing about it is constructible.
    for forbidden in [
        "--expected-revision",
        "--idempotency-key",
        "--command-type",
        "--execution-mark",
        "--auth-token ",
        "--shell",
    ] {
        assert!(
            !help.contains(forbidden),
            "subscribe help must not expose {forbidden}:\n{help}"
        );
    }

    let set = run_cli(tmp.path(), &["client", "subscribe", "set", "--help"], &[]);
    assert!(set.status.success(), "{}", stderr_of(&set));
    let set_help = stdout_of(&set);
    assert!(
        set_help.contains("-- <COMMAND>..."),
        "the separator must be part of the documented usage:\n{set_help}"
    );
    for expected in ["--blocked", "--instance-id", "--json", "<CHANGE_IDS>"] {
        assert!(
            set_help.contains(expected),
            "set help must mention {expected}:\n{set_help}"
        );
    }

    // A refused-or-not, help is an answer rather than a side effect.
    assert!(
        std::fs::read_dir(tmp.path())
            .expect("temp dir readable")
            .next()
            .is_none(),
        "printing help must write nothing"
    );
}

/// The first place an operator reads about the namespace is the top-level
/// synopsis, so it advertises the verbs that exist rather than the retired one.
/// `cflx client --help` being correct proves nothing about this text: it is a
/// hand-written `long_about` clap never regenerates from the subcommand set.
#[test]
fn top_level_help_synopsis_names_current_client_verbs_and_retires_enqueue() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let top_level = run_cli(tmp.path(), &["--help"], &[]);
    assert!(top_level.status.success(), "{}", stderr_of(&top_level));
    let help = stdout_of(&top_level);

    let synopsis = help
        .lines()
        .find(|line| line.trim_start().starts_with("client "))
        .unwrap_or_else(|| {
            panic!("the top-level help must describe the client namespace:\n{help}")
        });
    for verb in [
        "status",
        "mark",
        "start",
        "stop",
        "wait",
        "subscribe",
        "mcp",
    ] {
        assert!(
            synopsis.contains(verb),
            "the top-level synopsis must name {verb}:\n{synopsis}"
        );
    }

    for retired in ["enqueue", "notify"] {
        assert!(
            !help.contains(retired),
            "{retired} is retired and must not appear in the top-level help:\n{help}"
        );
    }
}

/// The control verbs are documented as the operator's own, and the namespace
/// must no longer offer an admission-shaped one.
#[test]
fn client_control_help_documents_operator_verbs_and_retires_enqueue() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let namespace = run_cli(tmp.path(), &["client", "--help"], &[]);
    assert!(namespace.status.success(), "{}", stderr_of(&namespace));
    let help = stdout_of(&namespace);
    for verb in [
        "status",
        "mark",
        "unmark",
        "start",
        "stop",
        "force-stop",
        "wait",
        "subscribe",
        "mcp",
    ] {
        assert!(
            help.contains(verb),
            "the namespace must offer {verb}:\n{help}"
        );
    }
    for retired in ["enqueue", "notify"] {
        assert!(
            !help.contains(retired),
            "{retired} is retired and must not be offered:\n{help}"
        );
    }

    let mark = run_cli(tmp.path(), &["client", "mark", "--help"], &[]);
    assert!(mark.status.success(), "{}", stderr_of(&mark));
    let mark_help = stdout_of(&mark);
    assert!(
        mark_help.contains("preserves every unrelated mark"),
        "mark help must state the target scope:\n{mark_help}"
    );
    assert!(
        mark_help.contains("does not construct queue intent"),
        "mark help must state that admission is not claimed:\n{mark_help}"
    );

    let start = run_cli(tmp.path(), &["client", "start", "--help"], &[]);
    assert!(start.status.success(), "{}", stderr_of(&start));
    let start_help = stdout_of(&start);
    assert!(
        start_help.contains("F5"),
        "start help must name its TUI equivalent:\n{start_help}"
    );
    assert!(
        start_help.contains("no target list"),
        "start help must say it takes no targets:\n{start_help}"
    );
}

/// Every argv the CLI itself rejects is rejected before an owner is contacted,
/// and a `--json` caller still gets its one envelope naming the operation it
/// attempted.
#[test]
fn client_subscribe_help_and_usage_rejects_a_malformed_invocation_before_any_request() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let instance = "a".repeat(32);
    let oversized_instance = "a".repeat(129);
    let cases: Vec<(Vec<&str>, &str, &str)> = vec![
        (
            vec![
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
                "--",
            ],
            "subscribe_set",
            "a callback with no program at all",
        ),
        (
            vec![
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
                "/bin/true",
            ],
            "subscribe_set",
            "a callback that skipped the separator",
        ),
        (
            vec!["subscribe", "set", "alpha", "--json", "--", "/bin/true"],
            "subscribe_set",
            "a registration that named no owner incarnation",
        ),
        (
            vec![
                "subscribe",
                "set",
                "../escape",
                "--instance-id",
                &instance,
                "--json",
                "--",
                "/bin/true",
            ],
            "subscribe_set",
            "an escaping change ID",
        ),
        (
            vec!["subscribe", "get", "alpha", "--instance-id", "", "--json"],
            "subscribe_get",
            "an empty instance ID",
        ),
        (
            vec![
                "subscribe",
                "clear",
                "alpha",
                "--instance-id",
                "has space",
                "--json",
            ],
            "subscribe_clear",
            "an instance ID with a separator in it",
        ),
        (
            vec![
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                ".reserved",
                "--json",
            ],
            "subscribe_get",
            "an instance ID starting with a reserved character",
        ),
        (
            vec![
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &oversized_instance,
                "--json",
            ],
            "subscribe_get",
            "an oversized instance ID",
        ),
        (
            vec!["subscribe", "get", "--instance-id", &instance, "--json"],
            "subscribe_get",
            "a request that named no proposal",
        ),
        (
            vec!["subscribe", "--json"],
            "subscribe_get",
            "a group with no operation",
        ),
    ];

    for (action, operation, what) in cases {
        let mut args = vec!["client"];
        args.extend(action);
        let output = run_cli(tmp.path(), &args, &[]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{what} must be a usage error, stderr={}",
            stderr_of(&output)
        );
        let stdout = stdout_of(&output);
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("{what}: stdout must be one envelope, got {stdout:?}: {e}"));
        assert_eq!(parsed["outcome"], "usage_error", "{what}");
        assert_eq!(parsed["operation"], operation, "{what}");
        assert!(
            std::fs::read_dir(tmp.path())
                .expect("temp dir readable")
                .next()
                .is_none(),
            "{what}: a rejected invocation must write nothing"
        );
    }
}

/// A flag *after* `--` is one of the callback's own arguments. Letting it
/// select the output contract would let a value decide how the caller is
/// answered, which is the same inference the whole-argument `--json` rule
/// already forbids.
#[test]
fn client_subscribe_help_and_usage_keeps_a_callback_flag_out_of_the_output_mode() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let absent = tmp.path().join("absent.sock");
    let socket = absent.display().to_string();
    let instance = "a".repeat(32);

    // `--json` is only ever the callback's, so the answer is the human line.
    let human = run_cli(
        tmp.path(),
        &[
            "client",
            "--unix-socket",
            &socket,
            "subscribe",
            "set",
            "alpha",
            "--instance-id",
            &instance,
            "--",
            "/bin/true",
            "--json",
        ],
        &[],
    );
    let stdout = stdout_of(&human);
    assert!(
        !stdout.trim_start().starts_with('{'),
        "a callback argument must not select machine output:\n{stdout}"
    );
    assert!(
        stdout.starts_with("subscribe_set: "),
        "the human line still names the operation:\n{stdout}"
    );

    // The same invocation with the flag before the separator is machine output.
    let machine = run_cli(
        tmp.path(),
        &[
            "client",
            "--unix-socket",
            &socket,
            "subscribe",
            "set",
            "alpha",
            "--instance-id",
            &instance,
            "--json",
            "--",
            "/bin/true",
            "--json",
        ],
        &[],
    );
    let stdout = stdout_of(&machine);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be one envelope, got {stdout:?}: {e}"));
    assert_eq!(parsed["operation"], "subscribe_set");
    assert!(!parsed["ok"].as_bool().expect("ok is a boolean"));
}

#[cfg(not(feature = "web-monitoring"))]
mod feature_disabled {
    use super::*;

    /// The MCP server has no envelope to emit — a host parsing stdout as
    /// JSON-RPC must not be handed prose — so its refusal is stderr-only, and
    /// it still happens before anything observable exists.
    #[test]
    fn feature_disabled_mcp_refuses_on_stderr_without_any_side_effect() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let output = run_cli(tmp.path(), &["client", "mcp"], &[]);
        assert_eq!(output.status.code(), Some(20));
        assert!(
            stdout_of(&output).is_empty(),
            "the protocol stream must carry nothing at all"
        );
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

    #[test]
    fn feature_disabled_client_refuses_before_any_side_effect() {
        let tmp = tempfile::tempdir().expect("temp dir");
        for action in [
            vec!["client", "status", "--json"],
            vec!["client", "mark", "alpha", "--json"],
            vec!["client", "unmark", "alpha", "--json"],
            vec!["client", "start", "--json"],
            vec!["client", "stop", "--json"],
            vec!["client", "force-stop", "--json"],
            vec!["client", "wait", "alpha", "--json"],
            vec![
                "client",
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                "owner",
                "--json",
                "--",
                "/bin/true",
            ],
            vec![
                "client",
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                "owner",
                "--json",
            ],
            vec![
                "client",
                "subscribe",
                "clear",
                "alpha",
                "--instance-id",
                "owner",
                "--json",
            ],
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

    /// Owner surfaces only the subscribe group needs.
    ///
    /// Off by default so every pre-existing test keeps describing the owner it
    /// always described: an owner with no execution-sink registry is a real
    /// shape — a process that predates them — and one whose router carries no
    /// transport marker is the shape the other groups exercise.
    #[derive(Default, Clone, Copy)]
    struct OwnerExtras {
        /// Bind an execution-sink registry and publish the capability.
        sinks: bool,
        /// The transport marker the router labels every request with.
        ///
        /// Production attaches this per listener. This fixture serves exactly
        /// one Unix socket, so a test that needs the refusal an exposed TCP
        /// listener gets injects `Tcp` here and then drives the *production*
        /// endpoint rule, rather than a stub the client talked to directly.
        transport: Option<conflux::web::remote_control_api::ApiTransport>,
    }

    impl OwnerExtras {
        /// The owner an operator registering a callback actually has: a sink
        /// registry, reached over the owner's own socket.
        fn sink_capable() -> Self {
            Self {
                sinks: true,
                transport: Some(conflux::web::remote_control_api::ApiTransport::Unix),
            }
        }
    }

    /// A real `/api/v2` router on a real Unix socket, plus the handles a test
    /// needs to drive it.
    struct Owner {
        socket: PathBuf,
        projection: Arc<Projection>,
        runtime: Arc<RemoteControlRuntime>,
        boundary: Arc<Boundary>,
        facts: Arc<ExecutionFactsStore>,
        sinks: Option<Arc<conflux::web::completion_sink::CompletionSinkRegistry>>,
        /// Monotonic dispatch identity for the facts store's absorption check.
        dispatch: AtomicUsize,
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

        /// Start an owner with the surfaces one notify test needs.
        async fn start_with(
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
            extras: OwnerExtras,
        ) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let socket = dir.path().join("cflx-api.sock");
            let mut owner = Self::start_layered(socket, executor, token, None, extras).await;
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
            let mut owner =
                Self::start_layered(socket, executor, token, Some(api), OwnerExtras::default())
                    .await;
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
            Self::start_layered(socket, executor, token, None, OwnerExtras::default()).await
        }

        async fn start_layered(
            socket: PathBuf,
            executor: Option<Arc<dyn RemoteControlExecutor>>,
            token: Option<&str>,
            api: Option<Arc<ApiSpy>>,
            extras: OwnerExtras,
        ) -> Self {
            let runtime = Arc::new(RemoteControlRuntime::new());
            if let Some(executor) = executor {
                runtime.bind(executor).await;
            }
            let facts = Arc::new(ExecutionFactsStore::new());
            runtime.bind_execution_facts(facts.clone());
            let boundary = Arc::new(Boundary::default());
            runtime.bind_run_boundary(boundary.clone());
            // Started before anything is admitted, because the registry becomes
            // the facts store's episode observer at construction: an episode
            // opened first would never reach it.
            let sinks = extras.sinks.then(|| {
                let registry = conflux::web::completion_sink::CompletionSinkRegistry::start(
                    runtime.projection().instance_id().to_string(),
                    facts.clone(),
                    runtime.execution_contract(),
                );
                runtime.bind_completion_sinks(registry.clone());
                registry
            });

            let auth = RemoteControlAuth::new(token.map(str::to_string), &[])
                .expect("test auth policy is valid");
            let app = router(
                RemoteControlState::new(runtime.projection(), Arc::new(auth), runtime.clone())
                    .with_gate(runtime.gate())
                    .with_execution_facts(runtime.execution_facts())
                    .with_execution_contract(runtime.execution_contract())
                    .with_completion_sinks(runtime.completion_sinks()),
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
            let app = match extras.transport {
                Some(transport) => app.layer(axum::Extension(transport)),
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
                facts,
                sinks,
                dispatch: AtomicUsize::new(0),
                shutdown,
                task,
                _dir: None,
            }
        }

        fn instance_id(&self) -> String {
            self.projection.instance_id().to_string()
        }

        /// Admit one change the way every admission source ultimately does, and
        /// return the execution episode it opened.
        ///
        /// The reducer is built per call and tracks only this change: the facts
        /// store re-projects exactly the changes the state it is handed tracks,
        /// so an earlier admission keeps the episode it already opened.
        fn admit(&self, change_id: &str) -> String {
            use conflux::events::{ExecutionEvent, OperatorCommandEffect};
            use conflux::orchestration::state::{OrchestratorState, ReducerCommand};

            let mut reducer = OrchestratorState::new(vec![change_id.to_string()], 0);
            reducer.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
            let event = ExecutionEvent::OperatorCommandApplied {
                effect: OperatorCommandEffect::QueueDelta {
                    change_id: change_id.to_string(),
                    queued: true,
                },
            };
            reducer.apply_execution_event(&event);
            let dispatch = self.dispatch.fetch_add(1, Ordering::SeqCst) as u64 + 1;
            self.facts
                .observe(dispatch, &event, Some(&reducer), chrono::Utc::now());
            self.facts
                .change(change_id)
                .execution_id
                .expect("admission opens an execution episode")
        }

        /// Admit one change and return once the registry knows its episode.
        ///
        /// Admission publishes the episode to the registry's dispatcher task,
        /// which is what creates the entry every sink request resolves against.
        /// This waits on that state transition rather than on a duration: the
        /// deadline is only a hang guard, and reaching it is a failure.
        async fn admit_registered(&self, change_id: &str) -> String {
            let execution_id = self.admit(change_id);
            let registry = self
                .sinks
                .as_ref()
                .expect("a sink-capable owner holds a registry");
            let instance_id = self.instance_id();
            for _ in 0..1_000 {
                if registry
                    .view(&execution_id, &instance_id, change_id)
                    .is_ok()
                {
                    return execution_id;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            panic!("the registry never opened an entry for execution {execution_id}");
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
    fn cli_surface_exposes_only_the_operator_verbs_wait_subscribe_and_mcp() {
        let tmp = neutral_cwd();
        let output = run_cli(tmp.path(), &["client", "--help"], &[]);
        assert!(output.status.success(), "{}", stderr_of(&output));
        let help = stdout_of(&output);
        for expected in [
            "status",
            "mark",
            "unmark",
            "start",
            "stop",
            "force-stop",
            "wait",
            "subscribe",
            "mcp",
            "--project-dir",
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
        // Retired: an admission-shaped verb is exactly what this namespace no
        // longer offers, and the remaining ones are not orchestration.
        for absent in [
            "enqueue", "notify", "retry", "resolve", "worktree", "archive",
        ] {
            assert!(
                !help.contains(&format!("  {absent} ")),
                "client must not offer a {absent} subcommand:\n{help}"
            );
        }
    }

    /// The MCP server's own help has to state the things a caller gets wrong:
    /// that it is still a client, that the tool set is the compact three, and
    /// that there is no wait tool to reach for.
    #[test]
    fn cli_surface_documents_the_mcp_server_as_a_bounded_client() {
        let tmp = neutral_cwd();
        let output = run_cli(tmp.path(), &["client", "mcp", "--help"], &[]);
        assert!(output.status.success(), "{}", stderr_of(&output));
        let help = stdout_of(&output);

        for tool in ["cflx_status", "cflx_control", "cflx_subscribe"] {
            assert!(help.contains(tool), "help must name {tool}:\n{help}");
        }
        for retired in ["cflx_enqueue", "cflx_notify"] {
            assert!(
                !help.contains(retired),
                "{retired} is retired and must not be named:\n{help}"
            );
        }
        assert!(
            help.contains("cflx_wait is deliberately absent"),
            "help must explain why there is no wait tool:\n{help}"
        );
        assert!(
            help.contains("token value is never accepted"),
            "help must state the credential rule:\n{help}"
        );
        // The connection options are the namespace's, and the protocol is not
        // reachable through them.
        for forbidden in ["--expected-revision", "--idempotency-key", "--auth-token "] {
            assert!(
                !help.contains(forbidden),
                "help must not expose {forbidden}:\n{help}"
            );
        }
    }

    #[test]
    fn cli_surface_rejects_a_malformed_change_id_as_usage() {
        let tmp = neutral_cwd();
        for bad in ["../escape", ".hidden", "-leading", "has space", ""] {
            let output = run_cli(tmp.path(), &["client", "mark", bad, "--json"], &[]);
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
            assert_eq!(parsed["operation"], "control_mark");
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
                vec!["client", "mark", "../escape", "--json"],
                "control_mark",
                "an invalid change ID",
            ),
            (
                vec!["client", "unmark", "", "--json"],
                "control_unmark",
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
                vec!["client", "mark", "--json"],
                "control_mark",
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
            vec!["client", "mark", "../escape"],
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
                "mark",
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
                        "mark",
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
                vec!["mark".into(), "alpha".into()],
                "owner_not_command_capable",
                7,
            ),
            (
                true,
                vec!["mark".into(), "ghost".into()],
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

        let marked = tokio::task::spawn_blocking({
            let cwd = tmp.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &[
                        "client",
                        "--unix-socket",
                        &socket,
                        "mark",
                        "alpha",
                        "--json",
                    ],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        assert_eq!(envelope(&marked)["outcome"], "owner_not_command_capable");
        assert_eq!(marked.status.code(), Some(7));
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
    // control
    // ========================================================================
    //
    // The client's control verbs are the operator's own, and the properties
    // that matter are all about what the *owner* was asked to do. So every test
    // here drives the compiled binary against a real `/api/v2` endpoint backed
    // by a spy executor, and asserts the exact command sequence the endpoint
    // saw. An in-process call could not distinguish "the client submitted only
    // `SetExecutionMark`" from "the client submitted whatever it liked and the
    // envelope said otherwise", which is the whole thing this group exists for.

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

    /// Run one `cflx client` invocation against this owner's socket.
    ///
    /// Blocking work off the runtime thread, the way every other group here
    /// drives the binary.
    async fn run_client(socket: &str, cwd: &Path, tail: &[&str]) -> Output {
        let socket = socket.to_string();
        let cwd = cwd.to_path_buf();
        let tail: Vec<String> = tail.iter().map(|value| (*value).to_string()).collect();
        tokio::task::spawn_blocking(move || {
            let mut args = vec!["client".to_string(), "--unix-socket".to_string(), socket];
            args.extend(tail);
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            run_cli(&cwd, &borrowed, &[])
        })
        .await
        .expect("the client invocation must not panic")
    }

    /// Run one control verb with `--json` against this owner.
    async fn control(owner: &Owner, tail: &[&str]) -> Output {
        let cwd = neutral_cwd();
        let mut args = tail.to_vec();
        args.push("--json");
        let output = run_client(&owner.socket(), cwd.path(), &args).await;
        drop(cwd);
        output
    }

    /// A change with its execution mark already set.
    fn marked(id: &str, app_mode: &str, display_status: &str) -> ChangeResource {
        let mut change = change(id, app_mode, display_status);
        change.execution_marked = true;
        change
    }

    /// The command types the endpoint actually executed, in order.
    fn submitted_names(spy: &SpyExecutor) -> Vec<&'static str> {
        spy.calls()
            .iter()
            .map(|command| match command {
                CommandSpec::SetExecutionMark { .. } => "set_execution_mark",
                CommandSpec::SetAllExecutionMarks {} => "set_all_execution_marks",
                CommandSpec::Start => "start",
                CommandSpec::Stop => "stop",
                CommandSpec::ForceStop => "force_stop",
                CommandSpec::SetQueueIntent { .. } => "set_queue_intent",
                CommandSpec::RetryChange { .. } => "retry_change",
                CommandSpec::RetryErrors { .. } => "retry_errors",
                _ => "other",
            })
            .collect()
    }

    /// The core of the change: marking is target-scoped, so an operator's
    /// unrelated next-run intent survives an agent's request. The old client
    /// could only answer this situation by refusing outright.
    #[tokio::test]
    async fn control_mark_preserves_unrelated_marks_and_submits_only_mark_commands() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![
                change("alpha", "select", "not queued"),
                marked("beta", "select", "not queued"),
                change("gamma", "select", "not queued"),
            ],
        ));

        let output = control(&owner, &["mark", "alpha", "gamma"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "marked", "{}", stderr_of(&output));
        assert_eq!(output.status.code(), Some(0));

        assert_eq!(
            submitted_names(&spy),
            vec!["set_execution_mark", "set_execution_mark"],
            "only the two named targets are written, and only as marks"
        );
        assert_eq!(
            spy.calls(),
            vec![
                CommandSpec::SetExecutionMark {
                    change_id: "alpha".to_string(),
                    marked: true
                },
                CommandSpec::SetExecutionMark {
                    change_id: "gamma".to_string(),
                    marked: true
                },
            ],
            "nothing addressed beta, and nothing addressed lifecycle or queue state"
        );

        // The per-target audit is in request order and names every target.
        let targets = parsed["detail"]["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0]["change_id"], "alpha");
        assert_eq!(targets[0]["changed"], true);
        assert_eq!(targets[1]["change_id"], "gamma");
        owner.stop().await;
    }

    /// A mark reports mark state and nothing else. No episode is named, because
    /// the write created none, and no admission is claimed.
    #[tokio::test]
    async fn control_mark_settlement_claims_no_admission() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let output = control(&owner, &["mark", "alpha"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "marked", "{}", stderr_of(&output));
        assert_eq!(parsed["operation"], "control_mark");
        assert_eq!(parsed["change_id"], "alpha");
        assert!(
            parsed.as_object().unwrap().get("execution_id").is_none(),
            "a mark write creates no execution episode to name: {parsed}"
        );
        let rendered = parsed.to_string();
        for forbidden in ["queue_intent", "admitted", "already_admitted"] {
            assert!(!rendered.contains(forbidden), "{forbidden}: {rendered}");
        }
        assert_eq!(submitted_names(&spy), vec!["set_execution_mark"]);
        owner.stop().await;
    }

    /// Desired state that already holds is a success with no command at all: the
    /// owner is already in the state the operator asked for.
    #[tokio::test]
    async fn control_mark_of_an_already_marked_target_settles_unchanged_without_a_command() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![marked("alpha", "select", "not queued")],
        ));

        let output = control(&owner, &["mark", "alpha"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "unchanged", "{}", stderr_of(&output));
        assert_eq!(output.status.code(), Some(0), "unchanged is a success");
        assert_eq!(parsed["detail"]["targets"][0]["changed"], false);
        assert!(
            spy.calls().is_empty(),
            "a settled desired state needs no command: {:?}",
            spy.calls()
        );
        owner.stop().await;
    }

    /// A terminal row is a fact about the target, and the *shared service* owns
    /// the reason. The client submits and reports what came back rather than
    /// reimplementing the mark matrix.
    #[tokio::test]
    async fn control_mark_of_a_terminal_target_is_a_reasoned_unchanged_no_op() {
        let spy = SpyExecutor::new();
        spy.script(vec![Ok(ExecutionSummary::no_op(
            "'alpha' is terminal and carries no next-run intent",
        ))]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "merged")],
        ));

        let output = control(&owner, &["mark", "alpha"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "unchanged", "{}", stderr_of(&output));
        assert_eq!(
            submitted_names(&spy),
            vec!["set_execution_mark"],
            "the shared service decides, so the command still goes out"
        );
        let reason = parsed["detail"]["targets"][0]["reason"].as_str().unwrap();
        assert!(reason.contains("next-run intent"), "{reason}");
        owner.stop().await;
    }

    #[tokio::test]
    async fn control_unmark_is_target_scoped_and_stops_nothing() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![
                marked("alpha", "select", "not queued"),
                marked("beta", "select", "not queued"),
            ],
        ));

        let output = control(&owner, &["unmark", "alpha"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "unmarked", "{}", stderr_of(&output));
        assert_eq!(parsed["operation"], "control_unmark");
        assert_eq!(
            spy.calls(),
            vec![CommandSpec::SetExecutionMark {
                change_id: "alpha".to_string(),
                marked: false
            }],
            "beta's mark is untouched, and nothing stopped or dequeued anything"
        );
        owner.stop().await;
    }

    /// An unknown target is a request-level refusal: no command record exists
    /// for any named proposal, including the ones that were perfectly valid.
    #[tokio::test]
    async fn control_refuses_an_unknown_proposal_before_any_command() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let output = control(&owner, &["mark", "alpha", "missing"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "change_not_found");
        assert_eq!(output.status.code(), Some(9));
        assert_eq!(parsed["change_id"], "missing");
        assert!(
            spy.calls().is_empty(),
            "classification happens before submission: {:?}",
            spy.calls()
        );
        owner.stop().await;
    }

    /// Shape refusals never reach an owner at all.
    #[tokio::test]
    async fn control_refuses_a_malformed_target_list_before_contact() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        let duplicated = control(&owner, &["mark", "alpha", "alpha"]).await;
        let parsed = envelope(&duplicated);
        assert_eq!(parsed["outcome"], "usage_error");
        assert_eq!(duplicated.status.code(), Some(2));
        assert!(
            parsed["message"]
                .as_str()
                .unwrap()
                .contains("more than once"),
            "{parsed}"
        );

        let many: Vec<String> = (0..65).map(|n| format!("change-{n}")).collect();
        let mut args = vec!["mark".to_string()];
        args.extend(many);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let oversized = control(&owner, &borrowed).await;
        assert_eq!(envelope(&oversized)["outcome"], "usage_error");

        assert!(spy.calls().is_empty());
        owner.stop().await;
    }

    /// Start is F5: it submits the shared intent and consumes whatever marks the
    /// owner holds. It never writes a mark of its own, and it never invents a
    /// target set.
    #[tokio::test]
    async fn control_start_submits_only_the_shared_start_intent() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![
                marked("alpha", "select", "not queued"),
                marked("beta", "select", "not queued"),
            ],
        ));

        let output = control(&owner, &["start"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "accepted", "{}", stderr_of(&output));
        assert_eq!(parsed["operation"], "control_start");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            spy.calls(),
            vec![CommandSpec::Start],
            "exactly the shared intent, with no mark rewriting on the way"
        );
        owner.stop().await;
    }

    #[tokio::test]
    async fn control_stop_and_force_stop_submit_their_own_shared_intents() {
        for (verb, operation, expected) in [
            ("stop", "control_stop", CommandSpec::Stop),
            ("force-stop", "control_force_stop", CommandSpec::ForceStop),
        ] {
            let spy = SpyExecutor::new();
            let owner = Owner::start(Some(spy.clone()), None).await;
            owner.publish(snapshot(
                "running",
                vec![change("alpha", "running", "applying")],
            ));

            let output = control(&owner, &[verb]).await;
            let parsed = envelope(&output);
            assert_eq!(
                parsed["outcome"],
                "accepted",
                "{verb}: {}",
                stderr_of(&output)
            );
            assert_eq!(parsed["operation"], operation);
            assert_eq!(spy.calls(), vec![expected], "{verb}");
            owner.stop().await;
        }
    }

    /// "Start only these" is not something the shared transaction can express,
    /// so it is refused from the arguments rather than approximated.
    #[tokio::test]
    async fn control_lifecycle_refuses_a_target_list_before_contact() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));

        for verb in ["start", "stop", "force-stop"] {
            let output = control(&owner, &[verb, "alpha"]).await;
            // Clap owns the arity, so this is the namespace's usage contract.
            assert_eq!(envelope(&output)["outcome"], "usage_error", "{verb}");
            assert_eq!(output.status.code(), Some(2), "{verb}");
        }
        assert!(spy.calls().is_empty());
        owner.stop().await;
    }

    /// The guarantee the whole change exists for, asserted against the endpoint
    /// rather than against the client's own prose: no control path reaches the
    /// scheduler's admitted set directly.
    #[tokio::test]
    async fn control_never_constructs_queue_intent_retry_or_a_bulk_mark() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "running",
            vec![
                change("alpha", "running", "not queued"),
                change("beta", "running", "error"),
            ],
        ));

        // A live owner offering queue intent, and a row carrying retryable
        // evidence: the two routes the previous client would have taken.
        let alpha = control(&owner, &["mark", "alpha"]).await;
        assert_eq!(
            envelope(&alpha)["outcome"],
            "marked",
            "{}",
            stderr_of(&alpha)
        );
        let beta = control(&owner, &["mark", "beta"]).await;
        assert_eq!(envelope(&beta)["outcome"], "marked", "{}", stderr_of(&beta));
        let started = control(&owner, &["start"]).await;
        assert_eq!(envelope(&started)["outcome"], "accepted");

        let names = submitted_names(&spy);
        assert_eq!(
            names,
            vec!["set_execution_mark", "set_execution_mark", "start"],
            "the only commands a client may submit are marks and shared lifecycle intents"
        );
        for forbidden in [
            "set_queue_intent",
            "retry_change",
            "retry_errors",
            "set_all_execution_marks",
        ] {
            assert!(
                !names.contains(&forbidden),
                "{forbidden} must be unreachable"
            );
        }
        owner.stop().await;
    }

    /// A later target failing after earlier ones settled is partial intent: the
    /// settled marks are real, the audit lists exactly the records created, and
    /// nothing claims a rollback.
    #[tokio::test]
    async fn control_reports_partial_intent_with_an_exact_audit_and_no_rollback() {
        let spy = SpyExecutor::new();
        spy.script(vec![
            Ok(ExecutionSummary::changed("marked alpha")),
            Ok(ExecutionSummary::changed("marked beta")),
            Err(CommandFailure::new(
                ErrorCode::TargetIneligible,
                "gamma cannot be marked right now",
            )),
        ]);
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![
                change("alpha", "select", "not queued"),
                change("beta", "select", "not queued"),
                change("gamma", "select", "not queued"),
            ],
        ));

        let output = control(&owner, &["mark", "alpha", "beta", "gamma"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "partial_intent");
        assert_eq!(output.status.code(), Some(15));
        assert_eq!(parsed["detail"]["rolled_back"], false);

        // Exactly the three records the endpoint made, in submission order.
        let commands = parsed["detail"]["commands_submitted"].as_array().unwrap();
        assert_eq!(commands.len(), 3, "{parsed}");
        for (index, change_id) in ["alpha", "beta", "gamma"].iter().enumerate() {
            assert_eq!(commands[index]["command"], "set_execution_mark");
            assert_eq!(commands[index]["change_id"], *change_id);
        }
        // Two targets settled; the third produced a record and no result.
        let targets = parsed["detail"]["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 2);
        assert!(parsed["message"].as_str().unwrap().contains("rolled back"));
        // And crucially no fourth command: the client must not try to undo the
        // marks it already settled.
        assert_eq!(spy.call_count(), 3, "{:?}", spy.calls());
        owner.stop().await;
    }

    /// A recomputation must not resubmit a command that already settled, and it
    /// must not lose one either. The audit list is what proves both.
    #[tokio::test]
    async fn control_recomputes_after_a_stale_revision_without_repeating_a_settled_effect() {
        let spy = SpyExecutor::new();
        let api = ApiSpy::new();
        let owner = Owner::start_intercepted(Some(spy.clone()), None, api.clone()).await;
        owner.publish(snapshot(
            "select",
            vec![
                change("alpha", "select", "not queued"),
                change("beta", "select", "not queued"),
            ],
        ));

        // One advance, injected in the only window that can produce a stale
        // submission: after the client took its observation, before the real
        // endpoint compares `expected_revision`. Deliberately routing-irrelevant
        // progress — a third change appears, which the request never names — so
        // the recomputed attempt must reach the same conclusion at a newer
        // revision rather than a different one.
        let projection = owner.projection.clone();
        api.inject_before_commands(vec![Box::new(move || {
            projection.apply_state(
                "test_snapshot",
                None,
                serde_json::json!({}),
                snapshot(
                    "select",
                    vec![
                        change("alpha", "select", "not queued"),
                        change("beta", "select", "not queued"),
                        change("gamma", "select", "not queued"),
                    ],
                ),
            );
        })]);

        let output = control(&owner, &["mark", "alpha", "beta"]).await;
        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "marked", "{}", stderr_of(&output));

        // The refused submission created no record and is absent; the
        // recomputed one is present exactly once, and so is beta's.
        let commands = parsed["detail"]["commands_submitted"].as_array().unwrap();
        assert_eq!(
            commands.len(),
            spy.call_count(),
            "the audit equals the records the endpoint actually made: {parsed}"
        );
        assert_eq!(
            submitted_names(&spy),
            vec!["set_execution_mark", "set_execution_mark"],
            "a stale submission produced no record, and nothing was submitted twice"
        );

        // The interceptor saw the whole conversation, so the audit can be
        // checked against the endpoint's own answers rather than against the
        // client's account of them: exactly the POSTs that came back with a
        // command record are the ones listed, and the refused one is not.
        let exchanges = api.exchanges();
        assert_eq!(
            exchanges.len(),
            3,
            "one refused submission, its recomputation, and beta's: {exchanges:#?}"
        );
        assert_eq!(exchanges[0].status, 409, "{exchanges:#?}");
        assert_eq!(exchanges[0].error_code.as_deref(), Some("stale_revision"));
        assert!(
            exchanges[0].record_id.is_none(),
            "a refused submission creates no record to audit"
        );
        let recorded: Vec<&str> = exchanges
            .iter()
            .filter_map(|exchange| exchange.record_id.as_deref())
            .collect();
        let audited: Vec<&str> = commands
            .iter()
            .map(|entry| entry["command_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            audited, recorded,
            "the audit is exactly the records the owner made, in order"
        );
        for exchange in &exchanges {
            assert_eq!(exchange.command_type, "set_execution_mark");
        }
        // Every submission carries its own idempotency key, because the key is
        // bound to the expected revision: reusing one across a recomputation
        // would be refused as a mismatch rather than replayed.
        let keys: std::collections::BTreeSet<&str> = exchanges
            .iter()
            .map(|exchange| exchange.idempotency_key.as_str())
            .collect();
        assert_eq!(keys.len(), exchanges.len(), "{exchanges:#?}");
        assert!(
            exchanges[1].expected_revision > exchanges[0].expected_revision,
            "the recomputation submits against the revision it just reread: {exchanges:#?}"
        );
        assert!(
            api.requests()
                .iter()
                .any(|request| request == "GET /api/v2/state"),
            "a recomputation rereads the owner's state rather than guessing"
        );
        owner.stop().await;
    }

    /// A new incarnation cannot prove whether a command submitted to the
    /// previous one settled, so the request *stops* — it does not report the
    /// settled marks as standing (they belonged to a process that is gone) and
    /// it does not walk the rest of the target list into an owner that never saw
    /// the first command.
    ///
    /// Two real owners behind one relay that switches after the first POST, so
    /// the client sees genuine v2 responses from two genuine incarnations.
    #[tokio::test]
    async fn control_aborts_when_the_socket_starts_serving_a_different_incarnation() {
        let first_spy = SpyExecutor::new();
        let second_spy = SpyExecutor::new();
        let first = Owner::start(Some(first_spy.clone()), None).await;
        let second = Owner::start(Some(second_spy.clone()), None).await;
        for owner in [&first, &second] {
            owner.publish(snapshot(
                "select",
                vec![
                    change("alpha", "select", "not queued"),
                    change("beta", "select", "not queued"),
                    change("gamma", "select", "not queued"),
                ],
            ));
        }

        let relay_dir = tempfile::tempdir().expect("temp dir");
        let relay = relay_dir.path().join("relay.sock");
        let cancel = switch_after_first_post(
            relay.clone(),
            PathBuf::from(first.socket()),
            PathBuf::from(second.socket()),
        )
        .await;

        let cwd = neutral_cwd();
        let output = run_client(
            relay.to_str().unwrap(),
            cwd.path(),
            &["mark", "alpha", "beta", "gamma", "--json"],
        )
        .await;
        let parsed = envelope(&output);
        assert_eq!(
            parsed["outcome"],
            "owner_restarted",
            "{}",
            stderr_of(&output)
        );
        assert_eq!(output.status.code(), Some(8));
        assert_ne!(
            parsed["detail"]["expected_instance_id"],
            parsed["detail"]["observed_instance_id"]
        );
        assert_eq!(
            parsed["detail"]["stopped_at"], "beta",
            "the request stops at the target whose record revealed the switch"
        );

        assert_eq!(
            first_spy.call_count(),
            1,
            "only the first mark reached the incarnation the client observed: {:?}",
            first_spy.calls()
        );
        assert_eq!(
            second_spy.call_count(),
            1,
            "exactly the in-flight command reached the replacement, and gamma was never \
             submitted at all: {:?}",
            second_spy.calls()
        );

        cancel.cancel();
        first.stop().await;
        second.stop().await;
    }

    /// A headless owner serves reads and binds no executor. Discovering that
    /// from a refusal would mean submitting a command already known to be
    /// unrunnable, so capability is read first.
    #[tokio::test]
    async fn control_reports_owner_not_command_capable_without_touching_the_workspace() {
        let owner = Owner::start(None, None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        let workspace = neutral_cwd();
        let before: Vec<_> = std::fs::read_dir(workspace.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        for tail in [vec!["mark", "alpha"], vec!["start"]] {
            let output = run_client(
                &owner.socket(),
                workspace.path(),
                &[tail.clone(), vec!["--json"]].concat(),
            )
            .await;
            assert_eq!(
                envelope(&output)["outcome"],
                "owner_not_command_capable",
                "{tail:?}"
            );
            assert_eq!(output.status.code(), Some(7), "{tail:?}");
        }

        let after: Vec<_> = std::fs::read_dir(workspace.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            before, after,
            "a refused control command must not touch the workspace"
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
    // client_subscribe
    // ========================================================================
    //
    // `cflx client subscribe` is the shell-facing adapter over the same
    // proposal-scoped subscription registry `cflx client mcp` exposes as
    // `cflx_subscribe`. Everything here therefore drives the compiled binary
    // against a real registry behind the real `/api/v2` endpoints: the
    // properties that matter — argv boundaries survive the shell, mutation is
    // socket-only, a subscription may precede admission, an owner restart is
    // typed — are all invisible from an in-process call.

    /// A subscription-capable owner that has admitted nothing.
    ///
    /// Deliberately without an execution: the whole point of keying a
    /// subscription by the proposal is that it works before there is an episode
    /// to name.
    async fn subscribe_owner(spy: Arc<SpyExecutor>) -> Owner {
        let owner = Owner::start_with(Some(spy), None, OwnerExtras::sink_capable()).await;
        owner.publish(snapshot(
            "select",
            vec![
                change("alpha", "select", "not queued"),
                change("beta", "select", "not queued"),
            ],
        ));
        owner.contract(merged_contract("main"));
        owner
    }

    /// The whole round trip through the owner that already exists: register,
    /// read back, remove. It must reach the *same* module the MCP tool calls,
    /// and it must not become a workflow command on the way — a subscription
    /// creates no command record and cannot move a proposal, so the spy behind
    /// the real command endpoint must never see anything.
    #[tokio::test]
    async fn client_subscribe_routes_through_existing_owner() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let set = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
                "--",
                "/bin/true",
            ],
        )
        .await;
        let parsed = envelope(&set);
        assert_eq!(parsed["operation"], "subscribe_set");
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&set));
        assert_eq!(parsed["ok"], true);
        assert_eq!(set.status.code(), Some(0));
        assert_eq!(parsed["change_id"], "alpha");
        assert_eq!(parsed["instance_id"], instance.as_str());
        assert_eq!(
            parsed["detail"]["subscriptions"][0]["sink"]["command"][0],
            "/bin/true"
        );
        assert_eq!(
            parsed["detail"]["subscriptions"][0]["sink"]["notify_blocked"],
            false
        );
        // Registering before admission is the case this resource exists for, so
        // no episode is named and none is synthesized.
        assert!(
            parsed["detail"]["subscriptions"][0]
                .as_object()
                .unwrap()
                .get("execution_id")
                .is_none(),
            "{parsed}"
        );

        let read = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        let parsed = envelope(&read);
        assert_eq!(parsed["operation"], "subscribe_get");
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&read));
        assert_eq!(parsed["detail"]["subscriptions"][0]["subscribed"], true);
        assert_eq!(
            parsed["detail"]["subscriptions"][0]["sink"]["command"][0],
            "/bin/true"
        );

        let cleared = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "clear",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        let parsed = envelope(&cleared);
        assert_eq!(parsed["operation"], "subscribe_clear");
        assert_eq!(parsed["outcome"], "cleared", "{}", stderr_of(&cleared));
        assert_eq!(cleared.status.code(), Some(0));

        let after = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        let parsed = envelope(&after);
        assert_eq!(parsed["detail"]["subscriptions"][0]["subscribed"], false);
        assert!(parsed["detail"]["subscriptions"][0]["sink"].is_null());

        assert_eq!(
            spy.call_count(),
            0,
            "a subscription is observability, not a workflow command"
        );
        owner.stop().await;
    }

    /// One request addresses several proposals, and clearing removes only the
    /// ones it names.
    #[tokio::test]
    async fn client_subscribe_addresses_several_proposals_and_clears_only_those() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let set = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "beta",
                "--instance-id",
                &instance,
                "--json",
                "--",
                "/bin/true",
            ],
        )
        .await;
        let parsed = envelope(&set);
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&set));
        let subscriptions = parsed["detail"]["subscriptions"].as_array().unwrap();
        assert_eq!(subscriptions.len(), 2);
        for (index, change_id) in ["alpha", "beta"].iter().enumerate() {
            assert_eq!(subscriptions[index]["change_id"], *change_id);
            assert_eq!(subscriptions[index]["subscribed"], true);
        }
        // A multi-target request names no single change: `change_id` is a
        // scalar, and naming the first would misdescribe the request.
        assert!(parsed.as_object().unwrap().get("change_id").is_none());

        let cleared = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "clear",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        assert_eq!(envelope(&cleared)["outcome"], "cleared");

        let read = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "beta",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        assert_eq!(
            envelope(&read)["detail"]["subscriptions"][0]["subscribed"],
            true,
            "clearing alpha must leave beta alone"
        );
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    /// A subscription registered before admission binds the episode the owner
    /// later opens, and a read reports that binding. This is the gap an
    /// execution-scoped registration could not cover: at registration time there
    /// was no execution ID to name.
    #[tokio::test]
    async fn client_subscribe_binds_the_episode_the_owner_later_opens() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let set = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
                "--",
                "/bin/true",
            ],
        )
        .await;
        let parsed = envelope(&set);
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&set));
        assert!(
            parsed.as_object().unwrap().get("execution_id").is_none(),
            "no episode exists yet, and none may be synthesized: {parsed}"
        );

        // The owner admits the work through the same path every admission
        // source takes, which opens the episode the subscription binds.
        let execution_id = owner.admit_registered("alpha").await;

        let read = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        let parsed = envelope(&read);
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&read));
        assert_eq!(
            parsed["execution_id"],
            execution_id.as_str(),
            "the read names the episode the owner bound: {parsed}"
        );
        assert_eq!(
            parsed["detail"]["subscriptions"][0]["execution_id"],
            execution_id.as_str()
        );
        assert_eq!(parsed["detail"]["subscriptions"][0]["subscribed"], true);
        assert_eq!(
            parsed["detail"]["subscriptions"][0]["terminal_dispatched"], false,
            "the episode is live, not finished"
        );

        // And none of it was a workflow command.
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    /// Everything after `--` is one argv element, exactly as typed. The values
    /// here are the ones a shell would have mangled: an embedded space, a
    /// leading hyphen, a glob, a variable reference, and a command separator.
    /// The owner stores them verbatim because it never sees a shell at all.
    #[tokio::test]
    async fn client_subscribe_preserves_every_argv_boundary_without_a_shell() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let argv = [
            "/absolute/callback",
            "--flag",
            "one argument",
            "*",
            "$HOME",
            "; rm -rf /",
            "--json",
        ];
        let mut args = vec![
            "subscribe",
            "set",
            "alpha",
            "--instance-id",
            &instance,
            "--json",
            "--",
        ];
        args.extend_from_slice(&argv);
        let output = run_client(&socket, tmp.path(), &args).await;

        let parsed = envelope(&output);
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&output));
        let stored = parsed["detail"]["subscriptions"][0]["sink"]["command"]
            .as_array()
            .expect("the socket that registered the argv is told it back")
            .iter()
            .map(|value| value.as_str().expect("argv elements are strings"))
            .collect::<Vec<_>>();
        assert_eq!(
            stored, argv,
            "each argument after `--` must survive as exactly one argv element"
        );
        // The trailing `--json` was callback data, not a request for machine
        // output: the mode came from the flag before the separator.
        assert_eq!(stored.last(), Some(&"--json"));
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    /// `blocked` is a non-terminal attention edge, so it is delivered only when
    /// the caller asked for it.
    #[tokio::test]
    async fn client_subscribe_blocked_delivery_is_an_explicit_opt_in() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        let without = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
                "--",
                "/bin/true",
            ],
        )
        .await;
        assert_eq!(
            envelope(&without)["detail"]["subscriptions"][0]["sink"]["notify_blocked"],
            false
        );

        let with = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--blocked",
                "--json",
                "--",
                "/bin/true",
            ],
        )
        .await;
        assert_eq!(
            envelope(&with)["detail"]["subscriptions"][0]["sink"]["notify_blocked"],
            true
        );
        owner.stop().await;
    }

    /// Human mode is one concise line on stdout, with nothing of the machine
    /// contract leaking into it.
    #[tokio::test]
    async fn client_subscribe_human_output_is_one_concise_line() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();

        let output = run_client(
            &owner.socket(),
            tmp.path(),
            &[
                "subscribe",
                "set",
                "alpha",
                "--instance-id",
                &instance,
                "--",
                "/bin/true",
            ],
        )
        .await;
        let stdout = stdout_of(&output);
        assert_eq!(stdout.lines().count(), 1, "human output must be one line");
        assert!(stdout.starts_with("subscribe_set: subscribed"), "{stdout}");
        assert!(!stdout.contains("schema_version"), "{stdout}");
        assert_eq!(output.status.code(), Some(0));
        owner.stop().await;
    }

    /// Subscriptions are process-local, so a caller holding one from a previous
    /// owner is holding nothing. That is `owner_restarted` — and emphatically
    /// not a silent registration against the replacement.
    #[tokio::test]
    async fn client_subscribe_reports_a_replaced_owner_incarnation() {
        let spy = SpyExecutor::new();
        let owner = subscribe_owner(spy.clone()).await;
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();
        // Well-formed, and provably not this owner: instance IDs are 32 hex
        // digits, so a run of zeroes parses and cannot collide.
        let stale_instance = "0".repeat(32);

        for action in ["set", "get", "clear"] {
            let mut args = vec![
                "subscribe",
                action,
                "alpha",
                "--instance-id",
                &stale_instance,
                "--json",
            ];
            if action == "set" {
                args.extend_from_slice(&["--", "/bin/true"]);
            }
            let output = run_client(&socket, tmp.path(), &args).await;
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "owner_restarted", "{action}");
            assert_eq!(output.status.code(), Some(8), "{action}");
            assert_eq!(parsed["detail"]["expected_instance_id"], stale_instance);
            assert_eq!(parsed["detail"]["observed_instance_id"], instance.as_str());
        }

        // Nothing was registered against the owner that is actually there.
        let read = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        assert_eq!(
            envelope(&read)["detail"]["subscriptions"][0]["subscribed"],
            false
        );
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    /// An owner that binds no registry has no such route. That is an
    /// owner-capability fact, published as a capability, and the client reads it
    /// before submitting anything — otherwise a 404 would be indistinguishable
    /// from an ordinary refusal.
    #[tokio::test]
    async fn client_subscribe_reports_an_owner_without_the_surface_as_unsupported() {
        let spy = SpyExecutor::new();
        let owner = Owner::start(Some(spy.clone()), None).await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        owner.contract(merged_contract("main"));
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        for action in ["set", "get", "clear"] {
            let mut args = vec![
                "subscribe",
                action,
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ];
            if action == "set" {
                args.extend_from_slice(&["--", "/bin/true"]);
            }
            let output = run_client(&socket, tmp.path(), &args).await;
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "unsupported_owner", "{action}");
            assert_eq!(output.status.code(), Some(26), "{action}");
            assert!(
                stderr_of(&output).contains("unsupported_owner"),
                "the diagnostic belongs on stderr"
            );
        }
        assert_eq!(spy.call_count(), 0);
        owner.stop().await;
    }

    /// A subscription stores an argv the owner will execute, so only the
    /// owner's own Unix socket may register or remove one. Reading is allowed
    /// anywhere, but a channel that may not register a command is not told one
    /// back either.
    ///
    /// The fixture labels its router as the exposed transport, which is exactly
    /// what production attaches per listener — so the refusal under test is the
    /// endpoint's own rule, and the outcome under test is the client's mapping
    /// of it.
    #[tokio::test]
    async fn client_subscribe_mutation_is_refused_off_the_owner_socket() {
        let spy = SpyExecutor::new();
        let owner = Owner::start_with(
            Some(spy.clone()),
            None,
            OwnerExtras {
                sinks: true,
                transport: Some(conflux::web::remote_control_api::ApiTransport::Tcp),
            },
        )
        .await;
        owner.publish(snapshot(
            "select",
            vec![change("alpha", "select", "not queued")],
        ));
        owner.contract(merged_contract("main"));
        let instance = owner.instance_id();
        let tmp = neutral_cwd();
        let socket = owner.socket();

        for action in ["set", "clear"] {
            let mut args = vec![
                "subscribe",
                action,
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ];
            if action == "set" {
                args.extend_from_slice(&["--", "/bin/true"]);
            }
            let output = run_client(&socket, tmp.path(), &args).await;
            let parsed = envelope(&output);
            assert_eq!(parsed["outcome"], "transport_not_permitted", "{action}");
            assert_eq!(output.status.code(), Some(25), "{action}");
        }

        // Reading still works, and still answers presence — it just does not
        // disclose an argv to a channel that may not register one.
        let read = run_client(
            &socket,
            tmp.path(),
            &[
                "subscribe",
                "get",
                "alpha",
                "--instance-id",
                &instance,
                "--json",
            ],
        )
        .await;
        let parsed = envelope(&read);
        assert_eq!(parsed["outcome"], "subscribed", "{}", stderr_of(&read));
        assert_eq!(parsed["detail"]["subscriptions"][0]["subscribed"], false);
        assert!(parsed["detail"]["subscriptions"][0]["sink"].is_null());

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
                        "mark",
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
        assert_eq!(parsed["outcome"], "marked", "stderr={}", stderr_of(&output));
        assert_eq!(output.status.code(), Some(0));

        // A mark claims no admission, so the scheduler must not have moved.
        assert!(
            marks.is_marked("alpha"),
            "the shared execution-mark store must hold the operator's selection"
        );
        assert!(
            scheduler.launches().is_empty(),
            "marking is selection: nothing may be launched by it"
        );

        // The explicit Start is what consumes that authoritative mark set.
        let socket = socket_path.display().to_string();
        let started = tokio::task::spawn_blocking({
            let cwd = workspace.path().to_path_buf();
            move || {
                run_cli(
                    &cwd,
                    &["client", "--unix-socket", &socket, "start", "--json"],
                    &[],
                )
            }
        })
        .await
        .unwrap();
        let parsed = envelope(&started);
        assert_eq!(
            parsed["outcome"],
            "accepted",
            "stderr={}",
            stderr_of(&started)
        );

        // The proof this test exists for: the shared coordinator really ran the
        // same transaction a keypress reaches.
        assert!(
            marks.is_marked("alpha"),
            "the shared execution-mark store must hold the started change"
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
            "cflx client mark",
            "cflx client start",
            "cflx client wait",
            "cflx client subscribe set",
        ] {
            assert!(agents.contains(example), "AGENTS.md must show `{example}`");
        }
        // The retired admission verb must not survive as guidance anywhere.
        for retired in ["cflx client enqueue", "cflx client notify", "cflx_enqueue"] {
            assert!(
                !agents.contains(retired),
                "AGENTS.md must not recommend the retired `{retired}`"
            );
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

    /// The three ideas a reader must not conflate — selection, lifecycle
    /// control, and owner-side admission — stated in the documents an operator
    /// and an agent actually read.
    #[test]
    fn documentation_separates_marking_from_starting_from_admission() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let agents = std::fs::read_to_string(repo_root.join("AGENTS.md")).expect("AGENTS.md");
        let readme = std::fs::read_to_string(repo_root.join("README.md")).expect("README.md");

        for (document, name) in [(&agents, "AGENTS.md"), (&readme, "README.md")] {
            assert!(
                document.contains("preserve") || document.contains("preserves"),
                "{name} must say a mark preserves unrelated marks"
            );
            assert!(
                document.contains("unrelated mark"),
                "{name} must name what a mark leaves alone"
            );
            assert!(
                document.contains("F5"),
                "{name} must describe Start through its TUI equivalent"
            );
            assert!(
                document.contains("authoritative mark set"),
                "{name} must say Start consumes the owner's own marks"
            );
            assert!(
                document.contains("settlement"),
                "{name} must attribute admission to owner-side settlement"
            );
            // And it must not imply queue intent is a client-side result.
            assert!(
                !document.contains("cflx client enqueue"),
                "{name} must not recommend the retired admission verb"
            );
        }
    }

    /// The compact namespace, the client-only MCP server, and the subscription
    /// semantics an operator gets wrong if nobody writes them down.
    #[test]
    fn documentation_states_the_mcp_and_subscription_contract() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let agents = std::fs::read_to_string(repo_root.join("AGENTS.md")).expect("AGENTS.md");
        let readme = std::fs::read_to_string(repo_root.join("README.md")).expect("README.md");

        for document in [&agents, &readme] {
            assert!(
                document.contains("cflx client mcp"),
                "every document must show the MCP server"
            );
            for tool in ["cflx_status", "cflx_control", "cflx_subscribe"] {
                assert!(document.contains(tool), "{tool} must be documented");
            }
            for retired in ["cflx_enqueue", "cflx_notify_set"] {
                assert!(
                    !document.contains(retired),
                    "the retired tool {retired} must not be documented"
                );
            }
            // `cflx_wait` may be *named*, but only to say it is not there: an
            // MCP host that went looking for it needs to be told where the
            // bounded oracle lives instead.
            assert!(
                document.contains("cflx_wait is deliberately absent")
                    || document.contains("cflx_wait` is deliberately absent")
                    || document.contains("deliberately absent from MCP"),
                "the documents must explain that MCP has no wait tool"
            );
            // UDS-only subscription mutation.
            assert!(
                document.contains("transport_not_permitted"),
                "the transport refusal is the rule an operator hits first"
            );
            // The environment contract, verbatim.
            for variable in [
                "CFLX_EVENT_PATH",
                "CFLX_EVENT_TYPE",
                "CFLX_EXECUTION_ID",
                "CFLX_CHANGE_ID",
                "CFLX_INSTANCE_ID",
            ] {
                assert!(document.contains(variable), "{variable} must be documented");
            }
            // Execution completion, not process completion.
            assert!(
                document.contains("process exit was never"),
                "a resident TUI's liveness must be distinguished from completion"
            );
            // Delivery notifies; it never resumes.
            assert!(
                document.contains("resumes no agent")
                    || document.contains("does not resume")
                    || document.contains("resumes nothing")
                    || document.contains("it does not resume"),
                "every document must say delivery does not resume an agent"
            );
            // A token is a variable name, never a value.
            assert!(
                document.contains("--auth-token-env"),
                "the credential rule must be stated"
            );
            // A subscription may precede admission — the reason it is keyed by
            // the proposal at all.
            assert!(
                document.contains("before"),
                "the pre-admission registration must be documented"
            );
            // Dedupe is per episode, so a replacement cannot replay.
            assert!(
                document.contains("episode"),
                "the delivery episode is the unit a reader has to understand"
            );
            // The event artifact is read-only by default and never read back.
            assert!(
                document.contains("`0400`") && document.contains("`0700`"),
                "the event artifact's default permissions must be documented"
            );
            assert!(
                document.contains("never reads it back") || document.contains("never read back"),
                "and why those permissions are not an integrity guarantee"
            );
            // Output is bounded during collection, not merely cut afterwards.
            assert!(
                document.contains("full pipe"),
                "an operator writing a chatty callback needs to know it will not wedge"
            );
            assert!(
                document.contains("truncation diagnostic"),
                "and that overflow is a diagnostic rather than a delivery failure"
            );
            // Shutdown ordering: one deadline, serialized, reap before cleanup.
            assert!(
                document.contains("reaped"),
                "shutdown must promise termination and reaping, not just a wait"
            );
        }

        // The direct shell-facing path, in both documents and in the embedded
        // skill: an agent that has a shell must not be told MCP is the only
        // way to register a callback.
        let skill = std::fs::read_to_string(repo_root.join("skills/cflx-run/SKILL.md"))
            .expect("the embedded cflx-run skill");
        for (document, name) in [
            (&agents, "AGENTS.md"),
            (&readme, "README.md"),
            (&skill, "the cflx-run skill"),
        ] {
            assert!(
                document.contains("cflx client subscribe set"),
                "{name} must show the direct callback command"
            );
            assert!(
                document.contains("cflx client subscribe clear"),
                "{name} must show how a registration is withdrawn"
            );
            assert!(
                document.contains("cflx_subscribe"),
                "{name} must keep MCP as the alternative for a host with no shell"
            );
            // Nothing may claim a registration happens by itself.
            for retired in ["auto-resume", "cflx_enqueue", "cflx client enqueue"] {
                assert!(
                    !document.contains(retired),
                    "{name} must not carry retired guidance: {retired}"
                );
            }
        }
        assert!(
            skill.contains("no `sh -c`"),
            "the skill must say the callback is argv rather than shell source"
        );
        assert!(
            skill.contains("not process completion"),
            "and that a resident TUI's exit is not the signal"
        );
        assert!(
            skill.contains("Nothing subscribes you to completion automatically"),
            "the skill must state that registration is explicit"
        );

        // Why a one-shot event file is separate from the lifecycle adapter's
        // long-lived stdin stream.
        assert!(
            agents.contains("lifecycle adapter") && agents.contains("one-shot"),
            "AGENTS.md must explain why subscriptions are not the lifecycle adapter"
        );

        // Crash limitations and the external fallback.
        assert!(
            agents.contains("owner_restarted"),
            "AGENTS.md must name the typed answer for a lost owner"
        );
    }
}
