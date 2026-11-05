//! The bounded remote text surface on the `/api/v2` wire.
//!
//! Integration: the production router, the real application transaction
//! ([`crate::web::remote_control_api::executor::wired_for_test`]), the real
//! worktree service over a disposable Git repository whose linked worktrees
//! carry a `.git` *file*, and the real managed-writer ledger behind the
//! managed-termination port. Every real-Git case runs in the explicit heavy
//! tier; hang guards are generous timeouts, never elapsed-time assertions.

use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::ai_command_runner::{ManagedWriterLedger, RunCommandScope, WriterCoverage};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::{
    ExecutionMarkStore, ImmediateKillEvidence, LiveRunManagedProcesses, ManagedProcessTermination,
    OperatorCommandService, ParallelRuntime,
};
use crate::orchestration::operator_coordinator::CoreMode;
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::worktrees::{
    RemoteWorktreeOperations, WorktreeOperations, WorktreeRegistry,
};
use crate::web::remote_control_api::{router, RemoteControlState};
use crate::web::state::WebState;
use crate::worktree_ops::git_backend::GitWorktreeBackend;
use crate::worktree_ops::service::{WorktreeEventSink, WorktreeOperationEvent, WorktreeService};

use super::{get, post_json, send, status_and_json};

const ALPHA: &str = "alpha";
const BETA: &str = "beta";

// ============================================================================
// Fixture
// ============================================================================

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn sha(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn put(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, bytes).unwrap();
}

