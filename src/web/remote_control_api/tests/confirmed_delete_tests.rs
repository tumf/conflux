//! `delete_worktree_confirmed` and its `delete-preview` read.
//!
//! The real-Git cases drive the real router, the real shared service, and the
//! real Git backend against a disposable repository whose managed worktrees
//! live outside it. The base commits a `.wt/teardown` that records each run in
//! a marker directory outside the repository, so "teardown ran" is a file on
//! disk rather than an inference. The contract cases at the end are pure.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{json, Value};
use tempfile::TempDir;

use super::{get, json_body, post_json, send, status_and_json, RecordingExecutor};
use crate::config::OrchestratorConfig;
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode, ALL_ERROR_CODES};
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::worktrees::{
    RemoteWorktreeOperations, WorktreeCapabilities, WorktreeOperations, WorktreeRegistry,
    CONFIRMED_DELETE_COMMAND, DELETE_PREVIEW_ROUTE,
};
use crate::web::remote_control_api::{router, RemoteControlState};
use crate::worktree_ops::consent::ConsentLimits;
use crate::worktree_ops::git_backend::GitWorktreeBackend;
use crate::worktree_ops::service::{NullEventSink, WorktreeService};

const TOKEN: &str = "confirmed-delete-token";

/// Teardown records its run, fails on request, or edits tracked content on
/// request — each keyed by the worktree's directory name, outside the repo.
const TEARDOWN: &str = r#"#!/bin/sh
name=$(basename "$PWD")
base="$ROOT_WORKTREE_PATH/.."
echo ran >> "$base/markers/$name"
if [ -f "$base/fail/$name" ]; then exit 3; fi
if [ -f "$base/edit/$name" ]; then echo teardown >> README; fi
exit 0
"#;

// ============================================================================
// Real-Git fixture
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

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

struct Fixture {
    dir: TempDir,
    root: PathBuf,
    workspaces: PathBuf,
    service: Arc<WorktreeService>,
    port: Arc<RemoteWorktreeOperations>,
    router: axum::Router,
    projection: Arc<Projection>,
    executor: Arc<RecordingExecutor>,
}

impl Fixture {
    fn new() -> Self {
        Self::with_limits(ConsentLimits::PRODUCTION)
    }

    fn with_limits(limits: ConsentLimits) -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("repo");
        let workspaces = dir.path().join("workspaces");
        for sub in ["repo", "workspaces", "markers", "fail", "edit"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        git(&root, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test User"),
            ("commit.gpgsign", "false"),
        ] {
            git(&root, &["config", key, value]);
        }
        write(&root.join("README"), "A");
        write(&root.join(".gitignore"), "target/\n*.log\n");
        write(&root.join(".wt/teardown"), TEARDOWN);
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                root.join(".wt/teardown"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "base"]);

