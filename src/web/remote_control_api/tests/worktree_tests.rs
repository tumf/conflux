//! Repository-local tests for the `/api/v2` worktree contract.
//!
//! Fast and boundary-free: opaque identity, redaction, the closed command
//! shapes, and the inherited v2 guards are all decidable without a repository.
//! Everything that genuinely needs real Git — teardown, dirty refusal, conflict
//! preservation, identity retirement across a real recreate — lives in the heavy
//! `tests/e2e_git_worktree_tests.rs` suite instead.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;

use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode, SUPPORTED_COMMANDS};
use crate::web::remote_control_api::executor::{CommandFailure, ExecutionSummary};
use crate::web::remote_control_api::worktrees::{
    map_worktree_error, repository_correlation_id, repository_relative_display, WorktreeKey,
    WorktreeListing, WorktreeOperations, WorktreeRegistry, WorktreeResource, WORKTREE_OPERATIONS,
};
use crate::worktree_ops::service::{
    DirtyState, WorktreeFacts, WorktreeOpError, RECOVERY_LOCAL_OR_TUI,
};

use super::{get, harness, json_body, post_json, send, status_and_json};

const TOKEN: &str = "worktree-token";
const REPO: &str = "/srv/repo";

// ============================================================================
// Fake port
// ============================================================================

/// Records what the API delegated and answers from a scripted listing.
#[derive(Default)]
pub(crate) struct FakeWorktreePort {
    listing: Mutex<Option<WorktreeListing>>,
    creates: Mutex<Vec<String>>,
    deletes: Mutex<Vec<String>>,
    merges: Mutex<Vec<String>>,
    outcome: Mutex<Option<CommandFailure>>,
}

impl FakeWorktreePort {
    pub(crate) fn set_listing(&self, listing: WorktreeListing) {
        *self.listing.lock().unwrap() = Some(listing);
    }

    pub(crate) fn fail_with(&self, failure: CommandFailure) {
        *self.outcome.lock().unwrap() = Some(failure);
    }

    pub(crate) fn creates(&self) -> Vec<String> {
        self.creates.lock().unwrap().clone()
    }

    pub(crate) fn deletes(&self) -> Vec<String> {
        self.deletes.lock().unwrap().clone()
    }

    pub(crate) fn merges(&self) -> Vec<String> {
        self.merges.lock().unwrap().clone()
    }

    fn answer(&self) -> Result<ExecutionSummary, CommandFailure> {
        match self.outcome.lock().unwrap().clone() {
            Some(failure) => Err(failure),
            None => Ok(ExecutionSummary::changed("done")),
        }
    }
}

#[async_trait]
impl WorktreeOperations for FakeWorktreePort {
    async fn list(&self) -> Result<WorktreeListing, CommandFailure> {
        self.listing.lock().unwrap().clone().ok_or_else(|| {
            CommandFailure::new(ErrorCode::LifecycleConflict, "no listing configured")
        })
    }

    async fn create(&self, change_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        self.creates.lock().unwrap().push(change_id.to_string());
        self.answer()
    }

    async fn delete(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        self.deletes.lock().unwrap().push(worktree_id.to_string());
        self.answer()
    }

    async fn merge(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        self.merges.lock().unwrap().push(worktree_id.to_string());
        self.answer()
    }
}

fn facts(path: &str, branch: &str) -> WorktreeFacts {
    let mut facts = WorktreeFacts::new(path, branch);
    facts.identity = format!("gitdir: {path}/.git");
    facts.head = "0123456789abcdef".to_string();
    facts
}

fn listing(entries: &[(&str, WorktreeFacts)]) -> WorktreeListing {
    let repository_id = repository_correlation_id(Path::new(REPO));
    WorktreeListing {
        repository_id: repository_id.clone(),
        worktrees: entries
            .iter()
            .map(|(id, facts)| {
                WorktreeResource::project(
                    (*id).to_string(),
                    repository_id.clone(),
                    Path::new(REPO),
                    facts,
                )
            })
            .collect(),
    }
}