/// SHA-256 of every file under `dir`, keyed by relative path, skipping Git's
/// own metadata (which `git status` itself refreshes).
fn digest_tree(dir: &Path) -> BTreeMap<String, String> {
    fn visit(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if meta.is_dir() {
                if name == ".git" {
                    continue;
                }
                visit(base, &path, out);
            } else if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("symlink:{}", std::fs::read_link(&path).unwrap().display()),
                );
            } else if name != ".git" {
                out.insert(
                    rel,
                    format!(
                        "{}:{:o}",
                        sha(&std::fs::read(&path).unwrap()),
                        meta.permissions().mode() & 0o7777
                    ),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(dir, dir, &mut out);
    out
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

/// Percent-encode a query value.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn change(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<WorktreeOperationEvent>>);

#[async_trait]
impl WorktreeEventSink for RecordingSink {
    async fn emit(&self, event: WorktreeOperationEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// A coverage port answering per canonical root; everything else is `default`.
struct FakeCoverage {
    answers: Vec<(PathBuf, WriterCoverage)>,
    default: WriterCoverage,
}

#[async_trait]
impl ManagedProcessTermination for FakeCoverage {
    async fn owns_managed_process(&self, _change_id: &str) -> bool {
        false
    }
    async fn kill_managed_process(&self, _change_id: &str) -> ImmediateKillEvidence {
        ImmediateKillEvidence::nothing_to_kill()
    }
    async fn writer_coverage_under(&self, root: &Path) -> WriterCoverage {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        self.answers
            .iter()
            .find(|(path, _)| *path == root)
            .map(|(_, coverage)| coverage.clone())
            .unwrap_or_else(|| self.default.clone())
    }
}

struct Fx {
    _dir: TempDir,
    outside: PathBuf,
    root: PathBuf,
    alpha: PathBuf,
    beta: PathBuf,
    router: axum::Router,
    projection: Arc<Projection>,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    web_state: Arc<WebState>,
    service: Arc<WorktreeService>,
    port: Arc<RemoteWorktreeOperations>,
    ledger: ManagedWriterLedger,
    sink: Arc<RecordingSink>,
    keys: Mutex<usize>,
}

impl Fx {
    async fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let root = base.join("repo");
        let workspaces = base.join("workspaces");
        let outside = base.join("outside");
        for path in [&root, &workspaces, &outside] {
            std::fs::create_dir_all(path).unwrap();
        }

        git(&root, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test User"),
            ("commit.gpgsign", "false"),
        ] {
            git(&root, &["config", key, value]);
        }
        put(&root, "openspec/changes/alpha/proposal.md", b"# alpha\n");
        put(&root, "openspec/changes/alpha/tasks.md", b"- [ ] one\n");
        put(&root, "openspec/changes/beta/tasks.md", b"- [ ] two\n");
        put(&root, "src/lib.rs", b"pub fn base() {}\n");
        put(&root, "README.md", b"readme\n");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "base"]);

        let alpha = workspaces.join(ALPHA);
        let beta = workspaces.join(BETA);
        for (branch, path) in [(ALPHA, &alpha), (BETA, &beta)] {
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    branch,
                    path.to_str().unwrap(),
                ],
            );
        }

        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec![ALPHA.to_string(), BETA.to_string()],
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                Arc::new(crate::tui::queue::DynamicQueue::new()),
                Arc::new(crate::orchestration::operator_command::NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service,
            Arc::new(RecordingScheduler::new()),
            Arc::new(ResolveReservations::new()),
            parallel,
        ));

        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks).await;
        web_state
            .seed_workspace_observation_for_tests(&[change(ALPHA), change(BETA)], "select")
            .await;
        web_state
            .seed_worktree_paths_for_tests(
                root.clone(),
                HashMap::from([
                    (ALPHA.to_string(), alpha.clone()),
                    (BETA.to_string(), beta.clone()),
                ]),
            )
            .await;
        let ledger = ManagedWriterLedger::with_owner_cwd(Ok(outside.clone()));
        web_state
            .set_managed_termination(Arc::new(
                LiveRunManagedProcesses::new(|| None).with_writer_ledger(ledger.clone()),
            ))
            .await;

        let sink = Arc::new(RecordingSink::default());
        let worktree_service = Arc::new(WorktreeService::new(
            Arc::new(GitWorktreeBackend::new(
                root.clone(),
                Arc::new(OrchestratorConfig::default()),
            )),
            sink.clone(),
            workspaces,
        ));
        let port = Arc::new(RemoteWorktreeOperations::new(
            worktree_service.clone(),
            Arc::new(WorktreeRegistry::new()),
            root.clone(),
        ));

        let core_mode = Arc::new(CoreMode::new());
        let (executor, application) = crate::web::remote_control_api::executor::wired_for_test(
            reducer.clone(),
            run_control,
            web_state.clone(),
            core_mode,
        );
        let runtime = web_state.remote_control();
        runtime
            .bind(Arc::new(executor.with_worktrees(port.clone())))
            .await;
        runtime.bind_gate(application.gate()).await;
        runtime.bind_worktrees(port.clone()).await;
        let projection = runtime.projection();
        let auth = RemoteControlAuth::new(None, &[]).expect("no-token auth is valid");
        let router = router(
            RemoteControlState::new(projection.clone(), Arc::new(auth), runtime.clone())
                .with_gate(runtime.gate())
                .with_worktrees(runtime.clone())
                .with_text_edit(web_state.clone()),
        );

        Self {
            _dir: dir,
            outside,
            root,
            alpha,
            beta,
            router,
            projection,
            reducer,
            web_state,
            service: worktree_service,
            port,
            ledger,
            sink,
            keys: Mutex::new(0),
        }
    }

    async fn set_port(&self, port: Arc<dyn ManagedProcessTermination>) {
        self.web_state.set_managed_termination(port).await;
    }

    /// The live opaque ID of a branch's worktree; `"main"` names the main one.
    async fn id(&self, branch: &str) -> String {
        self.port
            .list()
            .await
            .expect("listed")
            .worktrees
            .into_iter()
            .find(|w| {
                if branch == "main" {
                    w.is_main
                } else {
                    w.branch == branch
                }
            })
            .unwrap_or_else(|| panic!("worktree {branch} is listed"))
            .worktree_id
    }

    async fn read_wt(&self, id: &str, path: &str) -> (StatusCode, Value) {
        status_and_json(
            send(
                &self.router,
                get(
                    &format!("/api/v2/worktrees/{id}/text?path={}", encode(path)),
                    None,
                ),
            )
            .await,
        )
        .await
    }

    async fn read_change(&self, change_id: &str, path: &str) -> (StatusCode, Value) {
        status_and_json(
            send(
                &self.router,
                get(
                    &format!("/api/v2/changes/{change_id}/text?path={}", encode(path)),
                    None,
                ),
            )
            .await,
        )
        .await
    }

    fn key(&self) -> String {
        let mut keys = self.keys.lock().unwrap();
        *keys += 1;
        format!("text-{}", *keys)
    }

    async fn submit(&self, mut body: Value) -> (StatusCode, Value) {
        body["expected_revision"] = json!(self.projection.revision());
        if body.get("idempotency_key").is_none() {
            body["idempotency_key"] = json!(self.key());
        }
        status_and_json(
            send(
                &self.router,
                post_json("/api/v2/commands", None, &body.to_string()),
            )
            .await,
        )
        .await
    }

    async fn write_wt(
        &self,
        id: &str,
        path: &str,
        expected: Value,
        content: &str,
    ) -> (StatusCode, Value) {
        self.submit(json!({
            "type": "write_worktree_text",
            "target": {"worktree_id": id},
            "params": {"path": path, "expected": expected, "content": content},
        }))
        .await
    }

    async fn write_change(
        &self,
        change_id: &str,
        path: &str,
        expected: Value,
        content: &str,
    ) -> (StatusCode, Value) {
        self.submit(json!({
            "type": "write_change_text",
            "target": {"change_id": change_id},
            "params": {"path": path, "expected": expected, "content": content},
        }))
        .await
    }

    async fn refresh(&self) {
        self.web_state.sync_remote_control_projection().await;
    }
}

fn expect_content(id: &str) -> Value {
    json!({"kind": "content", "content_id": id})
}

fn absent() -> Value {
    json!({"kind": "absent"})
}

/// The read's `editable` and blocked reason.
fn blocked(read: &Value) -> Option<String> {
    assert_eq!(read["editable"], read["edit_blocked_reason"].is_null());
    read["edit_blocked_reason"].as_str().map(str::to_string)
}