        let backend = Arc::new(GitWorktreeBackend::new(
            root.clone(),
            Arc::new(OrchestratorConfig::default()),
        ));
        let service = Arc::new(
            WorktreeService::new(backend, Arc::new(NullEventSink), workspaces.clone())
                .with_consent_limits(limits),
        );
        let port = Arc::new(RemoteWorktreeOperations::new(
            service.clone(),
            Arc::new(WorktreeRegistry::new()),
            root.clone(),
        ));
        let projection = Arc::new(Projection::new());
        let auth = RemoteControlAuth::new(Some(TOKEN.to_string()), &[]).unwrap();
        let executor = RecordingExecutor::new();
        executor.bind_worktrees(port.clone());
        let router = router(
            RemoteControlState::new(projection.clone(), Arc::new(auth), executor.clone())
                .with_worktrees(port.clone()),
        );
        Self {
            dir,
            root,
            workspaces,
            service,
            port,
            router,
            projection,
            executor,
        }
    }

    /// Add a managed worktree on a new branch from base.
    fn add(&self, name: &str) -> PathBuf {
        let path = self.workspaces.join(name);
        git(
            &self.root,
            &["worktree", "add", "-q", "-b", name, path.to_str().unwrap()],
        );
        path
    }

    /// Add a managed worktree with one commit ahead of base.
    fn add_ahead(&self, name: &str) -> PathBuf {
        let path = self.add(name);
        write(&path.join("feature.txt"), "feature\n");
        git(&path, &["add", "-A"]);
        git(&path, &["commit", "-q", "-m", "feature"]);
        path
    }

    fn teardown_runs(&self, name: &str) -> usize {
        std::fs::read_to_string(self.dir.path().join("markers").join(name))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    fn fail_teardown(&self, name: &str, fail: bool) {
        let flag = self.dir.path().join("fail").join(name);
        if fail {
            write(&flag, "");
        } else {
            let _ = std::fs::remove_file(flag);
        }
    }

    fn edit_in_teardown(&self, name: &str) {
        write(&self.dir.path().join("edit").join(name), "");
    }

    fn branch_exists(&self, branch: &str) -> bool {
        StdCommand::new("git")
            .args([
                "rev-parse",
                "--verify",
                "-q",
                &format!("refs/heads/{branch}"),
            ])
            .current_dir(&self.root)
            .output()
            .unwrap()
            .status
            .success()
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

    async fn revision(&self) -> u64 {
        let state = json_body(send(&self.router, get("/api/v2/state", Some(TOKEN))).await).await;
        state["state_revision"].as_u64().unwrap_or(0)
    }

    async fn preview(&self, id: &str) -> (StatusCode, Value) {
        let uri = DELETE_PREVIEW_ROUTE.replace("{worktree_id}", id);
        status_and_json(send(&self.router, get(&uri, Some(TOKEN))).await).await
    }

    async fn submit(&self, body: Value) -> (StatusCode, Value) {
        let mut body = body;
        body["expected_revision"] = json!(self.revision().await);
        body["idempotency_key"] = json!(crate::ids::new_hex_id());
        status_and_json(
            send(
                &self.router,
                post_json("/api/v2/commands", Some(TOKEN), &body.to_string()),
            )
            .await,
        )
        .await
    }

    /// Submit `delete_worktree_confirmed` echoing `consent` verbatim.
    async fn confirm(
        &self,
        id: &str,
        consent: &Value,
        skip_teardown: bool,
        discard_known_dirty: bool,
        discard_commits_ahead: bool,
    ) -> (StatusCode, Value) {
        self.submit(json!({
            "type": CONFIRMED_DELETE_COMMAND,
            "target": {"worktree_id": id},
            "params": {
                "skip_teardown": skip_teardown,
                "discard_known_dirty": discard_known_dirty,
                "discard_commits_ahead": discard_commits_ahead,
                "consent": consent,
            },
        }))
        .await
    }

    async fn still_listed(&self, id: &str) -> bool {
        let uri = format!("/api/v2/worktrees/{id}");
        send(&self.router, get(&uri, Some(TOKEN))).await.status() == StatusCode::OK
    }
}

fn assert_refused(record: &Value, code: &str, reason: &str) {
    assert_eq!(record["state"], "failed", "{record}");
    assert_eq!(record["error_code"], code, "{record}");
    assert_eq!(
        record["result"]["kind"], "delete_worktree_confirmed",
        "{record}"
    );
    assert_eq!(record["result"]["outcome"], Value::Null, "{record}");
    assert_eq!(record["result"]["refusal"]["reason"], reason, "{record}");
}

// ============================================================================
// Real Git
// ============================================================================

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_preview_is_a_read_with_fresh_inspection() {
    let fixture = Fixture::new();
    let path = fixture.add_ahead("change-a");
    write(&path.join("README"), "dirty");
    let id = fixture.id("change-a").await;
    let revision = fixture.revision().await;

    let (status, preview) = fixture.preview(&id).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["worktree_id"], id);
    assert_eq!(preview["branch"], "change-a");
    assert_eq!(preview["dirty"], true);
    assert_eq!(preview["has_commits_ahead"], true);
    assert_eq!(
        preview["inspection"], "checked",
        "a fresh targeted inspection"
    );
    assert_eq!(preview["content_observation"], "observed");
    assert_eq!(preview["has_ignored_content"], false);
    assert_eq!(preview["teardown_script_present"], true);
    assert_eq!(preview["options"]["ordinary"]["eligible"], false);
    assert_eq!(
        preview["options"]["ordinary"]["blocked_reason"],
        "commits_ahead"
    );
    assert_eq!(
        preview["options"]["discard_known_dirty"]["blocked_reason"],
        "commits_ahead"
    );
    assert_eq!(
        preview["options"]["discard_commits_ahead"]["eligible"],
        true
    );
    assert_eq!(preview["options"]["discard_commits_ahead"]["commits"], 1);
    assert_eq!(
        preview["options"]["discard_commits_ahead"]["also_discards_dirty"],
        true
    );

    let consent = preview["consent"].as_object().unwrap();
    let mut keys: Vec<&String> = consent.keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "branch",
            "dirty",
            "discardable_content_id",
            "has_commits_ahead",
            "has_ignored_content",
            "head",
            "ignored_content_id",
            "merge_base",
        ]
    );
    assert!(preview["observed_at"].is_string());
    assert!(consent.get("observed_at").is_none());
    for field in ["discardable_content_id", "ignored_content_id"] {
        assert!(
            consent[field].as_str().unwrap().starts_with("sha256:"),
            "{field}"
        );
    }

    // A read: no command, no revision change, and the worktree is intact.
    assert_eq!(fixture.executor.call_count(), 0);
    assert_eq!(fixture.revision().await, revision);
    assert!(path.exists());

    let (status, missing) = fixture.preview(&"0".repeat(32)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["error_code"], "worktree_not_found");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_preview_consent_round_trips_verbatim() {
    let fixture = Fixture::new();
    fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    // The typed params decode the preview's own object, untouched.
    let spec: CommandSpec = serde_json::from_value(json!({
        "type": CONFIRMED_DELETE_COMMAND,
        "target": {"worktree_id": id},
        "params": {"skip_teardown": false, "discard_known_dirty": false,
                   "discard_commits_ahead": false, "consent": preview["consent"]},
    }))
    .expect("a verbatim echo never trips deny_unknown_fields");
    spec.validate().expect("a verbatim echo validates");

    // With nullable members left `null` by a non-observed outcome, too.
    let mut nulls = preview["consent"].clone();
    for field in [
        "has_ignored_content",
        "merge_base",
        "discardable_content_id",
        "ignored_content_id",
    ] {
        nulls[field] = Value::Null;
    }
    let spec: CommandSpec = serde_json::from_value(json!({
        "type": CONFIRMED_DELETE_COMMAND,
        "target": {"worktree_id": id},
        "params": {"skip_teardown": true, "discard_known_dirty": false,
                   "discard_commits_ahead": false, "consent": nulls},
    }))
    .unwrap();
    spec.validate()
        .expect("null nullable members validate without a discard permission");

    let (status, record) = fixture
        .confirm(&id, &preview["consent"], false, false, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "succeeded", "{record}");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_skip_teardown_removes_without_running_teardown() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    let (status, record) = fixture
        .confirm(&id, &preview["consent"], true, false, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["state"], "succeeded", "{record}");
    let result = &record["result"];
    assert_eq!(result["kind"], "delete_worktree_confirmed");
    assert_eq!(result["outcome"], "deleted");
    assert_eq!(result["teardown"], "skipped");
    assert_eq!(result["dirty_discarded"], false);
    assert_eq!(result["commits_discarded"], false);
    assert_eq!(result["branch_deleted"], true);
    assert_eq!(result["refusal"], Value::Null);
    assert_eq!(fixture.teardown_runs("change-a"), 0, "marker absent");
    assert!(!path.exists());
    assert!(!fixture.still_listed(&id).await, "the opaque ID is retired");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_ordinary_runs_teardown() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, false, false)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["teardown"], "ran");
    assert_eq!(fixture.teardown_runs("change-a"), 1, "marker present");
    assert!(!path.exists());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_discards_known_dirty() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    write(&path.join("README"), "B");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["options"]["ordinary"]["blocked_reason"], "dirty");
    assert_eq!(preview["options"]["discard_known_dirty"]["eligible"], true);
    assert_eq!(preview["options"]["discard_known_dirty"]["entries"], 1);

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, false, false)
        .await;
    assert_refused(&record, "worktree_dirty", "dirty");
    assert!(path.exists());

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, false)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["dirty_discarded"], true);
    assert_eq!(record["result"]["branch_deleted"], true);
    assert!(!path.exists());
    assert!(
        !fixture.branch_exists("change-a"),
        "the clean branch is cleaned up"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_discards_commits_ahead() {
    let fixture = Fixture::new();
    let path = fixture.add_ahead("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;
    assert!(preview["consent"]["merge_base"].is_string());

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, false, true)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["commits_discarded"], true);
    assert_eq!(record["result"]["branch_deleted"], true);
    assert!(!path.exists());
    assert!(
        !fixture.branch_exists("change-a"),
        "force-deleted at the consented commit"
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_ahead_with_dirty_needs_both_permissions() {
    let fixture = Fixture::new();
    let path = fixture.add_ahead("change-a");
    write(&path.join("README"), "dirty");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, false, true)
        .await;
    assert_refused(&record, "worktree_dirty", "dirty");
    assert!(path.exists());
    assert!(fixture.branch_exists("change-a"));
    assert_eq!(fixture.teardown_runs("change-a"), 0);

    // `discard_commits_ahead` with `dirty: false` and no identities never
    // reaches an observation at all.
    let mut bare = preview["consent"].clone();
    bare["dirty"] = json!(false);
    bare["discardable_content_id"] = Value::Null;
    bare["ignored_content_id"] = Value::Null;
    let (status, error) = fixture.confirm(&id, &bare, false, false, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    assert_eq!(error["error_code"], "validation_failed");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_discards_both_with_both_permissions() {
    let fixture = Fixture::new();
    let path = fixture.add_ahead("change-a");
    write(&path.join("README"), "dirty");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, true)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["dirty_discarded"], true);
    assert_eq!(record["result"]["commits_discarded"], true);
    assert!(!path.exists());
    assert!(!fixture.branch_exists("change-a"));
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_same_head_same_dirty_content_drift_is_stale() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    write(&path.join("README"), "A-edited");
    let id = fixture.id("change-a").await;

    // README A → B under the same HEAD and the same `dirty: true`.
    let (_, preview) = fixture.preview(&id).await;
    write(&path.join("README"), "B-edited");
    let (status, record) = fixture
        .confirm(&id, &preview["consent"], false, true, false)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{record}");
    assert_refused(&record, "delete_consent_stale", "content_changed");
    let observed = &record["result"]["refusal"]["observed"];
    assert_eq!(observed["dirty"], true);
    assert_eq!(observed["head"], preview["consent"]["head"]);
    assert_ne!(
        observed["discardable_content_id"], preview["consent"]["discardable_content_id"],
        "the refusal carries the new identity"
    );
    assert_eq!(record["result"]["refusal"]["teardown_ran"], false);
    assert!(path.exists());
    assert!(fixture.branch_exists("change-a"));
    assert!(fixture.still_listed(&id).await);

    // A new untracked file.
    let (_, preview) = fixture.preview(&id).await;
    write(&path.join("new.txt"), "new");
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, false)
        .await;
    assert_refused(&record, "delete_consent_stale", "content_changed");

    // A new commit.
    let (_, preview) = fixture.preview(&id).await;
    git(&path, &["add", "-A"]);
    git(&path, &["commit", "-q", "-m", "more"]);
    write(&path.join("README"), "C");
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, true)
        .await;
    assert_refused(&record, "delete_consent_stale", "head_moved");

    // Consent for `dirty: false` while the worktree is dirty.
    let (_, preview) = fixture.preview(&id).await;
    let mut clean = preview["consent"].clone();
    clean["dirty"] = json!(false);
    let (_, record) = fixture.confirm(&id, &clean, false, true, true).await;
    assert_refused(&record, "delete_consent_stale", "dirty_changed");

    // `has_commits_ahead` mismatch.
    let mut not_ahead = preview["consent"].clone();
    not_ahead["has_commits_ahead"] = json!(false);
    let (_, record) = fixture.confirm(&id, &not_ahead, false, true, true).await;
    assert_refused(&record, "delete_consent_stale", "ahead_changed");

    // A branch switch.
    let (_, preview) = fixture.preview(&id).await;
    git(&path, &["checkout", "-q", "-b", "switched"]);
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, true)
        .await;
    assert_refused(&record, "delete_consent_stale", "branch_changed");
    assert!(path.exists());
    assert_eq!(fixture.teardown_runs("change-a"), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_recreated_path_is_stale_identity() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    // Re-create a worktree at the same path, on the same branch and HEAD,
    // under a different Git administrative directory.
    git(
        &fixture.root,
        &["worktree", "remove", "--force", path.to_str().unwrap()],
    );
    let decoy = fixture.dir.path().join("decoy").join("change-a");
    git(
        &fixture.root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "decoy",
            decoy.to_str().unwrap(),
        ],
    );
    git(
        &fixture.root,
        &["worktree", "add", "-q", path.to_str().unwrap(), "change-a"],
    );

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, true, false)
        .await;
    assert_refused(&record, "delete_consent_stale", "identity_changed");
    assert!(path.exists(), "the re-created worktree is not removed");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_cancel_submits_nothing() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let revision = fixture.revision().await;

    let (status, _) = fixture.preview(&id).await;
    assert_eq!(status, StatusCode::OK);
    // The operator cancels: nothing is submitted.

    assert_eq!(fixture.executor.call_count(), 0, "no command delegated");
    assert_eq!(fixture.revision().await, revision);
    assert_eq!(fixture.projection.revision(), revision);
    assert!(fixture.still_listed(&id).await);
    assert!(path.exists());
    assert_eq!(fixture.teardown_runs("change-a"), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_teardown_failure_then_skip_recovery() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    fixture.fail_teardown("change-a", true);
    let id = fixture.id("change-a").await;

    // The ordinary delete fails, retaining the worktree and its ID.
    let (_, record) = fixture
        .submit(json!({"type": "delete_worktree", "target": {"worktree_id": id}, "params": {}}))
        .await;
    assert_eq!(record["state"], "failed", "{record}");
    assert_eq!(record["error_code"], "internal_error");
    assert!(path.exists());
    assert!(fixture.still_listed(&id).await);

    // So does a consented one that does not skip teardown.
    let (_, preview) = fixture.preview(&id).await;
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, false, false)
        .await;
    assert_refused(&record, "internal_error", "teardown_failed");
    assert_eq!(record["result"]["refusal"]["teardown_ran"], true);
    assert!(path.exists());

    // Fresh consent with `skip_teardown` is the recovery.
    let runs = fixture.teardown_runs("change-a");
    let (_, preview) = fixture.preview(&id).await;
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, false, false)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["teardown"], "skipped");
    assert_eq!(fixture.teardown_runs("change-a"), runs);
    assert!(!path.exists());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_teardown_that_edits_tracked_content_is_stale() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    write(&path.join("README"), "B");
    fixture.edit_in_teardown("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], false, true, false)
        .await;
    assert_refused(&record, "delete_consent_stale", "content_changed");
    assert_eq!(record["result"]["refusal"]["teardown_ran"], true);
    assert_eq!(record["result"]["teardown"], "ran");
    assert!(path.exists(), "the worktree is retained");
    assert!(fixture.still_listed(&id).await);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_refuses_main_busy_root_and_unknown() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");

    // Main worktree.
    let main_id = fixture.id("main").await;
    let (_, preview) = fixture.preview(&main_id).await;
    assert_eq!(preview["is_main"], true);
    assert_eq!(
        preview["options"]["ordinary"]["blocked_reason"],
        "main_worktree"
    );
    let (_, record) = fixture
        .confirm(&main_id, &preview["consent"], true, true, false)
        .await;
    assert_refused(&record, "target_ineligible", "main_worktree");

    // Held root guard.
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;
    {
        let _held = fixture.service.acquire_root_for_test().unwrap();
        let (_, record) = fixture
            .confirm(&id, &preview["consent"], true, true, false)
            .await;
        assert_refused(&record, "root_busy", "root_busy");
    }
    assert!(path.exists());

    // An unreadable index makes the dirty state (and the content) unknown.
    let admin = PathBuf::from(git(&path, &["rev-parse", "--absolute-git-dir"]));
    std::fs::write(admin.join("index"), b"not an index").unwrap();
    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["dirty"], Value::Null, "{preview}");
    assert_eq!(preview["content_observation"], "status_failed");
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, false, false)
        .await;
    assert_refused(&record, "worktree_dirty_unknown", "dirty_unknown");
    assert!(path.exists());
    assert_eq!(fixture.teardown_runs("change-a"), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_ordinary_delete_is_unchanged() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;

    for params in [
        json!({"skip_teardown": true}),
        json!({"discard_known_dirty": true}),
        json!({"discard_commits_ahead": true}),
    ] {
        let (status, error) = fixture
            .submit(
                json!({"type": "delete_worktree", "target": {"worktree_id": id}, "params": params}),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{params}: {error}"
        );
        assert_eq!(error["error_code"], "validation_failed");
    }
    assert_eq!(fixture.executor.call_count(), 0, "nothing delegated");

    // Fail-closed refusals are unchanged: a dirty worktree still refuses.
    write(&path.join("README"), "dirty");
    let (_, record) = fixture
        .submit(json!({"type": "delete_worktree", "target": {"worktree_id": id}, "params": {}}))
        .await;
    assert_eq!(record["error_code"], "worktree_dirty", "{record}");
    assert_eq!(record["result"], Value::Null, "{record}");
    assert!(path.exists());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_ignored_content_is_disclosed_and_consented() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    write(&path.join("build.log"), "one");
    let id = fixture.id("change-a").await;

    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["dirty"], false);
    assert_eq!(preview["has_ignored_content"], true);
    assert_eq!(preview["ignored_entries"], 1);
    assert_eq!(
        preview["options"]["ordinary"]["blocked_reason"],
        "ignored_content"
    );
    assert_eq!(
        preview["options"]["skip_teardown"]["blocked_reason"],
        "ignored_content"
    );
    assert_eq!(preview["options"]["discard_known_dirty"]["eligible"], true);

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, false, false)
        .await;
    assert_refused(&record, "worktree_dirty", "ignored_content");
    assert!(path.exists());

    write(&path.join("build.log"), "two");
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, true, false)
        .await;
    assert_refused(&record, "delete_consent_stale", "content_changed");
    assert!(path.exists());

    let (_, fresh) = fixture.preview(&id).await;
    let (_, record) = fixture
        .confirm(&id, &fresh["consent"], true, true, false)
        .await;
    assert_eq!(record["state"], "succeeded", "{record}");
    assert_eq!(record["result"]["dirty_discarded"], true);
    assert!(!path.exists());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_ignored_directory_drift_is_stale() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    write(&path.join("target/deep/b.o"), "AAAA");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["dirty"], false);
    assert_eq!(preview["has_ignored_content"], true);

    // Same-size rewrite with mtime and mode restored.
    let file = path.join("target/deep/b.o");
    let before = std::fs::metadata(&file).unwrap();
    std::fs::write(&file, "AAAB").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    std::fs::set_permissions(&file, before.permissions()).unwrap();
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        before.modified().unwrap()
    );

    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, true, false)
        .await;
    assert_refused(&record, "delete_consent_stale", "content_changed");
    assert_ne!(
        record["result"]["refusal"]["observed"]["ignored_content_id"],
        preview["consent"]["ignored_content_id"]
    );
    assert!(path.exists());
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_unbounded_observation_fails_closed() {
    let fixture = Fixture::with_limits(ConsentLimits {
        max_records: 4,
        max_hashed_bytes: 1024,
        ..ConsentLimits::PRODUCTION
    });
    let path = fixture.add("change-a");
    for index in 0..8 {
        write(&path.join(format!("target/f{index}.o")), "x");
    }
    let id = fixture.id("change-a").await;

    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["dirty"], false);
    assert_eq!(preview["content_observation"], "unbounded");
    assert_eq!(preview["consent"]["discardable_content_id"], Value::Null);
    assert_eq!(preview["consent"]["ignored_content_id"], Value::Null);
    assert_eq!(preview["has_ignored_content"], Value::Null);
    for option in [
        "ordinary",
        "skip_teardown",
        "discard_known_dirty",
        "discard_commits_ahead",
    ] {
        assert_eq!(preview["options"][option]["eligible"], false, "{option}");
        assert_eq!(
            preview["options"][option]["blocked_reason"], "content_unbounded",
            "{option}"
        );
    }

    // Every permission true: the consent cannot carry identities, so give it
    // well-formed ones it could never have earned. It still refuses.
    let mut consent = preview["consent"].clone();
    consent["discardable_content_id"] = json!(format!("sha256:{}", "0".repeat(64)));
    consent["ignored_content_id"] = json!(format!("sha256:{}", "0".repeat(64)));
    let (_, record) = fixture.confirm(&id, &consent, true, true, true).await;
    assert_refused(&record, "worktree_dirty_unknown", "content_unbounded");
    assert_eq!(record["result"]["refusal"]["teardown_ran"], false);
    assert!(path.exists());
    assert!(fixture.still_listed(&id).await);
    assert_eq!(fixture.teardown_runs("change-a"), 0, "no marker written");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_confirmed_delete_nested_repository_fails_closed() {
    for nested in ["vendor", "target/nested"] {
        let fixture = Fixture::new();
        let path = fixture.add("change-a");
        git(&path, &["init", "-q", nested]);
        write(&path.join(nested).join("file.txt"), "inner");
        let id = fixture.id("change-a").await;

        let (_, preview) = fixture.preview(&id).await;
        assert_eq!(
            preview["content_observation"], "nested_repository",
            "{nested}"
        );
        for option in [
            "ordinary",
            "skip_teardown",
            "discard_known_dirty",
            "discard_commits_ahead",
        ] {
            assert_eq!(
                preview["options"][option]["eligible"], false,
                "{nested} {option}"
            );
        }
        let mut consent = preview["consent"].clone();
        consent["discardable_content_id"] = json!(format!("sha256:{}", "0".repeat(64)));
        consent["ignored_content_id"] = json!(format!("sha256:{}", "0".repeat(64)));
        let (_, record) = fixture.confirm(&id, &consent, true, true, false).await;
        assert_refused(&record, "target_ineligible", "nested_repository");
        assert!(path.join(nested).exists(), "{nested}");
        assert_eq!(fixture.teardown_runs("change-a"), 0, "{nested}");
    }
}

