//! `merge_worktree_abort_on_conflict`: an explicit abort-on-conflict merge that
//! refuses predicted conflicts, aborts real ones with restoration verified from
//! the repository, and leaves `merge_worktree` exactly as it was.
//!
//! The real-Git cases drive the real router, the real shared service, and the
//! real Git backend against a disposable repository whose managed worktrees
//! live outside it. The restoration-evidence case uses a fake backend inside
//! the same router so every `restored` verdict is reachable deterministically.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::Value;
use tempfile::TempDir;

use super::{get, harness, json_body, post_json, send, status_and_json, Harness};
use crate::config::OrchestratorConfig;
use crate::vcs::git::commands::{AbortReport, Restoration};
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode, SUPPORTED_COMMANDS};
use crate::web::remote_control_api::worktrees::{
    RemoteWorktreeOperations, WorktreeOperations, WorktreeRegistry, MERGE_ABORT_COMMAND,
};
use crate::worktree_ops::git_backend::GitWorktreeBackend;
use crate::worktree_ops::service::{
    ConflictPolicy, MergeAttempt, SafetyFact, WorktreeBackend, WorktreeEventSink, WorktreeFacts,
    WorktreeOpResult, WorktreeOperationEvent, WorktreeService, RECOVERY_LOCAL_OR_TUI,
};

const TOKEN: &str = "merge-abort-token";

// ============================================================================
// Router helpers
// ============================================================================

/// Submit one worktree command at the current revision and return the record.
async fn submit(
    harness: &Harness,
    command: &str,
    worktree_id: &str,
    key: &str,
) -> (StatusCode, Value) {
    let state = json_body(send(&harness.router, get("/api/v2/state", Some(TOKEN))).await).await;
    let revision = state["state_revision"].as_u64().unwrap_or(0);
    let body = format!(
        r#"{{"type":"{command}","target":{{"worktree_id":"{worktree_id}"}},"params":{{}},"expected_revision":{revision},"idempotency_key":"{key}"}}"#
    );
    status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await,
    )
    .await
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<WorktreeOperationEvent>>);