fn command_body(json: &str, revision: u64, key: &str) -> String {
    let trimmed = json
        .strip_suffix('}')
        .expect("command fragments are JSON objects");
    format!(r#"{trimmed},"expected_revision":{revision},"idempotency_key":"{key}"}}"#)
}

// ============================================================================
// Task 1 — opaque process-local identity
// ============================================================================

fn key(path: &str, identity: &str) -> WorktreeKey {
    WorktreeKey {
        path: PathBuf::from(path),
        identity: identity.to_string(),
    }
}

#[test]
fn remote_worktree_ids_are_random_128_bit_hex_and_stable_across_observations() {
    let registry = WorktreeRegistry::new();
    let observed = vec![key("/w/a", "id-a"), key("/w/b", "id-b")];

    let first = registry.sync(&observed);
    assert_eq!(first.len(), 2);
    for id in &first {
        assert_eq!(id.len(), 32, "expected 128 bits of hex, got {id}");
        assert!(id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }
    assert_ne!(
        first[0], first[1],
        "distinct resources must not share an ID"
    );

    assert_eq!(
        registry.sync(&observed),
        first,
        "an unchanged observation must not reallocate identity"
    );
}

#[test]
fn remote_worktree_id_is_retired_when_the_resource_disappears() {
    let registry = WorktreeRegistry::new();
    let id = registry.sync(&[key("/w/a", "id-a")])[0].clone();
    assert!(registry.resolve(&id).is_some());

    registry.sync(&[]);

    assert!(
        registry.resolve(&id).is_none(),
        "a retired ID must not resolve"
    );
    assert!(registry.is_retired(&id));
    assert!(registry.is_empty());
}

#[test]
fn remote_worktree_recreated_path_receives_a_new_identity() {
    let registry = WorktreeRegistry::new();
    let original = registry.sync(&[key("/w/a", "id-a")])[0].clone();

    // Disappear, then come back at the very same path.
    registry.sync(&[]);
    let recreated = registry.sync(&[key("/w/a", "id-a")])[0].clone();

    assert_ne!(original, recreated, "a retired ID must never be reused");
    assert!(registry.is_retired(&original));
    assert_eq!(
        registry.resolve(&recreated).map(|k| k.path),
        Some(PathBuf::from("/w/a"))
    );
}

#[test]
fn remote_worktree_same_path_with_new_git_identity_is_a_new_resource() {
    let registry = WorktreeRegistry::new();
    let original = registry.sync(&[key("/w/a", "id-a")])[0].clone();

    // Torn down and rebuilt between two observations: the path is unchanged but
    // the Git identity is not, so it must not inherit the previous ID.
    let rebuilt = registry.sync(&[key("/w/a", "id-a-rebuilt")])[0].clone();

    assert_ne!(original, rebuilt);
    assert!(registry.is_retired(&original));
}

#[test]
fn remote_worktree_explicit_retirement_survives_a_later_observation() {
    let registry = WorktreeRegistry::new();
    let id = registry.sync(&[key("/w/a", "id-a")])[0].clone();

    registry.retire(&id);
    assert!(registry.resolve(&id).is_none());

    let reobserved = registry.sync(&[key("/w/a", "id-a")])[0].clone();
    assert_ne!(id, reobserved);
}

// ============================================================================
// Task 2 — redacted, non-confidential observations
// ============================================================================

#[test]
fn remote_worktree_repository_id_is_16_hex_and_stable_per_repository() {
    let a = repository_correlation_id(Path::new("/srv/one"));
    let b = repository_correlation_id(Path::new("/srv/one"));
    let c = repository_correlation_id(Path::new("/srv/two"));

    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.len(), 16);
    assert!(a
        .chars()
        .all(|ch| ch.is_ascii_hexdigit() && !ch.is_uppercase()));
    // A correlation value, not a secret: it must not be the identity itself.
    assert!(!a.contains("srv"));
}

#[test]
fn remote_worktree_display_paths_are_repository_relative() {
    assert_eq!(
        repository_relative_display(Path::new("/srv/repo"), Path::new("/srv/repo")),
        "."
    );
    assert_eq!(
        repository_relative_display(Path::new("/srv/repo"), Path::new("/srv/repo/sub/wt")),
        "sub/wt"
    );
    // Managed worktrees usually live outside the repository; escaping is normal.
    assert_eq!(
        repository_relative_display(Path::new("/srv/repo"), Path::new("/srv/workspaces/wt")),
        "../workspaces/wt"
    );
}

#[tokio::test]
async fn remote_worktree_list_never_serializes_the_canonical_root() {
    let harness = harness(Some(TOKEN), &[]);
    let mut main = facts("/srv/repo", "main");
    main.is_main = true;
    harness.worktrees.set_listing(listing(&[
        ("id-main", main),
        ("id-a", facts("/srv/workspaces/change-a", "change-a")),
    ]));

    let (status, body) =
        status_and_json(send(&harness.router, get("/api/v2/worktrees", Some(TOKEN))).await).await;

    assert_eq!(status, StatusCode::OK);
    let raw = serde_json::to_string(&body).unwrap();
    assert!(
        !raw.contains("/srv/repo"),
        "the canonical repository root leaked into the response: {raw}"
    );
    assert_eq!(body["worktrees"][0]["path"], ".");
    assert_eq!(body["worktrees"][1]["path"], "../workspaces/change-a");
    assert_eq!(
        body["repository_id"],
        repository_correlation_id(Path::new(REPO))
    );
}

#[test]
fn remote_worktree_unknown_dirty_serializes_as_null_and_blocks_deletion() {
    let mut unknown = facts("/srv/workspaces/change-a", "change-a");
    unknown.dirty = DirtyState::Unknown;
    let resource = &listing(&[("id-a", unknown)]).worktrees[0];

    let json = serde_json::to_value(resource).unwrap();
    assert_eq!(json["dirty"], serde_json::Value::Null);
    assert_eq!(json["operations"]["deletable"], false);
    assert!(json["operations"]["delete_blocked_reason"]
        .as_str()
        .unwrap()
        .contains("could not be determined"));
}

#[test]
fn remote_worktree_conflict_evidence_names_local_recovery() {
    let mut conflicted = facts("/srv/workspaces/change-a", "change-a");
    conflicted.conflict_files = vec!["src/main.rs".to_string()];
    let resource = &listing(&[("id-a", conflicted)]).worktrees[0];

    let json = serde_json::to_value(resource).unwrap();
    assert_eq!(json["conflict"]["files"][0], "src/main.rs");
    assert_eq!(json["conflict"]["recovery"], RECOVERY_LOCAL_OR_TUI);
}

#[test]
fn remote_worktree_resource_carries_no_absolute_path_or_target_field() {
    let resource =
        &listing(&[("id-a", facts("/srv/workspaces/change-a", "change-a"))]).worktrees[0];
    let json = serde_json::to_value(resource).unwrap();
    let object = json.as_object().unwrap();

    for forbidden in [
        "absolute_path",
        "canonical_path",
        "repository_root",
        "worktree_path",
        "base_commit",
        "worktree_command",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "v2 must not expose '{forbidden}'"
        );
    }
}