// ============================================================================
// Contract
// ============================================================================

#[test]
fn remote_confirmed_delete_commands_are_closed() {
    let consent = json!({
        "branch": "a", "head": "a".repeat(40), "dirty": true, "has_commits_ahead": false,
        "has_ignored_content": false, "merge_base": "b".repeat(40),
        "discardable_content_id": format!("sha256:{}", "c".repeat(64)),
        "ignored_content_id": format!("sha256:{}", "d".repeat(64)),
    });
    let command = |params: Value| json!({"type": CONFIRMED_DELETE_COMMAND, "target": {"worktree_id": "id"}, "params": params});
    let parse = |value: Value| serde_json::from_value::<CommandSpec>(value);

    let good = command(json!({"skip_teardown": false, "discard_known_dirty": true,
                              "discard_commits_ahead": true, "consent": consent}));
    parse(good).unwrap().validate().unwrap();

    // Unknown members are schema failures, in params, consent, and target.
    let mut extra = consent.clone();
    extra["observed_at"] = json!("now");
    for bad in [
        command(json!({"skip_teardown": false, "discard_known_dirty": false,
                       "discard_commits_ahead": false, "consent": consent, "force": true})),
        command(json!({"skip_teardown": false, "discard_known_dirty": false,
                       "discard_commits_ahead": false, "consent": extra})),
        json!({"type": CONFIRMED_DELETE_COMMAND, "target": {"worktree_id": "id", "path": "/x"},
               "params": {"skip_teardown": false, "discard_known_dirty": false,
                          "discard_commits_ahead": false, "consent": consent}}),
        command(
            json!({"discard_known_dirty": false, "discard_commits_ahead": false,
                       "consent": consent}),
        ),
    ] {
        assert!(parse(bad.clone()).is_err(), "{bad}");
    }

    // Field validation.
    let validate = |params: Value| parse(command(params)).unwrap().validate();
    let with = |field: &str, value: Value| {
        let mut c = consent.clone();
        c[field] = value;
        c
    };
    for (label, params) in [
        (
            "missing identities with discard_known_dirty",
            json!({"skip_teardown": false, "discard_known_dirty": true, "discard_commits_ahead": false,
                   "consent": with("ignored_content_id", Value::Null)}),
        ),
        (
            "discard_commits_ahead, dirty false, missing identities",
            json!({"skip_teardown": false, "discard_known_dirty": false, "discard_commits_ahead": true,
                   "consent": {
                       "branch": "a", "head": "a".repeat(40), "dirty": false, "has_commits_ahead": true,
                       "has_ignored_content": false, "merge_base": "b".repeat(40),
                       "discardable_content_id": null, "ignored_content_id": null}}),
        ),
        (
            "short head",
            json!({"skip_teardown": true, "discard_known_dirty": false, "discard_commits_ahead": false,
                   "consent": with("head", json!("abc123"))}),
        ),
        (
            "blank branch",
            json!({"skip_teardown": true, "discard_known_dirty": false, "discard_commits_ahead": false,
                   "consent": with("branch", json!(" "))}),
        ),
        (
            "malformed identity",
            json!({"skip_teardown": true, "discard_known_dirty": false, "discard_commits_ahead": false,
                   "consent": with("discardable_content_id", json!("sha256:XYZ"))}),
        ),
        (
            "ahead without merge base",
            json!({"skip_teardown": false, "discard_known_dirty": false, "discard_commits_ahead": true,
                   "consent": with("merge_base", Value::Null)}),
        ),
    ] {
        assert!(validate(params).is_err(), "{label}");
    }
}