#[async_trait]
impl WorktreeEventSink for RecordingSink {
    async fn emit(&self, event: WorktreeOperationEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl RecordingSink {
    fn events(&self) -> Vec<WorktreeOperationEvent> {
        self.0.lock().unwrap().clone()
    }
}

// ============================================================================
// Real-Git fixture
// ============================================================================

/// Run one fixture Git command synchronously, isolated from user config.
fn git_sync(cwd: &Path, args: &[&str]) -> String {
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

/// A base repository on `main`, managed worktrees outside it, and an
/// `on_merged` hook that counts its runs into a file outside the repository.
///
/// Worktrees: `conflicting` edits the line `main` later edits; `clean-b` and
/// `clean-c` each add their own file.
struct Fixture {
    dir: TempDir,
    root: PathBuf,
    counter: PathBuf,
    sink: Arc<RecordingSink>,
    harness: Harness,
    port: Arc<RemoteWorktreeOperations>,
}

impl Fixture {
    async fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("repo");
        let workspaces = dir.path().join("workspaces");
        let counter = dir.path().join("on_merged.count");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&workspaces).unwrap();

        git_sync(&root, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test User"),
            ("commit.gpgsign", "false"),
        ] {
            git_sync(&root, &["config", key, value]);
        }
        std::fs::write(root.join("shared.txt"), "base\n").unwrap();
        git_sync(&root, &["add", "-A"]);
        git_sync(&root, &["commit", "-q", "-m", "base"]);

        for (branch, file, contents) in [
            ("conflicting", "shared.txt", "conflicting\n"),
            ("clean-b", "b.txt", "b\n"),
            ("clean-c", "c.txt", "c\n"),
        ] {
            let path = workspaces.join(branch);
            git_sync(
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
            std::fs::write(path.join(file), contents).unwrap();
            git_sync(&path, &["add", "-A"]);
            git_sync(&path, &["commit", "-q", "-m", branch]);
        }

        std::fs::write(root.join("shared.txt"), "main\n").unwrap();
        git_sync(&root, &["commit", "-q", "-am", "main edit"]);

        let config = OrchestratorConfig {
            hooks: Some(crate::hooks::HooksConfig {
                on_merged: Some(crate::hooks::HookConfigValue::Simple(format!(
                    "echo merged >> '{}'",
                    counter.display()
                ))),
                ..Default::default()
            }),
            ..Default::default()
        };
        let backend = Arc::new(GitWorktreeBackend::new(root.clone(), Arc::new(config)));
        let sink = Arc::new(RecordingSink::default());
        let service = Arc::new(WorktreeService::new(
            backend,
            sink.clone(),
            workspaces.clone(),
        ));
        let port = Arc::new(RemoteWorktreeOperations::new(
            service,
            Arc::new(WorktreeRegistry::new()),
            root.clone(),
        ));
        let harness = harness(Some(TOKEN), &[]);
        harness.executor.bind_worktrees(port.clone());

        Self {
            dir,
            root,
            counter,
            sink,
            harness,
            port,
        }
    }

    /// The live opaque ID for a branch's worktree.
    async fn id(&self, branch: &str) -> String {
        self.port
            .list()
            .await
            .expect("listed")
            .worktrees
            .into_iter()
            .find(|w| w.branch == branch)
            .unwrap_or_else(|| panic!("worktree for {branch} is listed"))
            .worktree_id
    }

    fn head(&self) -> String {
        git_sync(&self.root, &["rev-parse", "HEAD"])
    }

    fn merge_head_present(&self) -> bool {
        self.root.join(".git").join("MERGE_HEAD").exists()
    }

    fn on_merged_runs(&self) -> usize {
        std::fs::read_to_string(&self.counter)
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    /// A shim, outside the repository, that fails `git merge --abort` and
    /// passes everything else through to the real `git`.
    fn failing_abort_shim(&self) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let real = std::env::split_paths(&std::env::var_os("PATH").expect("PATH is set"))
            .map(|dir| dir.join("git"))
            .find(|candidate| candidate.is_file())
            .expect("git is on PATH");
        let shim = self.dir.path().join("git-fail-abort");
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh\nif [ \"$1\" = merge ] && [ \"$2\" = --abort ]; then echo 'fatal: injected' >&2; exit 128; fi\nexec '{}' \"$@\"\n",
                real.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A child forked by a concurrent test while the write was open keeps a
        // writer reference until it execs, so the first exec of a fresh script
        // can fail with ETXTBSY. Once one exec succeeds no writer remains and
        // none can appear, so probe until then.
        for _ in 0..1000 {
            match std::process::Command::new(&shim)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .status()
            {
                Ok(_) => return shim,
                Err(err) if err.raw_os_error() == Some(libc::ETXTBSY) => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(err) => panic!("shim probe failed: {err}"),
            }
        }
        panic!("shim stayed busy");
    }
}

// ============================================================================
// Real Git
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_clean_branch_merges_and_runs_on_merged_once() {
    let fixture = Fixture::new().await;
    let before = fixture.head();
    let id = fixture.id("clean-b").await;

    let (status, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "succeeded", "{record}");
    let result = &record["result"];
    assert_eq!(result["kind"], "merge_abort");
    assert_eq!(result["outcome"], "merged");
    assert_eq!(result["worktree_id"], id);
    assert_eq!(result["branch"], "clean-b");
    assert_eq!(result["abort"], Value::Null);
    assert_eq!(result["recovery"], Value::Null);

    let parents = git_sync(&fixture.root, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        parents.split_whitespace().count(),
        3,
        "--no-ff merge commit"
    );
    assert!(parents.contains(&before));
    assert_eq!(fixture.on_merged_runs(), 1);
    assert_eq!(
        fixture.sink.events()[..2],
        [
            WorktreeOperationEvent::MergeStarted {
                branch: "clean-b".to_string()
            },
            WorktreeOperationEvent::MergeCompleted {
                branch: "clean-b".to_string()
            },
        ]
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_predicted_conflict_is_refused_with_files() {
    let fixture = Fixture::new().await;
    let before = fixture.head();
    let id = fixture.id("conflicting").await;

    let (status, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["state"], "failed");
    assert_eq!(record["error_code"], "merge_conflict_predicted");
    let result = &record["result"];
    assert_eq!(result["kind"], "merge_abort");
    assert_eq!(result["outcome"], "predicted_conflict");
    assert_eq!(result["conflict_files"], serde_json::json!(["shared.txt"]));
    assert_eq!(result["premerge_head"], Value::Null);
    assert_eq!(result["abort"], Value::Null);

    assert!(!fixture.merge_head_present());
    assert_eq!(fixture.head(), before);
    assert_eq!(fixture.on_merged_runs(), 0);
    assert_eq!(
        fixture.sink.events(),
        vec![WorktreeOperationEvent::MergeFailed {
            branch: "conflicting".to_string(),
            error: "the worktree conflicts with base".to_string(),
        }]
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_failed_abort_leaves_root_busy() {
    let fixture = Fixture::new().await;
    let before = fixture.head();

    // The conflict state a failed abort leaves behind, produced by the VCS
    // function itself under a shim whose `git merge --abort` fails.
    let shim = fixture.failing_abort_shim();
    let outcome = crate::vcs::git::commands::merge::aborting_merge_with(
        shim.as_os_str(),
        &fixture.root,
        "conflicting",
    )
    .await
    .expect("conflict is a value");
    let crate::vcs::git::commands::AbortingMergeOutcome::Conflict { abort, .. } = outcome else {
        panic!("the branch must conflict");
    };
    assert_eq!(abort.restored, Restoration::Failed);
    assert!(fixture.merge_head_present());

    // Every incompatible mutation is refused while `MERGE_HEAD` remains,
    // through either merge command.
    let id = fixture.id("clean-b").await;
    for (command, key) in [(MERGE_ABORT_COMMAND, "k1"), ("merge_worktree", "k2")] {
        let (_, record) = submit(&fixture.harness, command, &id, key).await;
        assert_eq!(record["error_code"], "root_busy", "{command}: {record}");
        assert_eq!(record["result"], Value::Null, "{command}: {record}");
    }
    assert!(fixture.merge_head_present(), "nothing was discarded");

    // Local recovery, then the merge is admissible again.
    git_sync(&fixture.root, &["merge", "--abort"]);
    assert_eq!(fixture.head(), before);
    let (_, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k3").await;
    assert_eq!(record["state"], "succeeded", "{record}");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_dirty_without_merge_head_refuses_clean_check() {
    let fixture = Fixture::new().await;
    let before = fixture.head();
    std::fs::write(fixture.root.join("shared.txt"), "dirty\n").unwrap();
    let id = fixture.id("clean-b").await;

    let (_, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(record["state"], "failed", "{record}");
    assert_ne!(record["error_code"], "root_busy", "{record}");
    assert!(
        record["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("Working directory is not clean"),
        "{record}"
    );
    assert!(!fixture.merge_head_present());
    assert_eq!(fixture.head(), before);
    assert_eq!(fixture.on_merged_runs(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_next_merge_after_confirmed_abort_succeeds() {
    let fixture = Fixture::new().await;
    let before = fixture.head();

    let outcome =
        crate::vcs::git::commands::merge_branch_aborting_on_conflict(&fixture.root, "conflicting")
            .await
            .expect("conflict is a value");
    let crate::vcs::git::commands::AbortingMergeOutcome::Conflict { abort, .. } = outcome else {
        panic!("the branch must conflict");
    };
    assert_eq!(abort.restored, Restoration::Confirmed);
    assert_eq!(fixture.head(), before);

    // Through either merge command.
    let b = fixture.id("clean-b").await;
    let (_, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &b, "k1").await;
    assert_eq!(record["state"], "succeeded", "{record}");
    let c = fixture.id("clean-c").await;
    let (_, record) = submit(&fixture.harness, "merge_worktree", &c, "k2").await;
    assert_eq!(record["state"], "succeeded", "{record}");
    // One run per completed merge; none for the aborted one.
    assert_eq!(fixture.on_merged_runs(), 2);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_merge_abort_merge_worktree_default_is_unchanged() {
    let fixture = Fixture::new().await;
    let id = fixture.id("conflicting").await;

    let (status, record) = submit(&fixture.harness, "merge_worktree", &id, "k1").await;

    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["error_code"], "merge_conflict");
    assert_eq!(
        record["result"],
        Value::Null,
        "merge_worktree gains no result kind"
    );
    let detail = record["detail"].as_str().unwrap_or_default();
    assert!(detail.contains("shared.txt"), "{detail}");
    assert!(detail.contains(RECOVERY_LOCAL_OR_TUI), "{detail}");
    assert!(
        fixture.merge_head_present(),
        "the intermediate state is retained"
    );
    assert_eq!(fixture.on_merged_runs(), 0);
}

// ============================================================================
// Restoration evidence through the router (fake backend)
// ============================================================================

/// Observes one mergeable worktree and fails every merge with a scripted abort.
struct AbortedBackend(Restoration);

#[async_trait]
impl WorktreeBackend for AbortedBackend {
    async fn observe(
        &self,
        _request: crate::worktree_ops::ObservationRequest,
    ) -> WorktreeOpResult<Vec<WorktreeFacts>> {
        let mut facts = WorktreeFacts::new("/srv/workspaces/alpha", "alpha");
        facts.identity = "gitdir: /srv/workspaces/alpha/.git".to_string();
        facts.head = "f".repeat(40);
        facts.has_commits_ahead = SafetyFact::Yes;
        Ok(vec![facts])
    }
    async fn base_head(&self) -> WorktreeOpResult<String> {
        Ok("e".repeat(40))
    }
    async fn create(&self, _: &Path, _: &str, _: &str) -> WorktreeOpResult<()> {
        Ok(())
    }
    async fn teardown(&self, _: &Path) -> WorktreeOpResult<()> {
        Ok(())
    }
    async fn remove_worktree(&self, _: &Path) -> WorktreeOpResult<()> {
        Ok(())
    }
    async fn branch_ref(&self, _: &str) -> WorktreeOpResult<Option<String>> {
        Ok(None)
    }
    async fn delete_branch_if_merged(&self, _: &str) -> WorktreeOpResult<()> {
        Ok(())
    }
    async fn delete_branch_at(&self, _: &str, _: &str) -> WorktreeOpResult<()> {
        Ok(())
    }
    async fn merge_into_base(
        &self,
        _branch: &str,
        policy: ConflictPolicy,
    ) -> WorktreeOpResult<MergeAttempt> {
        assert_eq!(policy, ConflictPolicy::AbortOnConflict);
        let confirmed = self.0 == Restoration::Confirmed;
        Ok(MergeAttempt::ConflictAborted {
            files: vec!["src/lib.rs".to_string()],
            abort: AbortReport {
                premerge_head: "e".repeat(40),
                exit_ok: confirmed,
                head_after: (self.0 != Restoration::Unknown).then(|| "e".repeat(40)),
                merge_head_present: Some(!confirmed),
                clean_after: Some(confirmed),
                restored: self.0,
                detail: if confirmed {
                    String::new()
                } else {
                    "fatal: injected".to_string()
                },
            },
        })
    }
    async fn run_on_merged(&self, _: &str, _: &Path) -> WorktreeOpResult<()> {
        panic!("on_merged must not run for an aborted merge");
    }
    async fn change_is_eligible(&self, _: &str) -> WorktreeOpResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn remote_merge_abort_real_conflict_settles_with_restoration_evidence() {
    for (restored, code, outcome, recovery) in [
        (
            Restoration::Confirmed,
            "merge_conflict_aborted",
            "conflict_aborted",
            Value::Null,
        ),
        (
            Restoration::Failed,
            "merge_abort_failed",
            "abort_failed",
            Value::from(RECOVERY_LOCAL_OR_TUI),
        ),
        (
            Restoration::Unknown,
            "merge_abort_failed",
            "abort_failed",
            Value::from(RECOVERY_LOCAL_OR_TUI),
        ),
    ] {
        let service = Arc::new(WorktreeService::new(
            Arc::new(AbortedBackend(restored)),
            Arc::new(RecordingSink::default()),
            PathBuf::from("/srv/workspaces"),
        ));
        let port = Arc::new(RemoteWorktreeOperations::new(
            service,
            Arc::new(WorktreeRegistry::new()),
            PathBuf::from("/srv/repo"),
        ));
        let harness = harness(Some(TOKEN), &[]);
        harness.executor.bind_worktrees(port.clone());
        let id = port.list().await.expect("listed").worktrees[0]
            .worktree_id
            .clone();

        let (status, record) = submit(&harness, MERGE_ABORT_COMMAND, &id, "k1").await;

        assert_eq!(status, StatusCode::CONFLICT, "{restored:?}: {record}");
        assert_eq!(record["state"], "failed");
        assert_eq!(record["error_code"], code, "{restored:?}");
        let result = &record["result"];
        assert_eq!(result["kind"], "merge_abort");
        assert_eq!(result["outcome"], outcome);
        assert_eq!(result["branch"], "alpha");
        assert_eq!(result["premerge_head"], "e".repeat(40));
        assert_eq!(result["conflict_files"], serde_json::json!(["src/lib.rs"]));
        assert_eq!(result["recovery"], recovery);
        assert_eq!(
            result["abort"]["restored"],
            format!("{restored:?}").to_lowercase()
        );
        assert_eq!(
            result["abort"]["exit_ok"],
            restored == Restoration::Confirmed
        );
        for member in ["head_after", "merge_head_present", "clean_after", "detail"] {
            assert!(
                result["abort"].get(member).is_some(),
                "{member} is always published: {result}"
            );
        }
        let detail = record["detail"].as_str().unwrap_or_default();
        assert!(!detail.contains("preserved"), "{detail}");
        if restored != Restoration::Confirmed {
            assert!(!detail.contains("Merge aborted"), "{detail}");
        }
    }
}

// ============================================================================
// Closed command surface
// ============================================================================

#[tokio::test]
async fn remote_merge_abort_command_is_closed() {
    assert!(SUPPORTED_COMMANDS.contains(&MERGE_ABORT_COMMAND));
    let parsed: CommandSpec = serde_json::from_str(
        r#"{"type":"merge_worktree_abort_on_conflict","target":{"worktree_id":"a"},"params":{}}"#,
    )
    .expect("the empty-params form parses");
    assert_eq!(parsed.type_name(), MERGE_ABORT_COMMAND);
    assert_eq!(
        parsed.target(),
        None,
        "addressed by opaque worktree ID only"
    );

    let harness = harness(Some(TOKEN), &[]);
    for (fragment, what) in [
        (
            r#"{"type":"merge_worktree_abort_on_conflict","target":{"worktree_id":"a"},"params":{"force":true}}"#,
            "unknown parameter",
        ),
        (
            r#"{"type":"merge_worktree_abort_on_conflict","target":{"worktree_id":"a","path":"/srv"},"params":{}}"#,
            "smuggled path",
        ),
        (
            r#"{"type":"merge_worktree","target":{"worktree_id":"a"},"params":{"conflict_policy":"abort"}}"#,
            "conflict_policy on the preserving command",
        ),
        // What an older owner sees: a command type it does not know.
        (
            r#"{"type":"merge_worktree_abort_on_conflict_v0","target":{"worktree_id":"a"},"params":{}}"#,
            "unknown command type",
        ),
    ] {
        let body = format!(
            r#"{},"expected_revision":0,"idempotency_key":"k"}}"#,
            fragment.strip_suffix('}').unwrap()
        );
        let (status, json) = status_and_json(
            send(
                &harness.router,
                post_json("/api/v2/commands", Some(TOKEN), &body),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{what}: {json}");
        assert_eq!(
            json["error_code"],
            ErrorCode::ValidationFailed.as_str(),
            "{what}"
        );
    }
    assert_eq!(
        harness.executor.call_count(),
        0,
        "nothing reached the executor"
    );
    assert!(harness.worktrees.abort_merges().is_empty());
}