#[tokio::test]
async fn remote_worktree_detail_resolves_by_opaque_id_only() {
    let harness = harness(Some(TOKEN), &[]);
    harness.worktrees.set_listing(listing(&[(
        "id-a",
        facts("/srv/workspaces/change-a", "change-a"),
    )]));

    let (status, body) =
        status_and_json(send(&harness.router, get("/api/v2/worktrees/id-a", Some(TOKEN))).await)
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["worktree"]["worktree_id"], "id-a");

    // A branch name, a path, and a retired ID are all equally "not a resource".
    for addressed in ["change-a", "..%2Fworkspaces%2Fchange-a", "retired-id"] {
        let (status, body) = status_and_json(
            send(
                &harness.router,
                get(&format!("/api/v2/worktrees/{addressed}"), Some(TOKEN)),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "addressed by {addressed}");
        assert_eq!(body["error_code"], "worktree_not_found");
    }
}

#[tokio::test]
async fn remote_worktree_reads_require_authentication() {
    let harness = harness(Some(TOKEN), &[]);
    harness.worktrees.set_listing(listing(&[]));

    for uri in ["/api/v2/worktrees", "/api/v2/worktrees/id-a"] {
        let response = send(&harness.router, get(uri, None)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }
}

// ============================================================================
// Task 7 — closed command shapes and negative security coverage
// ============================================================================

#[test]
fn remote_worktree_commands_reject_path_branch_and_base_commit_targets() {
    let smuggled = [
        r#"{"type":"create_worktree","target":{"change_id":"a","base_commit":"deadbeef"},"params":{}}"#,
        r#"{"type":"create_worktree","target":{"change_id":"a","branch":"evil"},"params":{}}"#,
        r#"{"type":"create_worktree","target":{"change_id":"a","path":"/etc"},"params":{}}"#,
        r#"{"type":"create_worktree","target":{"path":"/etc"},"params":{}}"#,
        r#"{"type":"delete_worktree","target":{"path":"/srv/workspaces/a"},"params":{}}"#,
        r#"{"type":"delete_worktree","target":{"worktree_id":"a","skip_teardown":true},"params":{}}"#,
        r#"{"type":"delete_worktree","target":{"worktree_id":"a"},"params":{"skip_teardown":true}}"#,
        r#"{"type":"delete_worktree","target":{"worktree_id":"a"},"params":{"force":true}}"#,
        r#"{"type":"merge_worktree","target":{"branch":"a"},"params":{}}"#,
        r#"{"type":"merge_worktree","target":{"worktree_id":"a"},"params":{"abort":true}}"#,
        r#"{"type":"merge_worktree","target":{"worktree_id":"a"},"params":{"strategy":"ours"}}"#,
        r#"{"type":"merge_worktree","target":{"repository_id":"abc"},"params":{}}"#,
    ];

    for body in smuggled {
        assert!(
            serde_json::from_str::<CommandSpec>(body).is_err(),
            "must fail typed validation: {body}"
        );
    }
}

#[test]
fn remote_worktree_command_surface_has_no_generic_or_editor_operation() {
    for absent in [
        "worktree_command",
        "run_in_worktree",
        "open_editor",
        "open_worktree",
        "create_session",
        "resolve_worktree_conflict",
        "abort_worktree_merge",
        "force_delete_worktree",
    ] {
        assert!(
            !SUPPORTED_COMMANDS.contains(&absent),
            "v2 must not advertise '{absent}'"
        );
        assert!(
            serde_json::from_str::<CommandSpec>(&format!(r#"{{"type":"{absent}"}}"#)).is_err(),
            "v2 must not parse '{absent}'"
        );
    }
}

#[tokio::test]
async fn remote_worktree_unauthenticated_mutation_never_reaches_the_service() {
    let harness = harness(Some(TOKEN), &[]);
    let body = command_body(
        r#"{"type":"delete_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
        0,
        "k1",
    );

    let response = send(&harness.router, post_json("/api/v2/commands", None, &body)).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(harness.worktrees.deletes().is_empty());
}

#[tokio::test]
async fn remote_worktree_unknown_parameter_is_a_422_with_no_delegation() {
    let harness = harness(Some(TOKEN), &[]);
    let body = command_body(
        r#"{"type":"delete_worktree","target":{"worktree_id":"id-a"},"params":{"skip_teardown":true}}"#,
        0,
        "k1",
    );

    let (status, json) = status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error_code"], "validation_failed");
    assert!(harness.worktrees.deletes().is_empty());
}

// ============================================================================
// Tasks 4-6 — delegation and inherited v2 guards
// ============================================================================

#[tokio::test]
async fn remote_worktree_create_delegates_only_the_change_id() {
    let harness = harness(Some(TOKEN), &[]);
    let body = command_body(
        r#"{"type":"create_worktree","target":{"change_id":"change-a"},"params":{}}"#,
        0,
        "k1",
    );

    let (status, json) = status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["type"], "create_worktree");
    assert_eq!(harness.worktrees.creates(), vec!["change-a".to_string()]);
}

#[tokio::test]
async fn remote_worktree_delete_and_merge_delegate_only_the_opaque_id() {
    let harness = harness(Some(TOKEN), &[]);

    for (index, (json, key)) in [
        (
            r#"{"type":"delete_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
            "k-delete",
        ),
        (
            r#"{"type":"merge_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
            "k-merge",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let body = command_body(json, index as u64, key);
        let response = send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        // Each accepted command advances the revision the next one must quote.
        harness.projection.apply_state(
            "worktree_changed",
            None,
            serde_json::json!({}),
            super::snapshot_with(&format!("c{index}"), "queued"),
        );
    }

    assert_eq!(harness.worktrees.deletes(), vec!["id-a".to_string()]);
    assert_eq!(harness.worktrees.merges(), vec!["id-a".to_string()]);
}

#[tokio::test]
async fn remote_worktree_commands_require_revision_and_idempotency_key() {
    let harness = harness(Some(TOKEN), &[]);

    for incomplete in [
        r#"{"type":"create_worktree","target":{"change_id":"a"},"params":{},"idempotency_key":"k"}"#,
        r#"{"type":"create_worktree","target":{"change_id":"a"},"params":{},"expected_revision":0}"#,
    ] {
        let (status, _) = status_and_json(
            send(
                &harness.router,
                post_json("/api/v2/commands", Some(TOKEN), incomplete),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{incomplete}");
    }
    assert!(harness.worktrees.creates().is_empty());
}

#[tokio::test]
async fn remote_worktree_stale_revision_is_refused_before_delegation() {
    let harness = harness(Some(TOKEN), &[]);
    harness.projection.apply_state(
        "moved_on",
        None,
        serde_json::json!({}),
        super::snapshot_with("c1", "queued"),
    );

    let body = command_body(
        r#"{"type":"merge_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
        0,
        "k1",
    );
    let (status, json) = status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error_code"], "stale_revision");
    assert!(harness.worktrees.merges().is_empty());
}

#[tokio::test]
async fn remote_worktree_replayed_delete_runs_its_side_effect_once() {
    let harness = harness(Some(TOKEN), &[]);
    let body = command_body(
        r#"{"type":"delete_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
        0,
        "same-key",
    );

    let first = send(
        &harness.router,
        post_json("/api/v2/commands", Some(TOKEN), &body),
    )
    .await;
    let second = send(
        &harness.router,
        post_json("/api/v2/commands", Some(TOKEN), &body),
    )
    .await;

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(
        harness.worktrees.deletes(),
        vec!["id-a".to_string()],
        "an exact replay must not delete twice"
    );
}

#[tokio::test]
async fn remote_worktree_service_refusals_surface_as_typed_v2_errors() {
    for (error, expected_code, expected_status) in [
        (
            WorktreeOpError::Exists("exists".to_string()),
            "worktree_exists",
            StatusCode::CONFLICT,
        ),
        (
            WorktreeOpError::NotFound("gone".to_string()),
            "worktree_not_found",
            StatusCode::NOT_FOUND,
        ),
        (
            WorktreeOpError::Dirty("dirty".to_string()),
            "worktree_dirty",
            StatusCode::CONFLICT,
        ),
        (
            WorktreeOpError::DirtyUnknown("unknown".to_string()),
            "worktree_dirty_unknown",
            StatusCode::CONFLICT,
        ),
        (
            WorktreeOpError::RootBusy("busy".to_string()),
            "root_busy",
            StatusCode::CONFLICT,
        ),
        (
            WorktreeOpError::MergeConflict {
                files: vec!["src/main.rs".to_string()],
                recovery: RECOVERY_LOCAL_OR_TUI,
            },
            "merge_conflict",
            StatusCode::CONFLICT,
        ),
    ] {
        let harness = harness(Some(TOKEN), &[]);
        harness.worktrees.fail_with(map_worktree_error(&error));

        let body = command_body(
            r#"{"type":"merge_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
            0,
            "k1",
        );
        let (status, json) = status_and_json(
            send(
                &harness.router,
                post_json("/api/v2/commands", Some(TOKEN), &body),
            )
            .await,
        )
        .await;

        assert_eq!(status, expected_status, "{expected_code}");
        assert_eq!(json["state"], "failed");
        assert_eq!(json["error_code"], expected_code);
    }
}

#[tokio::test]
async fn remote_worktree_merge_conflict_detail_names_files_and_local_recovery() {
    let harness = harness(Some(TOKEN), &[]);
    harness
        .worktrees
        .fail_with(map_worktree_error(&WorktreeOpError::MergeConflict {
            files: vec!["src/main.rs".to_string(), "README.md".to_string()],
            recovery: RECOVERY_LOCAL_OR_TUI,
        }));

    let body = command_body(
        r#"{"type":"merge_worktree","target":{"worktree_id":"id-a"},"params":{}}"#,
        0,
        "k1",
    );
    let (_, json) = status_and_json(
        send(
            &harness.router,
            post_json("/api/v2/commands", Some(TOKEN), &body),
        )
        .await,
    )
    .await;

    let detail = json["detail"]
        .as_str()
        .expect("failed commands carry detail");
    assert!(detail.contains("src/main.rs"));
    assert!(detail.contains("README.md"));
    assert!(detail.contains(RECOVERY_LOCAL_OR_TUI));
    assert!(detail.contains("preserved"));
}

#[tokio::test]
async fn remote_worktree_commands_are_refused_before_a_runtime_is_bound() {
    // The default port is the unbound one; nothing may be reported as accepted.
    let harness = harness(Some(TOKEN), &[]);
    let unbound = crate::web::remote_control_api::worktrees::UnboundWorktreeOperations;
    let failure = unbound.delete("id-a").await.expect_err("must refuse");
    assert_eq!(failure.error_code, ErrorCode::LifecycleConflict);

    let response = send(&harness.router, get("/api/v2/worktrees", Some(TOKEN))).await;
    // The harness port has no listing configured, which stands in for "unbound".
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

// ============================================================================
// Capabilities
// ============================================================================

#[tokio::test]
async fn remote_worktree_capabilities_report_the_operations_and_recovery_boundary() {
    let harness = harness(Some(TOKEN), &[]);
    let body =
        json_body(send(&harness.router, get("/api/v2/capabilities", Some(TOKEN))).await).await;

    let operations: Vec<String> =
        serde_json::from_value(body["worktrees"]["operations"].clone()).unwrap();
    assert_eq!(operations, WORKTREE_OPERATIONS.to_vec());
    assert_eq!(
        body["worktrees"]["merge_conflict_recovery"],
        RECOVERY_LOCAL_OR_TUI
    );
    assert_eq!(body["worktrees"]["merge_conflict_preserves_state"], true);
    assert_eq!(body["worktrees"]["delete_requires_teardown"], true);

    let commands: Vec<String> = serde_json::from_value(body["commands"].clone()).unwrap();
    for expected in ["create_worktree", "delete_worktree", "merge_worktree"] {
        assert!(commands.contains(&expected.to_string()), "{expected}");
    }

    let error_codes: Vec<String> = serde_json::from_value(body["error_codes"].clone()).unwrap();
    for expected in [
        "worktree_exists",
        "worktree_not_found",
        "worktree_dirty",
        "worktree_dirty_unknown",
        "merge_conflict",
    ] {
        assert!(error_codes.contains(&expected.to_string()), "{expected}");
    }
}

// ============================================================================
// Task 3 — adapter parity
// ============================================================================

#[tokio::test]
async fn remote_worktree_adapters_share_one_service_implementation() {
    use crate::worktree_ops::service::{
        ConflictPolicy, DeleteOptions, MergeAttempt, WorktreeBackend, WorktreeEventSink,
        WorktreeOpResult, WorktreeOperationEvent, WorktreeService,
    };

    /// One backend, driven twice: once with the TUI's policy values and once
    /// with the remote ones. Anything a frontend re-implemented instead of
    /// delegating would be missing from this recording.
    #[derive(Default)]
    struct ParityBackend {
        calls: Mutex<Vec<String>>,
        merged: std::sync::atomic::AtomicBool,
    }

    #[async_trait]
    impl WorktreeBackend for ParityBackend {
        async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
            let mut wt = WorktreeFacts::new("/srv/workspaces/change-a", "change-a");
            // Merging is what makes the worktree deletable, so the observation
            // has to move with it or the second half of the run is untestable.
            wt.has_commits_ahead = !self.merged.load(std::sync::atomic::Ordering::SeqCst);
            Ok(vec![wt])
        }
        async fn base_head(&self) -> WorktreeOpResult<String> {
            Ok("head".to_string())
        }
        async fn create(&self, _p: &Path, _b: &str, _c: &str) -> WorktreeOpResult<()> {
            Ok(())
        }
        async fn remove(&self, _path: &Path, skip_teardown: bool) -> WorktreeOpResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("remove(skip_teardown={skip_teardown})"));
            Ok(())
        }
        async fn delete_branch(&self, _branch: &str) -> WorktreeOpResult<()> {
            self.calls.lock().unwrap().push("delete_branch".to_string());
            Ok(())
        }
        async fn merge_into_base(
            &self,
            _branch: &str,
            policy: ConflictPolicy,
        ) -> WorktreeOpResult<MergeAttempt> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("merge({policy:?})"));
            self.merged.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(MergeAttempt::Merged)
        }
        async fn run_on_merged(&self, _c: &str, _p: &Path) -> WorktreeOpResult<()> {
            self.calls.lock().unwrap().push("on_merged".to_string());
            Ok(())
        }
        async fn change_is_eligible(&self, _change_id: &str) -> WorktreeOpResult<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct Sink(Mutex<Vec<String>>);

    #[async_trait]
    impl WorktreeEventSink for Sink {
        async fn emit(&self, event: WorktreeOperationEvent) {
            self.0.lock().unwrap().push(format!("{event:?}"));
        }
    }

    async fn run(options: DeleteOptions, policy: ConflictPolicy) -> (Vec<String>, Vec<String>) {
        let backend = Arc::new(ParityBackend::default());
        let sink = Arc::new(Sink::default());
        let service = WorktreeService::new(
            backend.clone(),
            sink.clone(),
            PathBuf::from("/srv/workspaces"),
        );
        let path = Path::new("/srv/workspaces/change-a");
        service.merge_worktree(path, policy).await.expect("merged");
        service
            .delete_worktree(path, options)
            .await
            .expect("deleted");
        let calls = backend.calls.lock().unwrap().clone();
        let events = sink.0.lock().unwrap().clone();
        (calls, events)
    }

    let (tui_calls, tui_events) =
        run(DeleteOptions::local(false), ConflictPolicy::AbortOnConflict).await;
    let (remote_calls, remote_events) = run(
        DeleteOptions::fail_closed(),
        ConflictPolicy::PreserveConflict,
    )
    .await;

    // Same operation sequence, same event sequence: the only difference between
    // the two frontends is the declared conflict policy.
    assert_eq!(
        tui_calls,
        vec![
            "merge(AbortOnConflict)".to_string(),
            "on_merged".to_string(),
            "remove(skip_teardown=false)".to_string(),
            "delete_branch".to_string(),
        ]
    );
    assert_eq!(
        remote_calls,
        vec![
            "merge(PreserveConflict)".to_string(),
            "on_merged".to_string(),
            "remove(skip_teardown=false)".to_string(),
            "delete_branch".to_string(),
        ]
    );
    assert_eq!(tui_events, remote_events);
}

// ============================================================================
// Published schema shape
// ============================================================================

#[test]
fn remote_worktree_published_schemas_expose_no_path_branch_or_base_commit_input() {
    use crate::web::remote_control_api::dto::{ChangeTarget, EmptyParams, WorktreeTarget};
    use utoipa::PartialSchema;

    let change_target = serde_json::to_value(ChangeTarget::schema()).unwrap();
    let worktree_target = serde_json::to_value(WorktreeTarget::schema()).unwrap();
    let params = serde_json::to_value(EmptyParams::schema()).unwrap();

    assert_eq!(
        change_target["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["change_id"]
    );
    assert_eq!(
        worktree_target["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["worktree_id"]
    );
    assert!(
        params["properties"]
            .as_object()
            .map(|properties| properties.is_empty())
            .unwrap_or(true),
        "the published parameter object must accept nothing: {params}"
    );

    // The generated contract is what a client codegens against, so the absence
    // has to hold there and not only in the deserializer. Only the property
    // names are inspected: descriptions deliberately *name* the fields that are
    // refused, and matching those would assert the opposite of the intent.
    let published: Vec<String> = [&change_target, &worktree_target, &params]
        .iter()
        .filter_map(|schema| schema["properties"].as_object())
        .flat_map(|properties| properties.keys().cloned())
        .collect();
    for forbidden in [
        "base_commit",
        "path",
        "branch",
        "skip_teardown",
        "force",
        "command",
        "repository_id",
    ] {
        assert!(
            !published.iter().any(|name| name == forbidden),
            "v2 worktree command schemas must not publish '{forbidden}': {published:?}"
        );
    }

    // Closed by construction: an unknown property is a schema violation.
    for schema in [&change_target, &worktree_target, &params] {
        assert_eq!(
            schema["additionalProperties"],
            serde_json::Value::Bool(false)
        );
    }
}
