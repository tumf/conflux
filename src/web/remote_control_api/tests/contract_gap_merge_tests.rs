//! Typed landing and hook evidence of `merge_worktree_abort_on_conflict`.
//!
//! A hook failure after a landed merge, and a Git failure after eligibility but
//! before any merge commit, both settle with a `merge_abort` result whose
//! `merge.landed` is re-read from the repository. `merge_worktree` keeps
//! `internal_error` with no result for the same failures.
//!
//! The real-Git cases drive the real router, service, and Git backend against a
//! disposable repository; the unknown-landing case uses a fake backend whose
//! re-read cannot answer.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::{json, Value};
use tempfile::TempDir;

use super::{get, harness, json_body, post_json, send, status_and_json, Harness};
use crate::config::OrchestratorConfig;
use crate::vcs::git::commands::Landing;
use crate::web::remote_control_api::dto::ErrorCode;
use crate::web::remote_control_api::worktrees::{
    RemoteWorktreeOperations, WorktreeCapabilities, WorktreeOperations, WorktreeRegistry,
    MERGE_ABORT_COMMAND, MERGE_ABORT_REASONS,
};
use crate::worktree_ops::git_backend::GitWorktreeBackend;
use crate::worktree_ops::service::{
    ConflictPolicy, MergeAttempt, NullEventSink, SafetyFact, WorktreeBackend, WorktreeFacts,
    WorktreeOpError, WorktreeOpResult, WorktreeService, RECOVERY_LOCAL_OR_TUI,
};

const TOKEN: &str = "contract-gap-merge-token";

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

/// A base repository on `main` with `clean-b` and `clean-c` (one commit ahead
/// each) and `fresh` (none ahead), and an `on_merged` hook that counts its runs
/// into a file outside the repository and exits with `hook_exit`.
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    counter: PathBuf,
    harness: Harness,
    service: Arc<WorktreeService>,
    port: Arc<RemoteWorktreeOperations>,
}