#[tokio::test]
async fn remote_confirmed_delete_validation_failure_is_a_422_with_no_delegation() {
    let harness = super::harness(Some(TOKEN), &[]);
    let body = json!({
        "type": CONFIRMED_DELETE_COMMAND,
        "target": {"worktree_id": "id-a"},
        "params": {"skip_teardown": false, "discard_known_dirty": true, "discard_commits_ahead": false,
                   "consent": {"branch": "a", "head": "a".repeat(40), "dirty": true,
                               "has_commits_ahead": false, "has_ignored_content": false,
                               "merge_base": null, "discardable_content_id": null,
                               "ignored_content_id": null}},
        "expected_revision": 0,
        "idempotency_key": "k1",
    });
    let (status, error) = status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body.to_string()),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    assert_eq!(error["error_code"], "validation_failed");
    assert_eq!(harness.executor.call_count(), 0);
    assert!(harness.worktrees.confirmed_deletes().is_empty());
}

#[test]
fn remote_confirmed_delete_old_owner_capabilities_decode_as_unavailable() {
    let current = serde_json::to_value(WorktreeCapabilities::default()).unwrap();
    assert_eq!(current["delete_requires_teardown"], true);
    assert_eq!(current["confirmed_delete"]["available"], true);
    assert_eq!(
        current["confirmed_delete"]["command"],
        CONFIRMED_DELETE_COMMAND
    );
    assert_eq!(
        current["confirmed_delete"]["preview_route"],
        DELETE_PREVIEW_ROUTE
    );
    assert_eq!(
        current["confirmed_delete"]["permissions"],
        json!([
            "skip_teardown",
            "discard_known_dirty",
            "discard_commits_ahead"
        ])
    );
    assert_eq!(
        current["confirmed_delete"]["consent_fields"],
        json!([
            "branch",
            "head",
            "dirty",
            "has_commits_ahead",
            "has_ignored_content",
            "merge_base",
            "discardable_content_id",
            "ignored_content_id"
        ])
    );
    assert_eq!(
        current["confirmed_delete"]["content_id_algorithm"],
        "sha256"
    );
    assert_eq!(current["confirmed_delete"]["binds_ignored_content"], true);
    assert!(current["operations"]
        .as_array()
        .unwrap()
        .contains(&json!("delete_confirmed")));

    // An older owner omits the block entirely and still says teardown is required.
    let mut older = current.clone();
    older.as_object_mut().unwrap().remove("confirmed_delete");
    let decoded: WorktreeCapabilities = serde_json::from_value(older).unwrap();
    assert!(!decoded.confirmed_delete.available);
    assert!(decoded.confirmed_delete.command.is_empty());
    assert!(decoded.delete_requires_teardown);
}