// ============================================================================
// Identity, write, no-op, stale, absent
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_read_reports_identity_mode_and_limits() {
    let fx = Fx::new().await;
    put(&fx.alpha, "notes.txt", b"hello\n");
    std::fs::set_permissions(
        fx.alpha.join("notes.txt"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let id = fx.id(ALPHA).await;
    let revision = fx.projection.revision();
    let registry = fx.projection.registry_sizes();

    let (status, read) = fx.read_wt(&id, "notes.txt").await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["content"], "hello\n");
    assert_eq!(read["content_id"], sha(b"hello\n"));
    assert_eq!(
        read["content_id"],
        "sha256:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
    );
    assert_eq!(read["size_bytes"], 6);
    assert_eq!(read["mode"], "0644");
    assert_eq!(read["exists"], true);
    assert_eq!(read["editable"], true, "{read}");
    assert_eq!(
        read["scope"],
        json!({"kind": "worktree", "worktree_id": id})
    );
    assert_eq!(
        read["limits"],
        json!({"max_bytes": 1048576, "max_path_bytes": 1024, "content_id_algorithm": "sha256"})
    );
    assert_eq!(
        fx.projection.revision(),
        revision,
        "a read advances no revision"
    );
    assert_eq!(
        fx.projection.registry_sizes(),
        registry,
        "a read creates no record"
    );

    let (_, absent) = fx.read_wt(&id, "new.txt").await;
    assert_eq!(absent["exists"], false);
    assert!(absent["content"].is_null() && absent["content_id"].is_null());

    let (status, caps) =
        status_and_json(send(&fx.router, get("/api/v2/capabilities", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(caps["text_edit"]["available"], true);
    assert!(caps["worktrees"]["operations"]
        .as_array()
        .unwrap()
        .contains(&json!("text")));

    // Unknown targets are typed refusals.
    let (status, error) = fx.read_change("nope", "tasks.md").await;
    assert_eq!(
        (status, error["error_code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let (status, error) = fx.read_wt(&"0".repeat(32), "a").await;
    assert_eq!(
        (status, error["error_code"].as_str()),
        (StatusCode::NOT_FOUND, Some("worktree_not_found"))
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_write_replaces_bytes_atomically_and_dirties_git() {
    let fx = Fx::new().await;
    std::fs::set_permissions(
        fx.alpha.join("src/lib.rs"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let head = git(&fx.alpha, &["rev-parse", "HEAD"]);
    let id = fx.id(ALPHA).await;
    let (_, read) = fx.read_wt(&id, "src/lib.rs").await;
    assert_eq!(read["mode"], "0600");
    let events_before = fx.sink.0.lock().unwrap().len();

    let (status, record) = fx
        .write_wt(
            &id,
            "src/lib.rs",
            expect_content(read["content_id"].as_str().unwrap()),
            "pub fn edited() {}\n",
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "succeeded");
    let result = &record["result"];
    assert_eq!(result["kind"], "text_write");
    assert_eq!(result["outcome"], "written");
    assert_eq!(result["content_id_before"], read["content_id"]);
    assert_eq!(result["content_id_after"], sha(b"pub fn edited() {}\n"));
    assert_eq!(result["bytes_written"], 19);
    assert_eq!(result["mode"], "0600");
    assert_eq!(result["effects"]["file_written"], true);

    assert_eq!(
        std::fs::read(fx.alpha.join("src/lib.rs")).unwrap(),
        b"pub fn edited() {}\n"
    );
    assert_eq!(
        std::fs::metadata(fx.alpha.join("src/lib.rs"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(git(&fx.alpha, &["status", "--porcelain"]), "M src/lib.rs");
    assert_eq!(
        git(&fx.alpha, &["rev-parse", "HEAD"]),
        head,
        "no commit is created"
    );
    assert_eq!(listing(&fx.alpha.join("src")), vec!["lib.rs".to_string()]);
    assert!(
        fx.sink.0.lock().unwrap()[events_before..]
            .iter()
            .any(|event| matches!(event, WorktreeOperationEvent::Refreshed)),
        "the worktree observation refresh follows the write"
    );
    let (_, worktree) =
        status_and_json(send(&fx.router, get(&format!("/api/v2/worktrees/{id}"), None)).await)
            .await;
    assert_eq!(worktree["worktree"]["dirty"], true);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_identical_content_is_a_no_op() {
    let fx = Fx::new().await;
    let id = fx.id(ALPHA).await;
    let before = std::fs::metadata(fx.alpha.join("README.md")).unwrap();
    let (_, read) = fx.read_wt(&id, "README.md").await;

    let (status, record) = fx
        .write_wt(
            &id,
            "README.md",
            expect_content(read["content_id"].as_str().unwrap()),
            "readme\n",
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "no_op");
    assert_eq!(record["result"]["outcome"], "unchanged");
    assert_eq!(record["result"]["effects"]["file_written"], false);
    let after = std::fs::metadata(fx.alpha.join("README.md")).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec())
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_stale_identity_writes_nothing() {
    let fx = Fx::new().await;
    let id = fx.id(ALPHA).await;
    let (_, read) = fx.read_wt(&id, "README.md").await;
    std::fs::write(fx.alpha.join("README.md"), b"changed elsewhere\n").unwrap();
    let names = listing(&fx.alpha);

    let (status, record) = fx
        .write_wt(
            &id,
            "README.md",
            expect_content(read["content_id"].as_str().unwrap()),
            "mine\n",
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["error_code"], "text_stale");
    assert_eq!(
        record["result"]["current_content_id"],
        sha(b"changed elsewhere\n")
    );
    assert_eq!(record["result"]["effects"]["file_written"], false);
    assert_eq!(
        std::fs::read(fx.alpha.join("README.md")).unwrap(),
        b"changed elsewhere\n"
    );
    assert_eq!(listing(&fx.alpha), names);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_absent_expectation_creates_once() {
    let fx = Fx::new().await;
    let id = fx.id(ALPHA).await;

    let (status, record) = fx.write_wt(&id, "docs/new.md", absent(), "x\n").await;
    // A missing intermediate directory is never created.
    assert_eq!(
        (status, record["error_code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    assert!(!fx.alpha.join("docs").exists());

    let (status, record) = fx.write_wt(&id, "new.md", absent(), "fresh\n").await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert!(record["result"]["content_id_before"].is_null());
    let mode = std::fs::metadata(fx.alpha.join("new.md"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode & !0o644,
        0,
        "created as 0644 subject to umask: {mode:o}"
    );

    let (status, record) = fx.write_wt(&id, "new.md", absent(), "again\n").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(record["error_code"], "text_stale");
    assert_eq!(record["result"]["current_content_id"], sha(b"fresh\n"));

    let (status, record) = fx
        .write_wt(&id, "missing.md", expect_content(&sha(b"x")), "y\n")
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(record["error_code"], "text_stale");
    assert!(record["result"]["current_content_id"].is_null());
    assert!(!fx.alpha.join("missing.md").exists());
    assert_eq!(std::fs::read(fx.alpha.join("new.md")).unwrap(), b"fresh\n");
}

// ============================================================================
// Containment
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_refuses_every_containment_violation() {
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    let main = fx.id("main").await;
    put(&fx.alpha, ".wt/teardown", b"echo bye\n");
    put(&fx.root, "openspec/changes/alpha/.wt/setup", b"echo hi\n");
    put(&fx.root, ".conflux-inflight", b"alpha\n");
    put(&fx.root, "sub/.conflux-inflight", b"plain\n");
    put(&fx.alpha, "run.sh", b"echo\n");
    std::fs::set_permissions(
        fx.alpha.join("run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    put(&fx.alpha, "big.txt", &vec![b'a'; 1_048_577]);
    put(&fx.alpha, "nul.txt", b"a\0b\n");
    put(&fx.alpha, "latin1.txt", &[0x63, 0x61, 0x66, 0xe9]);
    put(&fx.alpha, "target.txt", b"target\n");
    put(&fx.alpha, "swap.txt", b"swap\n");
    std::os::unix::fs::symlink(fx.alpha.join("src"), fx.alpha.join("linkdir")).unwrap();
    std::os::unix::fs::symlink(fx.alpha.join("target.txt"), fx.alpha.join("link.txt")).unwrap();
    let (_, swap_read) = fx.read_wt(&alpha, "swap.txt").await;
    assert_eq!(swap_read["exists"], true);

    // A symlink substituted after a successful read.
    std::fs::remove_file(fx.alpha.join("swap.txt")).unwrap();
    std::os::unix::fs::symlink(fx.alpha.join("target.txt"), fx.alpha.join("swap.txt")).unwrap();

    let alpha_before = digest_tree(&fx.alpha);
    let root_before = digest_tree(&fx.root);
    let any = sha(b"anything");

    // Lexical violations: 422, no record.
    let registry = fx.projection.registry_sizes();
    for path in [
        "../README.md",
        "/etc/passwd",
        "a\\b",
        "a//b",
        "src/",
        "./README.md",
    ] {
        let (status, error) = fx.read_wt(&alpha, path).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "read {path}: {error}"
        );
        let (status, error) = fx.write_wt(&alpha, path, expect_content(&any), "x").await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "write {path}: {error}"
        );
        assert_eq!(error["error_code"], "validation_failed");
    }
    assert_eq!(fx.projection.registry_sizes(), registry);

    enum Scope<'a> {
        Wt(&'a str),
        Change(&'a str),
    }
    let cases: Vec<(Scope, &str, &str)> = vec![
        (Scope::Wt(&alpha), ".git", "git_metadata"),
        (Scope::Wt(&main), ".git/config", "git_metadata"),
        (Scope::Wt(&alpha), ".wt/teardown", "conflux_owned_path"),
        (Scope::Change(ALPHA), ".wt/setup", "conflux_owned_path"),
        (Scope::Wt(&main), ".conflux-inflight", "conflux_owned_path"),
        (Scope::Wt(&alpha), "linkdir/lib.rs", "unsafe_path"),
        (Scope::Wt(&alpha), "link.txt", "unsafe_path"),
        (Scope::Wt(&alpha), "swap.txt", "unsafe_path"),
        (Scope::Wt(&alpha), "src", "not_regular_file"),
        (Scope::Wt(&alpha), "run.sh", "executable"),
        (Scope::Wt(&alpha), "big.txt", "too_large"),
        (Scope::Wt(&alpha), "nul.txt", "not_text"),
        (Scope::Wt(&alpha), "latin1.txt", "not_text"),
    ];
    for (scope, path, reason) in &cases {
        let (read_status, read) = match scope {
            Scope::Wt(id) => fx.read_wt(id, path).await,
            Scope::Change(change) => fx.read_change(change, path).await,
        };
        assert_eq!(read_status, StatusCode::CONFLICT, "read {path}: {read}");
        assert_eq!(read["error_code"], "text_not_editable", "{path}");
        assert!(
            read["message"].as_str().unwrap().contains(reason),
            "{path}: {read}"
        );

        let expected = if *path == "swap.txt" {
            expect_content(swap_read["content_id"].as_str().unwrap())
        } else {
            expect_content(&any)
        };
        let (status, record) = match scope {
            Scope::Wt(id) => fx.write_wt(id, path, expected, "overwritten\n").await,
            Scope::Change(change) => {
                fx.write_change(change, path, expected, "overwritten\n")
                    .await
            }
        };
        assert_eq!(status, StatusCode::CONFLICT, "write {path}: {record}");
        assert_eq!(record["error_code"], "text_not_editable");
        assert_eq!(record["result"]["reason"], *reason, "{path}");
        assert_eq!(record["result"]["effects"]["file_written"], false);
    }

    // A change directory that disappeared is `root_missing`, on the record too.
    std::fs::remove_dir_all(fx.root.join("openspec/changes/beta")).unwrap();
    let (status, record) = fx
        .write_change(BETA, "tasks.md", expect_content(&any), "x\n")
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["error_code"], "text_not_editable");
    assert_eq!(record["result"]["reason"], "root_missing");
    let (status, read) = fx.read_change(BETA, "tasks.md").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(read["message"].as_str().unwrap().contains("root_missing"));
    let root_before = {
        let mut before = root_before;
        before.retain(|path, _| !path.starts_with("openspec/changes/beta/"));
        before
    };

    // The same name below the top level is ordinary text.
    let (status, nested) = fx.read_wt(&main, "sub/.conflux-inflight").await;
    assert_eq!(status, StatusCode::OK, "{nested}");

    assert_eq!(
        digest_tree(&fx.alpha),
        alpha_before,
        "no byte changed under alpha"
    );
    assert_eq!(
        digest_tree(&fx.root),
        root_before,
        "no byte changed under the root"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_refuses_nested_worktree_through_main_root() {
    let fx = Fx::new().await;
    let nested = fx.root.join(".worktrees").join("gamma");
    git(
        &fx.root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "gamma",
            nested.to_str().unwrap(),
        ],
    );
    std::fs::create_dir_all(fx.root.join("embedded")).unwrap();
    std::os::unix::fs::symlink("/nonexistent", fx.root.join("embedded/.git")).unwrap();
    put(&fx.root, "embedded/file.txt", b"embedded\n");
    let main = fx.id("main").await;
    let gamma = fx.id("gamma").await;
    let root_before = digest_tree(&fx.root);
    let gamma_before = digest_tree(&nested);

    for path in [".worktrees/gamma/src/lib.rs", "embedded/file.txt"] {
        let (status, read) = fx.read_wt(&main, path).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {read}");
        assert!(read["message"]
            .as_str()
            .unwrap()
            .contains("nested_repository"));
        let (status, record) = fx
            .write_wt(
                &main,
                path,
                expect_content(&sha(b"pub fn base() {}\n")),
                "hijack\n",
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{record}");
        assert_eq!(record["result"]["reason"], "nested_repository");
    }

    // The same file through the nested worktree's own ID is served.
    let (status, read) = fx.read_wt(&gamma, "src/lib.rs").await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["content"], "pub fn base() {}\n");

    assert_eq!(digest_tree(&fx.root), root_before);
    assert_eq!(digest_tree(&nested), gamma_before);
}

// ============================================================================
// Writer exclusion
// ============================================================================

/// The read and the command agree on the reason; nothing is written.
async fn assert_refused(fx: &Fx, wt: Option<&str>, change: Option<&str>, reason: &str) {
    if let Some(id) = wt {
        let (status, read) = fx.read_wt(id, "README.md").await;
        assert_eq!(status, StatusCode::OK, "{read}");
        assert_eq!(blocked(&read).as_deref(), Some(reason), "{read}");
        assert!(read["edit_blocked_detail"].is_string());
        let before = std::fs::read(fx.alpha.join("README.md")).ok();
        let (status, record) = fx
            .write_wt(
                id,
                "README.md",
                expect_content(read["content_id"].as_str().unwrap()),
                "blocked\n",
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{record}");
        assert_eq!(record["error_code"], "target_ineligible");
        assert_eq!(record["result"]["reason"], reason);
        assert_eq!(record["result"]["detail"], read["edit_blocked_detail"]);
        assert_eq!(std::fs::read(fx.alpha.join("README.md")).ok(), before);
    }
    if let Some(change) = change {
        let (status, read) = fx.read_change(change, "tasks.md").await;
        assert_eq!(status, StatusCode::OK, "{read}");
        assert_eq!(blocked(&read).as_deref(), Some(reason), "{read}");
        let (status, record) = fx
            .write_change(
                change,
                "tasks.md",
                expect_content(read["content_id"].as_str().unwrap()),
                "blocked\n",
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{record}");
        assert_eq!(record["result"]["reason"], reason);
    }
}

async fn assert_editable(fx: &Fx, wt: Option<&str>, change: Option<&str>) {
    if let Some(id) = wt {
        let (status, read) = fx.read_wt(id, "README.md").await;
        assert_eq!(status, StatusCode::OK, "{read}");
        assert_eq!(blocked(&read), None, "{read}");
    }
    if let Some(change) = change {
        let (status, read) = fx.read_change(change, "tasks.md").await;
        assert_eq!(status, StatusCode::OK, "{read}");
        assert_eq!(blocked(&read), None, "{read}");
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_writer_exclusion_refuses_active_targets() {
    // An active owning change.
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    fx.reducer
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: ALPHA.to_string(),
            command: "apply".to_string(),
        });
    fx.refresh().await;
    assert_refused(&fx, Some(&alpha), Some(ALPHA), "change_active").await;
    assert_editable(&fx, Some(&fx.id(BETA).await), Some(BETA)).await;
    // The main worktree is no way around the change's own refusal.
    let main = fx.id("main").await;
    let (_, read) = fx.read_wt(&main, "openspec/changes/alpha/tasks.md").await;
    assert_eq!(blocked(&read).as_deref(), Some("change_active"), "{read}");
    let (status, record) = fx
        .write_wt(
            &main,
            "openspec/changes/alpha/tasks.md",
            expect_content(read["content_id"].as_str().unwrap()),
            "bypass\n",
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["result"]["reason"], "change_active");
    assert_eq!(
        std::fs::read(fx.root.join("openspec/changes/alpha/tasks.md")).unwrap(),
        b"- [ ] one\n"
    );
    let (_, read) = fx.read_wt(&main, "openspec/changes/beta/tasks.md").await;
    assert_eq!(blocked(&read), None, "{read}");
    let (_, read) = fx.read_wt(&main, "README.md").await;
    assert_eq!(blocked(&read), None, "{read}");

    // A writer with no change ID bound to alpha's directory, and a sibling the
    // same port reports `Clear` for.
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    fx.set_port(Arc::new(FakeCoverage {
        answers: vec![(
            fx.alpha.clone(),
            WriterCoverage::Busy {
                detail: "a lease with no change ID is bound here".to_string(),
            },
        )],
        default: WriterCoverage::Clear,
    }))
    .await;
    assert_refused(&fx, Some(&alpha), None, "managed_process_live").await;
    let (_, read) = fx.read_wt(&alpha, "README.md").await;
    assert_eq!(
        read["edit_blocked_detail"],
        "a lease with no change ID is bound here"
    );
    assert_editable(&fx, Some(&fx.id(BETA).await), Some(ALPHA)).await;

    // The repository root guard.
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    {
        let _guard = fx
            .service
            .acquire_root_for_test()
            .expect("the guard is free");
        assert_refused(&fx, Some(&alpha), Some(ALPHA), "root_busy").await;
    }
    assert_editable(&fx, Some(&alpha), Some(ALPHA)).await;

    // A running resolve, on the main worktree row that no change owns.
    let fx = Fx::new().await;
    let main = fx.id("main").await;
    fx.web_state
        .apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: BETA.to_string(),
            command: "resolve".to_string(),
        })
        .await;
    fx.refresh().await;
    let (_, read) = fx.read_wt(&main, "README.md").await;
    assert_eq!(blocked(&read).as_deref(), Some("root_busy"), "{read}");
    let (status, record) = fx
        .write_wt(
            &main,
            "README.md",
            expect_content(read["content_id"].as_str().unwrap()),
            "x\n",
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(record["result"]["reason"], "root_busy");
    assert_eq!(
        std::fs::read(fx.root.join("README.md")).unwrap(),
        b"readme\n"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_writer_exclusion_refuses_admitted_targets() {
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    fx.reducer
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue(ALPHA.to_string()));
    fx.refresh().await;
    let (_, state) = status_and_json(send(&fx.router, get("/api/v2/state", None)).await).await;
    let row = state["snapshot"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == ALPHA)
        .unwrap()
        .clone();
    assert_eq!(row["display_status"], "queued", "{row}");
    assert_refused(&fx, Some(&alpha), Some(ALPHA), "change_admitted").await;

    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    fx.reducer
        .write()
        .await
        .reconcile_dependency_blocker(ALPHA, &[BETA.to_string()]);
    fx.refresh().await;
    assert_refused(&fx, Some(&alpha), Some(ALPHA), "change_admitted").await;
    assert_editable(&fx, Some(&fx.id(BETA).await), Some(BETA)).await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_retained_scope_from_previous_run_blocks_edits() {
    let fx = Fx::new().await;
    let supervisor = Arc::new(
        crate::tui::run_supervisor::TuiRunSupervisor::idle_for_test(
            fx.root.clone(),
            fx.reducer.clone(),
        )
        .with_writer_ledger(fx.ledger.clone()),
    );
    let scope_owner = supervisor.clone();
    fx.set_port(Arc::new(
        LiveRunManagedProcesses::new(move || scope_owner.run_command_scope())
            .with_writer_ledger(fx.ledger.clone()),
    ))
    .await;
    let alpha = fx.id(ALPHA).await;
    let beta = fx.id(BETA).await;
    let force_stop_before = fx.projection.snapshot().0.changes[0].actions;

    // Run A retains an identity it never proved quiescent.
    let run_a = RunCommandScope::new();
    supervisor.install_run_for_test(None, None, Some(run_a.clone()));
    let retained =
        run_a.register_unproven_in_for_test("apply", Some(ALPHA), 4_000_001, Some(&fx.alpha));
    retained.retain_unconfirmed();
    // Run B replaces it with a fresh, empty scope.
    supervisor.install_run_for_test(None, None, Some(RunCommandScope::new()));
    fx.refresh().await;

    let (_, read) = fx.read_wt(&alpha, "README.md").await;
    assert_eq!(
        blocked(&read).as_deref(),
        Some("managed_process_live"),
        "{read}"
    );
    assert!(
        read["edit_blocked_detail"]
            .as_str()
            .unwrap()
            .contains("4000001"),
        "the detail names the retained identity: {read}"
    );
    assert_refused(&fx, Some(&alpha), None, "managed_process_live").await;
    assert_editable(&fx, Some(&beta), None).await;
    assert_eq!(
        fx.projection.snapshot().0.changes[0].actions,
        force_stop_before,
        "force_stop_change eligibility is unchanged"
    );

    retained.release_proven(4_000_001);
    assert_editable(&fx, Some(&alpha), None).await;
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_unbound_lifecycle_activity_refuses() {
    for arrangement in ["no-ledger", "unresolved-lease"] {
        let fx = Fx::new().await;
        match arrangement {
            "no-ledger" => {
                fx.set_port(Arc::new(LiveRunManagedProcesses::new(|| None)))
                    .await
            }
            _ => drop(fx.ledger.take_lease("shell", Some(&fx.alpha))),
        }
        let alpha = fx.id(ALPHA).await;
        let main = fx.id("main").await;
        assert_refused(&fx, Some(&alpha), Some(ALPHA), "managed_process_live").await;
        let (_, read) = fx.read_wt(&main, "README.md").await;
        assert_eq!(blocked(&read).as_deref(), Some("managed_process_live"));
        assert!(
            read["edit_blocked_detail"]
                .as_str()
                .unwrap()
                .contains("unknown"),
            "{arrangement}: {read}"
        );
        let snapshot = fx.projection.snapshot().0;
        assert!(
            snapshot
                .changes
                .iter()
                .all(|change| !change.actions.force_stop_change.allowed),
            "coverage does not feed force_stop_change"
        );
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_session_owned_writer_blocks_target() {
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    let main = fx.id("main").await;

    // The session shape: no change ID, no operation, bound to alpha's directory.
    let scope = RunCommandScope::new();
    fx.ledger.register_scope(&scope);
    let session = scope
        .register_in_for_test(None, None, Some(&fx.alpha))
        .expect("an open scope admits it");
    assert_refused(&fx, Some(&alpha), None, "managed_process_live").await;
    // The change directory lives under the project root, not under alpha.
    assert_editable(&fx, Some(&main), Some(ALPHA)).await;
    session.release_confirmed();
    assert_editable(&fx, Some(&alpha), None).await;

    // A cwd-less writer runs in the owner's directory: the project root.
    let at_root = ManagedWriterLedger::with_owner_cwd(Ok(fx.root.clone()));
    fx.set_port(Arc::new(
        LiveRunManagedProcesses::new(|| None).with_writer_ledger(at_root.clone()),
    ))
    .await;
    let lease = at_root.take_lease("shell", None);
    let (_, read) = fx.read_wt(&main, "README.md").await;
    assert_eq!(blocked(&read).as_deref(), Some("managed_process_live"));
    assert_refused(&fx, None, Some(ALPHA), "managed_process_live").await;
    assert_refused(&fx, None, Some(BETA), "managed_process_live").await;
    assert_editable(&fx, Some(&alpha), None).await;
    lease.release_unspawned();
    assert_editable(&fx, Some(&main), Some(ALPHA)).await;
    assert!(fx.outside.is_dir());
}

// ============================================================================
// Scope isolation, failure, closure
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_change_scope_edits_only_the_change_directory() {
    let fx = Fx::new().await;
    let beta_dir = fx.root.join("openspec/changes/beta");
    let beta_before = digest_tree(&beta_dir);
    let alpha_wt_before = digest_tree(&fx.alpha);

    let (_, read) = fx.read_change(ALPHA, "tasks.md").await;
    assert_eq!(read["content"], "- [ ] one\n");
    assert_eq!(read["scope"], json!({"kind": "change", "change_id": ALPHA}));
    let (status, record) = fx
        .write_change(
            ALPHA,
            "tasks.md",
            expect_content(read["content_id"].as_str().unwrap()),
            "- [x] one\n",
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(
        std::fs::read(fx.root.join("openspec/changes/alpha/tasks.md")).unwrap(),
        b"- [x] one\n"
    );
    assert_eq!(
        git(&fx.root, &["status", "--porcelain"]),
        "M openspec/changes/alpha/tasks.md"
    );

    let (status, _) = fx
        .write_change(
            ALPHA,
            "../beta/tasks.md",
            expect_content(&sha(b"- [ ] two\n")),
            "x\n",
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(digest_tree(&beta_dir), beta_before);
    assert_eq!(digest_tree(&fx.alpha), alpha_wt_before);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_cross_target_isolation() {
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    let root_before = digest_tree(&fx.root);
    let alpha_before = digest_tree(&fx.alpha);
    let beta_before = digest_tree(&fx.beta);

    let (_, read) = fx.read_wt(&alpha, "src/lib.rs").await;
    let (status, record) = fx
        .write_wt(
            &alpha,
            "src/lib.rs",
            expect_content(read["content_id"].as_str().unwrap()),
            "pub fn alpha() {}\n",
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");

    assert_eq!(digest_tree(&fx.root), root_before);
    assert_eq!(digest_tree(&fx.beta), beta_before);
    let alpha_after = digest_tree(&fx.alpha);
    let changed: Vec<&String> = alpha_after
        .keys()
        .chain(alpha_before.keys())
        .filter(|path| alpha_after.get(*path) != alpha_before.get(*path))
        .collect();
    assert_eq!(
        changed,
        vec!["src/lib.rs", "src/lib.rs"],
        "only the written path differs"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_failure_leaves_no_temporary_file() {
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: permission bits do not bind UID 0");
        return;
    }
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    let (_, read) = fx.read_wt(&alpha, "src/lib.rs").await;
    let src = fx.alpha.join("src");
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o555)).unwrap();

    let (status, record) = fx
        .write_wt(
            &alpha,
            "src/lib.rs",
            expect_content(read["content_id"].as_str().unwrap()),
            "x\n",
        )
        .await;
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{record}");
    assert_eq!(record["error_code"], "internal_error");
    assert_eq!(record["result"]["effects"]["file_written"], false);
    assert_eq!(listing(&src), vec!["lib.rs".to_string()]);
    assert_eq!(
        std::fs::read(src.join("lib.rs")).unwrap(),
        b"pub fn base() {}\n"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_text_edit_commands_are_closed_and_validated_on_the_wire() {
    let fx = Fx::new().await;
    let alpha = fx.id(ALPHA).await;
    let registry = fx.projection.registry_sizes();
    let oversize = "a".repeat(1_048_577);
    for params in [
        json!({"path": "README.md", "expected": absent(), "content": "", "force": true}),
        json!({"path": "README.md", "content": ""}),
        json!({"path": "README.md", "expected": {"kind": "content", "content_id": "sha256:12"}, "content": ""}),
        json!({"path": "README.md", "expected": absent(), "content": oversize}),
        json!({"path": "README.md", "expected": absent(), "content": "a\u{0}b"}),
    ] {
        let (status, error) = fx
            .submit(json!({"type": "write_worktree_text", "target": {"worktree_id": alpha}, "params": params}))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
        assert_eq!(error["error_code"], "validation_failed");
    }
    assert_eq!(
        fx.projection.registry_sizes(),
        registry,
        "no record was created"
    );

    // An exact replay of a settled write returns the original record and
    // writes nothing a second time.
    let (_, read) = fx.read_wt(&alpha, "README.md").await;
    let body = json!({
        "type": "write_worktree_text",
        "target": {"worktree_id": alpha},
        "params": {"path": "README.md", "expected": expect_content(read["content_id"].as_str().unwrap()), "content": "once\n"},
        "expected_revision": fx.projection.revision(),
        "idempotency_key": "replayed",
    });
    let first = status_and_json(
        send(
            &fx.router,
            post_json("/api/v2/commands", None, &body.to_string()),
        )
        .await,
    )
    .await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    let after_first = std::fs::metadata(fx.alpha.join("README.md")).unwrap();
    assert_eq!(
        std::fs::read(fx.alpha.join("README.md")).unwrap(),
        b"once\n"
    );
    let replay = status_and_json(
        send(
            &fx.router,
            post_json("/api/v2/commands", None, &body.to_string()),
        )
        .await,
    )
    .await;
    assert_eq!(replay.1["command_id"], first.1["command_id"]);
    assert_eq!(replay.1["result"], first.1["result"]);
    let after_replay = std::fs::metadata(fx.alpha.join("README.md")).unwrap();
    assert_eq!(
        (
            after_replay.ino(),
            after_replay.mtime(),
            after_replay.mtime_nsec()
        ),
        (
            after_first.ino(),
            after_first.mtime(),
            after_first.mtime_nsec()
        ),
        "the replay wrote nothing"
    );
}