impl Fixture {
    async fn new(hook_exit: i32) -> Self {
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

        for (branch, file) in [
            ("clean-b", Some("b.txt")),
            ("clean-c", Some("c.txt")),
            ("fresh", None),
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
            if let Some(file) = file {
                std::fs::write(path.join(file), "x\n").unwrap();
                git_sync(&path, &["add", "-A"]);
                git_sync(&path, &["commit", "-q", "-m", branch]);
            }
        }

        let config = OrchestratorConfig {
            hooks: Some(crate::hooks::HooksConfig {
                on_merged: Some(crate::hooks::HookConfigValue::Full(
                    crate::hooks::HookConfig {
                        command: format!(
                            "echo merged >> '{}'; exit {hook_exit}",
                            counter.display()
                        ),
                        continue_on_failure: false,
                        timeout: 60,
                        git_commit_no_verify: false,
                        max_retries: 0,
                        retry_delay_secs: 0,
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        };
        let backend = Arc::new(GitWorktreeBackend::new(root.clone(), Arc::new(config)));
        let service = Arc::new(WorktreeService::new(
            backend,
            Arc::new(NullEventSink),
            workspaces.clone(),
        ));
        let port = Arc::new(RemoteWorktreeOperations::new(
            service.clone(),
            Arc::new(WorktreeRegistry::new()),
            root.clone(),
        ));
        let harness = harness(Some(TOKEN), &[]);
        harness.executor.bind_worktrees(port.clone());
        Self {
            _dir: dir,
            root,
            counter,
            harness,
            service,
            port,
        }
    }

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

    fn on_merged_runs(&self) -> usize {
        std::fs::read_to_string(&self.counter)
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_hook_failure_after_landed_merge_is_typed() {
    let fixture = Fixture::new(1).await;
    let before = fixture.head();
    let id = fixture.id("clean-b").await;

    let (status, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_eq!(record["state"], "failed", "{record}");
    assert_eq!(record["error_code"], "merge_hook_failed", "{record}");
    assert!(record["detail"]
        .as_str()
        .unwrap_or_default()
        .contains("on_merged hook failed for 'clean-b'; branch merged transition blocked"));
    let result = &record["result"];
    assert_eq!(result["kind"], "merge_abort");
    assert_eq!(result["outcome"], "merged_hook_failed");
    assert_eq!(result["premerge_head"], before.as_str());
    assert_eq!(result["recovery"], RECOVERY_LOCAL_OR_TUI);
    assert_eq!(result["abort"], Value::Null);
    let merge = &result["merge"];
    assert_eq!(merge["landed"], "confirmed", "{result}");
    assert_eq!(merge["merge_head_present"], false);
    assert_eq!(merge["detail"], Value::Null);
    let head_after = merge["head_after"].as_str().expect("HEAD was re-read");
    assert_ne!(head_after, before);
    let first_parent = git_sync(&fixture.root, &["rev-parse", &format!("{head_after}^1")]);
    assert_eq!(
        first_parent, before,
        "a merge commit on top of premerge_head"
    );
    let hook = &result["hook"];
    assert_eq!(hook["ran"], true);
    assert_eq!(hook["exit_ok"], false);
    assert!(
        hook["detail"].as_str().is_some_and(|d| !d.is_empty()),
        "{hook}"
    );

    // Exactly one hook run; nothing re-merged, aborted, reset, or re-ran.
    assert_eq!(fixture.on_merged_runs(), 1);
    assert_eq!(
        fixture.head(),
        head_after,
        "base HEAD is unchanged after settlement"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_worktree_hook_failure_is_unchanged() {
    let fixture = Fixture::new(1).await;
    let id = fixture.id("clean-b").await;

    let (status, record) = submit(&fixture.harness, "merge_worktree", &id, "k1").await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{record}");
    assert_eq!(record["error_code"], "internal_error", "{record}");
    assert_eq!(record["result"], Value::Null, "{record}");
    assert_eq!(fixture.on_merged_runs(), 1);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_pre_merge_failure_reports_not_landed() {
    let fixture = Fixture::new(0).await;
    let before = fixture.head();
    // Dirty base: eligibility passes, the clean check inside the merge fails.
    std::fs::write(fixture.root.join("shared.txt"), "dirty\n").unwrap();
    let id = fixture.id("clean-b").await;

    let (status, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{record}");
    assert_eq!(record["error_code"], "internal_error", "{record}");
    assert!(record["detail"]
        .as_str()
        .unwrap_or_default()
        .contains("Working directory is not clean"));
    let result = &record["result"];
    assert_eq!(result["outcome"], "merge_failed", "{record}");
    assert_eq!(result["premerge_head"], before.as_str());
    assert_eq!(result["merge"]["landed"], "not_landed", "{result}");
    assert_eq!(result["merge"]["head_after"], before.as_str());
    assert_eq!(result["merge"]["merge_head_present"], false);
    assert_eq!(result["merge"]["clean_after"], false, "informational only");
    assert_eq!(
        result["hook"],
        json!({"ran": false, "exit_ok": null, "detail": null})
    );
    assert_eq!(result["recovery"], Value::Null);
    assert_eq!(fixture.on_merged_runs(), 0);
    assert_eq!(fixture.head(), before);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_success_carries_confirmed_landing() {
    let fixture = Fixture::new(0).await;
    let id = fixture.id("clean-c").await;

    let (status, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(status, StatusCode::OK, "{record}");
    let result = &record["result"];
    assert_eq!(result["outcome"], "merged");
    assert_eq!(result["merge"]["landed"], "confirmed", "{result}");
    assert_eq!(result["merge"]["head_after"], fixture.head().as_str());
    assert_eq!(
        result["hook"],
        json!({"ran": true, "exit_ok": true, "detail": null})
    );
    assert_eq!(fixture.on_merged_runs(), 1);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_precheck_refusals_keep_null_result() {
    let fixture = Fixture::new(0).await;

    // Unknown opaque ID.
    let (_, record) = submit(
        &fixture.harness,
        MERGE_ABORT_COMMAND,
        "0123456789abcdef0123456789abcdef",
        "k1",
    )
    .await;
    assert_eq!(record["error_code"], "worktree_not_found", "{record}");
    assert_eq!(record["result"], Value::Null);

    // Nothing ahead of base: refused at eligibility.
    let fresh = fixture.id("fresh").await;
    let (_, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &fresh, "k2").await;
    assert_eq!(record["error_code"], "target_ineligible", "{record}");
    assert_eq!(record["result"], Value::Null);

    // The root guard is held by another operation.
    let id = fixture.id("clean-b").await;
    {
        let _guard = fixture.service.acquire_root_for_test().expect("guard");
        let (_, record) = submit(&fixture.harness, MERGE_ABORT_COMMAND, &id, "k3").await;
        assert_eq!(record["error_code"], "root_busy", "{record}");
        assert_eq!(record["result"], Value::Null);
    }
    assert_eq!(fixture.on_merged_runs(), 0);
}

#[test]
fn remote_contract_gap_merge_capability_lists_new_reasons() {
    let capability = WorktreeCapabilities::default().merge_abort;
    for reason in ["merge_failed", "merged_hook_failed"] {
        assert!(MERGE_ABORT_REASONS.contains(&reason));
        assert!(capability.reasons.iter().any(|r| r == reason), "{reason}");
    }
    assert_eq!(ErrorCode::MergeHookFailed.as_str(), "merge_hook_failed");
    assert_eq!(
        ErrorCode::MergeHookFailed.http_status(),
        StatusCode::CONFLICT
    );
    assert!(
        crate::web::remote_control_api::dto::ALL_ERROR_CODES.contains(&ErrorCode::MergeHookFailed)
    );
}

// ============================================================================
// Unknown landing (fake backend)
// ============================================================================

/// One mergeable worktree whose merge fails with a Git error, on a backend
/// whose landing re-read cannot answer (the trait default).
struct UnreadableLandingBackend;

#[async_trait]
impl WorktreeBackend for UnreadableLandingBackend {
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
        _policy: ConflictPolicy,
    ) -> WorktreeOpResult<MergeAttempt> {
        Err(WorktreeOpError::Internal(
            "git merge alpha failed: fatal: injected".to_string(),
        ))
    }
    async fn run_on_merged(&self, _: &str, _: &Path) -> WorktreeOpResult<()> {
        panic!("on_merged must not run when the merge failed");
    }
    async fn change_is_eligible(&self, _: &str) -> WorktreeOpResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn remote_contract_gap_merge_unknown_landing_is_not_inferred() {
    let service = Arc::new(WorktreeService::new(
        Arc::new(UnreadableLandingBackend),
        Arc::new(NullEventSink),
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

    let (_, record) = submit(&harness, MERGE_ABORT_COMMAND, &id, "k1").await;

    assert_eq!(record["error_code"], "internal_error", "{record}");
    let result = &record["result"];
    assert_eq!(result["outcome"], "merge_failed");
    assert_eq!(result["merge"]["landed"], "unknown", "{result}");
    assert!(
        result["merge"]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("re-read")),
        "{result}"
    );
    assert_eq!(result["merge"]["head_after"], Value::Null);
    assert_eq!(result["hook"]["ran"], false);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_merge_unreadable_reread_names_the_failed_read() {
    let fixture = Fixture::new(0).await;
    let before = fixture.head();
    // A Git program that cannot read anything.
    let report = crate::vcs::git::commands::merge::read_merge_landing_with(
        std::ffi::OsStr::new("/nonexistent/git"),
        &fixture.root,
        Some(&before),
    )
    .await;
    assert_eq!(report.landed, Landing::Unknown);
    assert_eq!(report.head_after, None);
    assert!(report
        .detail
        .as_deref()
        .is_some_and(|d| d.contains("HEAD could not be re-read")));

    // The real program reads `not_landed` for the untouched base.
    let report = crate::vcs::git::commands::read_merge_landing(&fixture.root, Some(&before)).await;
    assert_eq!(report.landed, Landing::NotLanded);
}