#[test]
fn remote_confirmed_delete_stale_consent_has_its_own_conflict_code() {
    assert!(ALL_ERROR_CODES.contains(&ErrorCode::DeleteConsentStale));
    assert_eq!(
        ErrorCode::DeleteConsentStale.as_str(),
        "delete_consent_stale"
    );
    assert_eq!(
        ErrorCode::DeleteConsentStale.http_status(),
        StatusCode::CONFLICT
    );
}

// ============================================================================
// Contract gaps: listing budget and detached HEAD
// ============================================================================

const OPTIONS: [&str; 4] = [
    "ordinary",
    "skip_teardown",
    "discard_known_dirty",
    "discard_commits_ahead",
];

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_delete_listing_name_bytes_are_bounded_in_the_preview() {
    let fixture = Fixture::with_limits(ConsentLimits {
        max_listing_name_bytes: 16,
        ..ConsentLimits::PRODUCTION
    });
    let path = fixture.add("change-a");
    for index in 0..20 {
        write(&path.join(format!("target/f{index}.o")), "x");
    }
    let id = fixture.id("change-a").await;

    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["content_observation"], "unbounded", "{preview}");
    assert_eq!(preview["consent"]["discardable_content_id"], Value::Null);
    assert_eq!(preview["consent"]["ignored_content_id"], Value::Null);
    for option in OPTIONS {
        assert_eq!(preview["options"][option]["eligible"], false, "{option}");
        assert_eq!(
            preview["options"][option]["blocked_reason"], "content_unbounded",
            "{option}"
        );
    }
}

/// The opaque ID of the one detached worktree.
async fn detached_id(fixture: &Fixture) -> String {
    fixture
        .port
        .list()
        .await
        .expect("listed")
        .worktrees
        .into_iter()
        .find(|w| w.branch.is_empty() && !w.is_main)
        .expect("the detached worktree is listed")
        .worktree_id
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_delete_detached_preview_is_ineligible() {
    let fixture = Fixture::new();
    let path = fixture.add_ahead("change-a");
    git(&path, &["checkout", "-q", "--detach"]);
    let id = detached_id(&fixture).await;

    let (status, preview) = fixture.preview(&id).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["is_detached"], true, "{preview}");
    assert_eq!(preview["branch"], "");
    assert_eq!(preview["consent"]["branch"], "");
    for option in OPTIONS {
        assert_eq!(preview["options"][option]["eligible"], false, "{option}");
        assert_eq!(
            preview["options"][option]["blocked_reason"], "detached_head",
            "{option}: {preview}"
        );
    }

    // The published consent can never be accepted: no detached destructive
    // support exists remotely.
    let (status, error) = fixture
        .confirm(&id, &preview["consent"], true, true, true)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    assert_eq!(error["error_code"], "validation_failed");
    assert!(path.exists());
    assert!(fixture.still_listed(&id).await);
    assert_eq!(fixture.teardown_runs("change-a"), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_delete_detach_after_preview_is_refused_under_guard() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    let id = fixture.id("change-a").await;
    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["is_detached"], false);
    assert_eq!(
        preview["options"]["skip_teardown"]["eligible"], true,
        "{preview}"
    );

    git(&path, &["checkout", "-q", "--detach"]);
    let (_, record) = fixture
        .confirm(&id, &preview["consent"], true, false, false)
        .await;
    assert_refused(&record, "target_ineligible", "detached_head");
    assert_eq!(record["result"]["refusal"]["teardown_ran"], false);
    assert!(path.exists(), "nothing was removed");
    assert!(fixture.still_listed(&id).await, "the ID is retained");
}

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn remote_contract_gap_delete_attached_ahead_unknown_is_unchanged() {
    let fixture = Fixture::new();
    let path = fixture.add("change-a");
    // An attached branch with history unrelated to base: no merge base.
    git(&path, &["checkout", "-q", "--orphan", "change-a-orphan"]);
    git(&path, &["commit", "-q", "-m", "orphan"]);
    let id = fixture.id("change-a-orphan").await;

    let (_, preview) = fixture.preview(&id).await;
    assert_eq!(preview["is_detached"], false);
    assert_eq!(
        preview["options"]["discard_commits_ahead"]["blocked_reason"], "ahead_unknown",
        "{preview}"
    );
}

#[test]
fn remote_contract_gap_delete_capability_lists_detached_head() {
    let capability = WorktreeCapabilities::default().confirmed_delete;
    assert!(capability.reasons.iter().any(|r| r == "detached_head"));
    assert_eq!(
        crate::worktree_ops::consent::ConfirmedDeleteReason::ALL.len(),
        20
    );
}
