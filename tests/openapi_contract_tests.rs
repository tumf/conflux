//! Contract assertions against the generated `/api/v2` OpenAPI document.
//!
//! This repository tracks no OpenAPI file. The contract exists in exactly two
//! places a consumer can reach — `cflx openapi` and `GET /api/v2/openapi.yaml` —
//! and both are the same function, so there is no artifact to keep in sync and
//! no drift check to run. What replaces the drift check is *completeness*:
//!
//! * **The document is complete.** Every supported route, command variant, error
//!   code, event category, security declaration, and required published field is
//!   present — and nothing removed has crept back in. A document that generates
//!   cleanly and still says too little fails here.
//! * **The document is the one consumers get.** The bytes `cflx openapi` writes
//!   and the bytes the router serves are compared directly, so an export path
//!   cannot quietly diverge from the live one.
//! * **The completeness check would notice.** Positive assertions can never
//!   establish that about themselves, so the same predicates are also run against
//!   deliberately incomplete copies of the document, which must fail.
//!
//! The payload checks are the "generated consumer compiles" property expressed in
//! Rust: the DTO values below are constructed field by field from the real types,
//! so adding a field to a published response forces this file to change, and the
//! structural validator then proves the published schema describes what those
//! types actually serialize to.
//!
//! ```text
//! cargo test --features web-monitoring --test openapi_contract_tests
//! ```

#![cfg(feature = "web-monitoring")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use conflux::web::openapi::{
    document_yaml, BEARER_SCHEME, GENERATED_BANNER, SUPPORTED_V2_PATHS, UNAUTHENTICATED_V2_PATHS,
};
use conflux::web::remote_control_api::dto::{
    ActionEligibility, ApiError, ApplyCommitEvidence, AttentionState, ChangeActions,
    ChangeActivity, ChangeBlocker, ChangeExecutionState, ChangeExecutionStatus, ChangeResource,
    ChangeTiming, ChangeWorktree, CommandRecord, CommandResult, CommandState, ErrorCode,
    EventCategory, EventEnvelope, ExecutionPhase, ExecutionStatusResponse, InstanceSnapshot,
    LatestLogProjection, ParallelEligibility, ParallelRuntimeState, ProcessExecutionStatus,
    QueueIntent, SnapshotTotals, StateResponse, ALL_CHANGE_EXECUTION_STATES, ALL_ERROR_CODES,
    ALL_EXECUTION_PHASES, ALL_TERMINAL_MODES, SUPPORTED_COMMANDS,
};
use conflux::web::remote_control_api::worktrees::{
    WorktreeConflict, WorktreeEligibility, WorktreeResource, WorktreeResponse,
};

/// Paths this repository removed. None of them may reappear as supported API.
///
/// The unversioned `/api/*` surface and `/ws` went away with the legacy
/// single-instance server, and `/api/v1/*` went away with multi-project server
/// mode. A generated client that still saw them would emit calls that 404.
const REMOVED_PATHS: &[&str] = &[
    "/api/health",
    "/api/state",
    "/api/changes",
    "/api/changes/{id}",
    "/api/control/start",
    "/api/control/stop",
    "/api/control/cancel-stop",
    "/api/control/force-stop",
    "/api/control/retry",
    "/api/worktrees",
    "/api/worktrees/create",
    "/api/worktrees/delete",
    "/api/worktrees/merge",
    "/api/worktrees/refresh",
    "/api/worktrees/command",
    "/ws",
    "/api/v1/projects",
    "/api/v1/logs",
    "/api/v1/stats/overview",
    "/api/v1/stats/projects/{id}/history",
];

/// Fields a client cannot work without, per published schema.
///
/// These are the members a consumer branches on rather than merely reads:
/// stream ordering, optimistic-concurrency revisions, command identity, and
/// error dispatch. Publishing them as optional would compile a client that
/// silently tolerates their absence, which is exactly the failure a schema is
/// supposed to prevent.
const REQUIRED_MEMBERS: &[(&str, &[&str])] = &[
    (
        "StateResponse",
        &[
            "instance_id",
            "state_revision",
            "event_sequence",
            "snapshot",
        ],
    ),
    (
        "EventEnvelope",
        &[
            "instance_id",
            "event_sequence",
            "state_revision",
            "category",
            "event_type",
            "timestamp",
        ],
    ),
    (
        "CommandRecord",
        &[
            "command_id",
            "instance_id",
            // `command_type` on the wire; the discriminant a client dispatches on.
            "type",
            "state",
            "expected_revision",
            "correlation_id",
            "idempotency_key",
        ],
    ),
    ("ApiError", &["error_code", "message", "correlation_id"]),
    (
        "WorktreeResponse",
        &["instance_id", "state_revision", "worktree"],
    ),
    // The execution observation. `observed_at` is required because it is what
    // a client renders relative time against; without it, a consumer falls back
    // to its own clock and the whole absolute-time contract is pointless.
    (
        "ExecutionStatusResponse",
        &[
            "instance_id",
            "state_revision",
            "event_sequence",
            "observed_at",
            "process",
            "changes",
        ],
    ),
    (
        "ProcessExecutionStatus",
        &["app_mode", "scheduler_running", "has_active_work"],
    ),
    (
        "ChangeExecutionStatus",
        &["id", "execution_state", "current_phase"],
    ),
    ("LatestLogProjection", &["message", "level", "created_at"]),
];

/// The execution-status resource publishes absolute instants and nothing else.
///
/// A client that found an elapsed counter here would render a value that is
/// already stale, and a server that published one would have to advance its own
/// revision forever to keep it fresh.
const FORBIDDEN_TIME_MEMBERS: &[&str] = &[
    "elapsed_seconds",
    "elapsed_ms",
    "age_seconds",
    "seconds_ago",
    "relative_time",
];

/// Snapshot members an operator console decides from.
///
/// A silently dropped member is the drift most likely to go unnoticed: the
/// payload still validates, it just says less.
const REQUIRED_CHANGE_FIELDS: &[&str] = &[
    "queue_intent",
    "attention",
    "actions",
    "parallel",
    "timing",
    "blocker",
    "latest_activity",
    "worktree",
    "execution_marked",
    "error_detail",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The generated contract, parsed as JSON — which is what a consumer works with.
fn generated() -> Value {
    serde_yaml::from_str(&document_yaml()).expect("the generated document must be valid YAML")
}

fn schemas(doc: &Value) -> &Value {
    &doc["components"]["schemas"]
}

// ============================================================================
// The document consumers actually get
// ============================================================================

#[test]
fn generation_is_byte_for_byte_deterministic() {
    assert_eq!(
        document_yaml(),
        document_yaml(),
        "OpenAPI generation must not depend on iteration or hashing order"
    );
}

/// A copy of a generated document outlives the build that produced it, so the
/// document has to carry its own provenance and its own export instructions —
/// naming the two ways to regenerate it and nothing that no longer exists.
#[test]
fn the_document_carries_current_export_instructions() {
    let text = document_yaml();
    assert!(
        text.starts_with(GENERATED_BANNER),
        "the generated document must open with the generated-document banner"
    );
    assert!(
        GENERATED_BANNER.contains("cflx openapi"),
        "the banner must name the CLI export path"
    );
    assert!(
        GENERATED_BANNER.contains("/api/v2/openapi.yaml"),
        "the banner must name the live endpoint"
    );
    for removed in ["docs/openapi.yaml", "make openapi", "make check-openapi"] {
        assert!(
            !text.contains(removed),
            "the generated document still points at `{removed}`, which no longer exists"
        );
    }
}

/// The export path and the live path must be one document, byte for byte.
///
/// `cflx openapi` is the offline half of the contract surface. Comparing the
/// process's stdout against the router's response body is the only check that
/// covers both halves end to end: a CLI layer that added a preamble, trimmed a
/// newline, or served a stale document would pass every structural assertion in
/// this file and still hand a consumer different bytes than the server does.
#[tokio::test]
async fn the_cli_export_and_the_live_endpoint_serve_identical_bytes() {
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    let response = protected_router()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v2/openapi.yaml")
                .header("host", "127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("the contract route answers without credentials");
    let served = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the contract body is readable");

    let exported = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .arg("openapi")
        // Outside the repository, so the export cannot be reading anything local.
        .current_dir(std::env::temp_dir())
        .output()
        .expect("the cflx binary under test is runnable");
    assert!(
        exported.status.success(),
        "cflx openapi failed: {}",
        String::from_utf8_lossy(&exported.stderr)
    );

    assert_eq!(
        String::from_utf8(exported.stdout).expect("the export is UTF-8"),
        String::from_utf8(served.to_vec()).expect("the served body is UTF-8"),
        "cflx openapi and GET /api/v2/openapi.yaml must serve the same document"
    );
}

/// A tracked schema file is the failure mode this change removed: it is a second
/// representation that cannot be kept honest, and a stale one is worse than none
/// because a generated consumer will believe it.
#[test]
fn the_repository_tracks_no_openapi_artifact() {
    let mut found = Vec::new();
    collect_openapi_files(&repo_root(), 0, &mut found);
    assert!(
        found.is_empty(),
        "the contract is generated, not tracked; these files duplicate it: {found:?}"
    );
}

/// Walk the working tree for OpenAPI-looking files, skipping build and vendor
/// output that is not part of the repository's own content.
fn collect_openapi_files(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    const SKIP: &[&str] = &["target", "node_modules", ".git", ".codegraph", ".beads"];
    if depth > 4 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                collect_openapi_files(&path, depth + 1, found);
            }
        } else if name.starts_with("openapi.")
            && (name.ends_with(".yaml") || name.ends_with(".json"))
        {
            found.push(path);
        }
    }
    found.sort();
}

// ============================================================================
// Completeness predicates
//
// Each returns the assertion failures it found rather than panicking, so the
// same predicate can be run against the real document (which must produce none)
// and against a deliberately incomplete copy (which must produce some).
// ============================================================================

/// Every completeness predicate, run over one document.
fn contract_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    errors.extend(route_surface_errors(doc));
    errors.extend(removed_path_errors(doc));
    errors.extend(security_errors(doc));
    errors.extend(command_vocabulary_errors(doc));
    errors.extend(error_code_errors(doc));
    errors.extend(event_vocabulary_errors(doc));
    errors.extend(reference_errors(doc));
    errors.extend(required_member_errors(doc));
    errors.extend(payload_errors(doc));
    errors.extend(snapshot_field_errors(doc));
    errors.extend(execution_observability_errors(doc));
    errors
}

fn published_paths(doc: &Value) -> BTreeSet<String> {
    doc["paths"]
        .as_object()
        .map(|paths| paths.keys().cloned().collect())
        .unwrap_or_default()
}

fn route_surface_errors(doc: &Value) -> Vec<String> {
    let published = published_paths(doc);
    let supported: BTreeSet<String> = SUPPORTED_V2_PATHS.iter().map(|p| p.to_string()).collect();
    let mut errors = Vec::new();
    for missing in supported.difference(&published) {
        errors.push(format!(
            "{missing} is a supported route but is not published"
        ));
    }
    for extra in published.difference(&supported) {
        errors.push(format!(
            "{extra} is published but is not a supported v2 route"
        ));
    }
    errors
}

fn removed_path_errors(doc: &Value) -> Vec<String> {
    let published = published_paths(doc);
    let mut errors = Vec::new();
    for removed in REMOVED_PATHS {
        if published.contains(*removed) {
            errors.push(format!(
                "{removed} was removed from this server but is still published as supported API"
            ));
        }
    }
    for path in &published {
        if !path.starts_with("/api/v2/") {
            errors.push(format!(
                "{path} is outside the only namespace this process serves"
            ));
        }
    }
    errors
}

fn security_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let scheme = &doc["components"]["securitySchemes"][BEARER_SCHEME];
    if scheme["type"] != "http" || scheme["scheme"] != "bearer" {
        errors.push(format!("{BEARER_SCHEME} is not declared as HTTP bearer"));
    }
    if !scheme["description"]
        .as_str()
        .unwrap_or_default()
        .contains("Authorization")
    {
        errors.push("the bearer scheme must name the header credentials are accepted in".into());
    }

    match doc["security"].as_array() {
        Some(global) if global.iter().any(|req| req.get(BEARER_SCHEME).is_some()) => {}
        _ => errors.push(format!("the document default must require {BEARER_SCHEME}")),
    }

    let empty = serde_json::Map::new();
    for (path, item) in doc["paths"].as_object().unwrap_or(&empty) {
        for (method, operation) in item.as_object().unwrap_or(&empty) {
            let declared = operation.get("security").and_then(Value::as_array);
            if UNAUTHENTICATED_V2_PATHS.contains(&path.as_str()) {
                if declared.map(Vec::len) != Some(0) {
                    errors.push(format!(
                        "{method} {path} is unauthenticated and must override the default with `security: []`"
                    ));
                }
            } else if declared.is_some() {
                errors.push(format!(
                    "{method} {path} must inherit the document-wide bearer requirement"
                ));
            }
        }
    }
    errors
}

fn published_enum(doc: &Value, schema: &str) -> BTreeSet<String> {
    schemas(doc)[schema]["enum"]
        .as_array()
        .map(|members| {
            members
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn command_vocabulary_errors(doc: &Value) -> Vec<String> {
    let Some(variants) = schemas(doc)["CommandSpec"]["oneOf"].as_array() else {
        return vec!["CommandSpec must be a discriminated union".into()];
    };

    let mut errors = Vec::new();
    let mut published = BTreeSet::new();
    for variant in variants {
        match variant["properties"]["type"]["enum"][0].as_str() {
            Some(discriminant) => {
                published.insert(discriminant.to_string());
            }
            None => errors.push(format!(
                "a command variant does not pin its `type`: {variant}"
            )),
        }
        let required: Vec<&str> = variant["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !required.contains(&"type") {
            errors.push(format!(
                "a command variant that does not require `type` cannot be dispatched: {variant}"
            ));
        }
    }

    let supported: BTreeSet<String> = SUPPORTED_COMMANDS.iter().map(|c| c.to_string()).collect();
    for missing in supported.difference(&published) {
        errors.push(format!(
            "the command `{missing}` is accepted but is not published"
        ));
    }
    for extra in published.difference(&supported) {
        errors.push(format!(
            "the command `{extra}` is published but is not accepted"
        ));
    }
    errors
}

fn error_code_errors(doc: &Value) -> Vec<String> {
    let published = published_enum(doc, "ErrorCode");
    let known: BTreeSet<String> = ALL_ERROR_CODES
        .iter()
        .map(|c| c.as_str().to_string())
        .collect();

    let mut errors = Vec::new();
    for missing in known.difference(&published) {
        errors.push(format!(
            "the error code `{missing}` can be emitted but is not published"
        ));
    }
    for extra in published.difference(&known) {
        errors.push(format!(
            "the error code `{extra}` is published but is never emitted"
        ));
    }
    errors
}

fn event_vocabulary_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let states = published_enum(doc, "CommandState");
    let known_states = serialized_names(&[
        CommandState::Running,
        CommandState::Succeeded,
        CommandState::NoOp,
        CommandState::Failed,
    ]);
    for missing in known_states.difference(&states) {
        errors.push(format!(
            "the command outcome `{missing}` is reachable but is not published"
        ));
    }

    let categories = published_enum(doc, "EventCategory");
    let known_categories =
        serialized_names(&[EventCategory::State, EventCategory::Log, EventCategory::Gap]);
    for missing in known_categories.difference(&categories) {
        errors.push(format!(
            "the event category `{missing}` is emitted but is not published"
        ));
    }
    if !categories.contains("gap") {
        errors.push("the replay-gap category is what tells a client to resnapshot".into());
    }
    errors
}

fn serialized_names<T: serde::Serialize>(values: &[T]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value)
                .expect("closed vocabularies serialize")
                .as_str()
                .expect("closed vocabularies serialize as strings")
                .to_string()
        })
        .collect()
}

fn reference_errors(doc: &Value) -> Vec<String> {
    let defined: BTreeSet<String> = schemas(doc)
        .as_object()
        .map(|s| s.keys().cloned().collect())
        .unwrap_or_default();

    let mut referenced = BTreeSet::new();
    collect_refs(doc, &mut referenced);

    let mut errors = Vec::new();
    for dangling in referenced.difference(&defined) {
        errors.push(format!(
            "the schema `{dangling}` is referenced but never defined"
        ));
    }
    for orphan in defined.difference(&referenced) {
        errors.push(format!(
            "the schema `{orphan}` is defined but unreachable from any operation"
        ));
    }
    errors
}

fn collect_refs(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" {
                    if let Some(name) = child
                        .as_str()
                        .and_then(|r| r.strip_prefix("#/components/schemas/"))
                    {
                        into.insert(name.to_string());
                    }
                } else {
                    collect_refs(child, into);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, into)),
        _ => {}
    }
}

fn required_member_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    for (schema, members) in REQUIRED_MEMBERS {
        let required: BTreeSet<&str> = schemas(doc)[*schema]["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for member in *members {
            if !required.contains(member) {
                errors.push(format!(
                    "{schema}.{member} must be published as required; a client branches on it"
                ));
            }
        }
    }
    errors
}

fn payload_errors(doc: &Value) -> Vec<String> {
    // Each pair is a schema name and a value produced by the real DTO types.
    // Constructing them field by field is the compile-time half of the check:
    // a new published field cannot land without this file being updated.
    let cases: Vec<(&str, Value)> = vec![
        ("StateResponse", to_value(&state_response())),
        ("CommandRecord", to_value(&command_record())),
        ("CommandRecord", to_value(&settled_stop_record())),
        ("WorktreeResponse", to_value(&worktree_response())),
        ("EventEnvelope", to_value(&event_envelope())),
        ("ApiError", to_value(&api_error())),
        ("ExecutionStatusResponse", to_value(&execution_status())),
    ];

    let mut errors = Vec::new();
    for (name, payload) in cases {
        let schema = &schemas(doc)[name];
        if schema.is_null() {
            errors.push(format!("{name} is served by this API but is not published"));
            continue;
        }
        validate(&payload, schema, schemas(doc), name, &mut errors);
    }
    errors
}

fn snapshot_field_errors(doc: &Value) -> Vec<String> {
    let payload = to_value(&state_response());
    let change = &payload["snapshot"]["changes"][0];

    let published: BTreeSet<&str> = schemas(doc)["ChangeResource"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let serialized: BTreeSet<&str> = change
        .as_object()
        .expect("serialized change")
        .keys()
        .map(String::as_str)
        .collect();

    let mut errors = Vec::new();
    for unpublished in serialized.difference(&published) {
        errors.push(format!(
            "the server serializes the change field `{unpublished}` but the contract omits it"
        ));
    }
    for expected in REQUIRED_CHANGE_FIELDS {
        if !published.contains(expected) {
            errors.push(format!(
                "the authoritative snapshot must publish `{expected}`"
            ));
        }
    }
    errors
}

/// The execution-observability surface a remote agent depends on.
///
/// Every assertion here exists because an agent would otherwise have to infer
/// the answer: closed vocabularies instead of display strings, a typed stop
/// result instead of generic success, absolute instants instead of counters, and
/// a log projection that carries no filesystem locator.
fn execution_observability_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let expected_phases: BTreeSet<String> = ALL_EXECUTION_PHASES
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    if published_enum(doc, "ExecutionPhase") != expected_phases {
        errors.push(
            "ExecutionPhase must publish exactly the closed phase vocabulary the server \
             projects"
                .to_string(),
        );
    }

    let expected_states: BTreeSet<String> = ALL_CHANGE_EXECUTION_STATES
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    if published_enum(doc, "ChangeExecutionState") != expected_states {
        errors.push(
            "ChangeExecutionState must publish exactly the closed execution-state vocabulary"
                .to_string(),
        );
    }

    // The typed command result: a machine consumer must be able to branch on it
    // without parsing the detail sentence.
    let stop_variant = schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants.iter().find(|variant| {
                variant["properties"]["kind"]["enum"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| value == "stop_and_dequeue"))
            })
        })
        .cloned();
    match stop_variant {
        None => errors
            .push("CommandResult must publish the stop_and_dequeue settlement variant".to_string()),
        Some(variant) => {
            let properties: BTreeSet<&str> = variant["properties"]
                .as_object()
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            for member in [
                "cancelled_phase",
                "last_completed_phase",
                "apply_commit",
                "effects_rolled_back",
            ] {
                if !properties.contains(member) {
                    errors.push(format!(
                        "the stop_and_dequeue result must publish `{member}`; a client would \
                         otherwise have to inspect Git itself"
                    ));
                }
            }
        }
    }

    // The latest-log projection is closed and path-free. `workspace_path` and the
    // display `timestamp` exist on the retained entry and must not travel here.
    let log_properties: BTreeSet<&str> = schemas(doc)["LatestLogProjection"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_log: BTreeSet<&str> = ["message", "level", "operation", "iteration", "created_at"]
        .into_iter()
        .collect();
    if log_properties != expected_log {
        errors.push(format!(
            "LatestLogProjection must publish exactly {expected_log:?}, found {log_properties:?}"
        ));
    }

    // No published execution schema may carry a relative-time member.
    for schema in [
        "ExecutionStatusResponse",
        "ProcessExecutionStatus",
        "ChangeExecutionStatus",
        "LatestLogProjection",
    ] {
        let properties: BTreeSet<&str> = schemas(doc)[schema]["properties"]
            .as_object()
            .map(|p| p.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if properties.is_empty() {
            errors.push(format!(
                "{schema} is served by this API but is not published"
            ));
            continue;
        }
        for forbidden in FORBIDDEN_TIME_MEMBERS {
            if properties.contains(forbidden) {
                errors.push(format!(
                    "{schema}.{forbidden} is a relative-time member; this resource publishes \
                     absolute UTC instants only"
                ));
            }
        }
    }

    errors
}

/// The owner execution contract: what would *prove* a change finished.
///
/// A client cannot derive this from the snapshot. `display_status: merged` says
/// what the owner believes; which repository evidence certifies that depends on
/// whether the owner merges to base, publishes base, or pushes the change
/// branch. If any of these fields is unpublished, a generated client has no way
/// to verify completion and can only fall back to trusting presentation — which
/// is the exact failure this resource exists to remove.
fn owner_contract_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let expected_modes: BTreeSet<String> = ALL_TERMINAL_MODES
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    if published_enum(doc, "TerminalMode") != expected_modes {
        errors.push(format!(
            "TerminalMode must publish exactly {expected_modes:?}, found {:?}",
            published_enum(doc, "TerminalMode")
        ));
    }

    let contract: BTreeSet<&str> = schemas(doc)["OwnerExecutionContract"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for member in ["base_branch", "terminal_mode", "remote", "pushed_branch"] {
        if !contract.contains(member) {
            errors.push(format!("OwnerExecutionContract must publish `{member}`"));
        }
    }
    // Only the two universal facts are required: `remote` and `pushed_branch`
    // are omitted when inapplicable, and publishing them as required would make
    // a generated client demand a value a `merged` owner never has.
    let required: BTreeSet<&str> = schemas(doc)["OwnerExecutionContract"]["required"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for member in ["base_branch", "terminal_mode"] {
        if !required.contains(member) {
            errors.push(format!(
                "OwnerExecutionContract.{member} must be required: every mode has one"
            ));
        }
    }
    for member in ["remote", "pushed_branch"] {
        if required.contains(member) {
            errors.push(format!(
                "OwnerExecutionContract.{member} must stay optional: it is absent for the modes \
                 it does not apply to"
            ));
        }
    }

    // The response joins the contract to an incarnation and a revision, which is
    // what makes it combinable with `/api/v2/state` in one coherent observation.
    let response: BTreeSet<&str> = schemas(doc)["ExecutionContractResponse"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for member in ["instance_id", "state_revision", "contract"] {
        if !response.contains(member) {
            errors.push(format!(
                "ExecutionContractResponse must publish `{member}`; without it the contract \
                 cannot be reconciled with a snapshot"
            ));
        }
    }

    // Typed command capability, so a client learns "this process cannot mutate"
    // from discovery rather than from a refusal it has to classify.
    let capability: BTreeSet<&str> = schemas(doc)["CommandExecutionCapability"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if !capability.contains("available") {
        errors.push("CommandExecutionCapability must publish `available`".to_string());
    }
    let capabilities: BTreeSet<&str> = schemas(doc)["CapabilitiesResponse"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if !capabilities.contains("command_execution") {
        errors.push(
            "CapabilitiesResponse must publish `command_execution`; discovery is where a client \
             learns an executor is unbound"
                .to_string(),
        );
    }

    errors
}

// ============================================================================
// The generated document is complete
// ============================================================================

/// The owner execution contract is published with its closed mode vocabulary.
#[test]
fn owner_execution_contract_is_published_with_its_terminal_modes() {
    assert_eq!(owner_contract_errors(&generated()), Vec::<String>::new());
}

/// The execution-status resource is published with its closed vocabularies.
#[test]
fn agent_execution_observability_contract_publishes_the_execution_surface() {
    assert_eq!(
        execution_observability_errors(&generated()),
        Vec::<String>::new()
    );
}

#[test]
fn the_document_publishes_exactly_the_supported_route_surface() {
    assert_eq!(route_surface_errors(&generated()), Vec::<String>::new());
}

#[test]
fn no_removed_path_is_presented_as_supported() {
    assert_eq!(removed_path_errors(&generated()), Vec::<String>::new());
}

#[test]
fn bearer_authentication_is_declared_and_scoped() {
    assert_eq!(security_errors(&generated()), Vec::<String>::new());
}

#[test]
fn the_command_schema_covers_exactly_the_supported_command_set() {
    assert_eq!(
        command_vocabulary_errors(&generated()),
        Vec::<String>::new()
    );
}

#[test]
fn the_error_code_enum_covers_every_typed_error() {
    assert_eq!(error_code_errors(&generated()), Vec::<String>::new());
}

#[test]
fn command_outcome_and_event_vocabularies_are_complete() {
    assert_eq!(event_vocabulary_errors(&generated()), Vec::<String>::new());
}

#[test]
fn every_reference_resolves_and_no_schema_is_unreachable() {
    assert_eq!(reference_errors(&generated()), Vec::<String>::new());
}

#[test]
fn the_members_a_client_branches_on_are_published_as_required() {
    assert_eq!(required_member_errors(&generated()), Vec::<String>::new());
}

#[test]
fn published_schemas_describe_what_the_server_actually_serializes() {
    assert_eq!(payload_errors(&generated()), Vec::<String>::new());
}

#[test]
fn the_authoritative_snapshot_publishes_every_operator_field() {
    assert_eq!(snapshot_field_errors(&generated()), Vec::<String>::new());
}

/// The retired Apply-limit token stays published, and stays a token.
///
/// No current projection or command admission produces
/// `apply_iteration_limit_active` any more: a retained Apply ceiling is
/// diagnostic evidence about the invocation that stopped, not an operator-action
/// block. The member still has to appear in the generated document, because a
/// client generated against an older contract — or one replaying a recorded
/// snapshot that carries it — would otherwise drop the variant or fail to
/// deserialize.
#[test]
fn retired_iteration_limit_blocked_reason_token_remains_published() {
    let doc = generated();
    let published = published_enum(&doc, "ActionBlockedReason");
    assert!(
        published.contains("apply_iteration_limit_active"),
        "the retired Apply-limit reason must stay published: {published:?}"
    );
    // The same token the server actually serializes, taken from the type rather
    // than repeated as a literal on both sides.
    assert_eq!(
        serde_json::to_value(
            conflux::web::remote_control_api::dto::ActionBlockedReason::ApplyIterationLimitActive
        )
        .unwrap(),
        json!("apply_iteration_limit_active")
    );
    // And a payload carrying it still validates against the published schema.
    let mut resource = change_resource();
    resource.actions.retry_change = ActionEligibility::blocked(
        conflux::web::remote_control_api::dto::ActionBlockedReason::ApplyIterationLimitActive,
    );
    let mut errors = Vec::new();
    let value = to_value(&resource);
    let defs = schemas(&doc);
    validate(
        &value,
        &defs["ChangeResource"],
        defs,
        "ChangeResource",
        &mut errors,
    );
    assert_eq!(errors, Vec::<String>::new());
}

/// Closes the third side of the triangle: the constant and the document could
/// agree with each other and still describe a route the server never binds.
///
/// The probe is credential-free against a token-protected instance, which
/// separates the two answers cleanly. A gated route refuses with 401 before any
/// handler runs, so a bound route cannot be mistaken for an unbound one just
/// because its handler happens to answer 404 for a missing resource; an
/// unmatched path never reaches the gate at all and falls through to 404.
#[tokio::test]
async fn every_published_path_is_bound_with_the_authentication_it_declares() {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    let router = protected_router();

    for path in SUPPORTED_V2_PATHS {
        let concrete = path
            .replace("{change_id}", "no-such-change")
            .replace("{worktree_id}", "0")
            .replace("{command_id}", "0");
        let method = if *path == "/api/v2/commands" {
            Method::POST
        } else {
            Method::GET
        };
        let request = Request::builder()
            .method(method)
            .uri(&concrete)
            .header("host", "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let status = router.clone().oneshot(request).await.unwrap().status();

        if UNAUTHENTICATED_V2_PATHS.contains(path) {
            assert_ne!(
                status,
                StatusCode::NOT_FOUND,
                "{path} is published as unauthenticated but the router does not bind it"
            );
        } else {
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "{path} is published behind the bearer scheme; a credential-free request \
                 must be refused by the gate, and a 404 here means the route is unbound"
            );
        }
    }
}

/// A v2 router that enforces a bearer token, with the executor stubbed out.
///
/// Nothing here executes: the probe above never gets past the gate, and the
/// unauthenticated routes it does reach serve the contract rather than state.
fn protected_router() -> axum::Router {
    use std::sync::Arc;

    use conflux::web::remote_control_api::auth::RemoteControlAuth;
    use conflux::web::remote_control_api::dto::CommandSpec;
    use conflux::web::remote_control_api::executor::{
        CommandFailure, ExecutionSummary, RemoteControlExecutor,
    };
    use conflux::web::remote_control_api::projection::Projection;
    use conflux::web::remote_control_api::{router, RemoteControlState};

    struct RefusingExecutor;

    #[async_trait::async_trait]
    impl RemoteControlExecutor for RefusingExecutor {
        async fn execute(
            &self,
            _command: &CommandSpec,
        ) -> Result<ExecutionSummary, CommandFailure> {
            unreachable!("the route-binding probe never presents credentials")
        }
    }

    let auth = RemoteControlAuth::new(Some("probe-token".to_string()), &[])
        .expect("no origins is a valid policy");
    router(RemoteControlState::new(
        Arc::new(Projection::new()),
        Arc::new(auth),
        Arc::new(RefusingExecutor),
    ))
}

// ============================================================================
// The completeness check would notice
// ============================================================================

/// One incompleteness fixture: what it does to the document, the substring the
/// resulting failure must name, and the mutation that removes the contract element.
type IncompleteCase = (&'static str, &'static str, Box<dyn Fn(&mut Value)>);

/// Everything above proves the document is *currently* complete. This proves the
/// check would notice if it stopped being complete — the one property a suite of
/// positive assertions can never establish about itself.
///
/// Each case removes exactly one contract element from a private copy of the
/// document and requires the predicates to fail *and* to name what went missing,
/// because a failure that does not identify the element is not a useful error
/// message. The real document is never written to or mutated.
#[test]
fn incomplete_contracts_are_rejected_and_the_failure_names_what_is_missing() {
    let cases: Vec<IncompleteCase> = vec![
        (
            "a supported route is dropped",
            "/api/v2/state",
            Box::new(|doc| {
                doc["paths"]
                    .as_object_mut()
                    .unwrap()
                    .remove("/api/v2/state")
                    .expect("the route was published");
            }),
        ),
        (
            "a removed route reappears",
            "/ws",
            Box::new(|doc| {
                let health = doc["paths"]["/api/v2/health"].clone();
                doc["paths"]
                    .as_object_mut()
                    .unwrap()
                    .insert("/ws".to_string(), health);
            }),
        ),
        (
            "a command variant is dropped",
            "the command",
            Box::new(|doc| {
                doc["components"]["schemas"]["CommandSpec"]["oneOf"]
                    .as_array_mut()
                    .unwrap()
                    .pop()
                    .expect("the union has variants");
            }),
        ),
        (
            "an error code is dropped",
            "the error code",
            Box::new(|doc| {
                doc["components"]["schemas"]["ErrorCode"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .pop()
                    .expect("the vocabulary has members");
            }),
        ),
        (
            "the execution-status route is dropped",
            "/api/v2/execution-status",
            Box::new(|doc| {
                doc["paths"]
                    .as_object_mut()
                    .unwrap()
                    .remove("/api/v2/execution-status")
                    .expect("the route was published");
            }),
        ),
        (
            "the typed stop settlement result is dropped",
            "stop_and_dequeue",
            Box::new(|doc| {
                doc["components"]["schemas"]["CommandResult"]["oneOf"]
                    .as_array_mut()
                    .unwrap()
                    .clear();
            }),
        ),
        (
            "an Apply-commit evidence member is dropped",
            "apply_commit",
            Box::new(|doc| {
                for variant in doc["components"]["schemas"]["CommandResult"]["oneOf"]
                    .as_array_mut()
                    .unwrap()
                {
                    variant["properties"]
                        .as_object_mut()
                        .unwrap()
                        .remove("apply_commit");
                }
            }),
        ),
        (
            "a phase vocabulary member is dropped",
            "ExecutionPhase",
            Box::new(|doc| {
                doc["components"]["schemas"]["ExecutionPhase"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|member| member != "acceptance");
            }),
        ),
        (
            "the latest-log projection grows a workspace path",
            "LatestLogProjection",
            Box::new(|doc| {
                doc["components"]["schemas"]["LatestLogProjection"]["properties"]
                    .as_object_mut()
                    .unwrap()
                    .insert("workspace_path".to_string(), json!({"type": "string"}));
            }),
        ),
        (
            "the execution status grows an elapsed counter",
            "elapsed_seconds",
            Box::new(|doc| {
                doc["components"]["schemas"]["ChangeExecutionStatus"]["properties"]
                    .as_object_mut()
                    .unwrap()
                    .insert("elapsed_seconds".to_string(), json!({"type": "integer"}));
            }),
        ),
        (
            "the replay-gap event category is dropped",
            "gap",
            Box::new(|doc| {
                doc["components"]["schemas"]["EventCategory"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|member| member != "gap");
            }),
        ),
        (
            "the bearer scheme is dropped",
            BEARER_SCHEME,
            Box::new(|doc| {
                doc["components"]["securitySchemes"]
                    .as_object_mut()
                    .unwrap()
                    .remove(BEARER_SCHEME)
                    .expect("the scheme was declared");
            }),
        ),
        (
            "an unauthenticated route stops overriding the default",
            "/api/v2/health",
            Box::new(|doc| {
                doc["paths"]["/api/v2/health"]["get"]
                    .as_object_mut()
                    .unwrap()
                    .remove("security")
                    .expect("the override was declared");
            }),
        ),
        (
            "a required stream-ordering member becomes optional",
            "event_sequence",
            Box::new(|doc| {
                doc["components"]["schemas"]["EventEnvelope"]["required"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|member| member != "event_sequence");
            }),
        ),
        (
            "a snapshot field is dropped",
            "queue_intent",
            Box::new(|doc| {
                doc["components"]["schemas"]["ChangeResource"]["properties"]
                    .as_object_mut()
                    .unwrap()
                    .remove("queue_intent")
                    .expect("the field was published");
            }),
        ),
    ];

    let baseline = generated();
    assert_eq!(
        contract_errors(&baseline),
        Vec::<String>::new(),
        "the generated contract must start complete"
    );

    for (case, expected, mutate) in cases {
        let mut incomplete = baseline.clone();
        mutate(&mut incomplete);
        assert_ne!(
            incomplete, baseline,
            "the `{case}` fixture must actually change the document"
        );

        let errors = contract_errors(&incomplete);
        assert!(
            !errors.is_empty(),
            "contract verification accepted a document where {case}"
        );
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "the failure for `{case}` must name `{expected}`, got: {errors:?}"
        );
    }

    // The mutations were applied to private copies; the real contract is intact.
    assert_eq!(contract_errors(&generated()), Vec::<String>::new());
}

// ============================================================================
// Structural validation
// ============================================================================

fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("DTOs must serialize")
}

/// Structural JSON Schema check, limited to the constructs utoipa emits here.
///
/// Not a general validator: it exists to catch a published schema that has
/// drifted away from what the Rust types serialize, which is a mismatch of
/// required members, property names, enum members, and container shape.
fn validate(value: &Value, schema: &Value, defs: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/components/schemas/"))
    {
        let resolved = &defs[reference];
        if resolved.is_null() {
            errors.push(format!("{path}: unresolved $ref to {reference}"));
        } else {
            validate(value, resolved, defs, path, errors);
        }
        return;
    }

    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matched = branches.iter().any(|branch| {
            let mut branch_errors = Vec::new();
            validate(value, branch, defs, path, &mut branch_errors);
            branch_errors.is_empty()
        });
        if !matched {
            errors.push(format!("{path}: {value} matches no branch of the union"));
        }
        return;
    }

    if let Some(members) = schema.get("enum").and_then(Value::as_array) {
        if !members.contains(value) {
            errors.push(format!("{path}: {value} is not a published member"));
        }
        return;
    }

    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        // An untyped schema (for example an opaque event payload) accepts anything.
        _ => return,
    };

    let actual = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::String(_) => "string",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    // JSON Schema treats an integer as an acceptable `number`.
    let accepted = types.contains(&actual) || (actual == "integer" && types.contains(&"number"));
    if !accepted {
        errors.push(format!("{path}: serialized {actual}, published {types:?}"));
        return;
    }

    match value {
        Value::Object(members) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            for required in schema
                .get("required")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                let name = required.as_str().unwrap_or_default();
                if !members.contains_key(name) {
                    errors.push(format!("{path}: required `{name}` is not serialized"));
                }
            }
            if let Some(properties) = properties {
                for (name, member) in members {
                    match properties.get(name) {
                        Some(property) => {
                            validate(member, property, defs, &format!("{path}.{name}"), errors)
                        }
                        None => {
                            errors.push(format!("{path}: `{name}` is serialized but unpublished"))
                        }
                    }
                }
            }
        }
        Value::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    validate(item, item_schema, defs, &format!("{path}[{index}]"), errors);
                }
            }
        }
        _ => {}
    }
}

// ============================================================================
// Fixtures built from the real types
// ============================================================================

fn state_response() -> StateResponse {
    StateResponse {
        instance_id: "instance-1".to_string(),
        state_revision: 7,
        event_sequence: 12,
        snapshot: InstanceSnapshot {
            app_mode: "running".to_string(),
            persistent_scheduler_idle: false,
            is_resolving: false,
            process_error: Some("sanitized process failure".to_string()),
            parallel: ParallelRuntimeState {
                max_concurrent: 4,
                vcs_backend: "git".to_string(),
            },
            changes: vec![change_resource()],
            totals: SnapshotTotals {
                total: 1,
                completed: 0,
                in_progress: 1,
                pending: 0,
            },
        },
    }
}

fn change_resource() -> ChangeResource {
    ChangeResource {
        id: "a-change".to_string(),
        display_status: "blocked".to_string(),
        progress_status: "in_progress".to_string(),
        completed_tasks: 1,
        total_tasks: 3,
        progress_percent: 33.3,
        dependencies: vec!["another-change".to_string()],
        iteration_number: Some(2),
        execution_marked: true,
        queue_intent: QueueIntent::Queued,
        attention: AttentionState::New,
        blocker: Some(ChangeBlocker {
            status: "blocked".to_string(),
            kind: conflux::web::remote_control_api::dto::BlockerKind::Dependency,
            category: Some("dependency".to_string()),
            detail: Some("waiting on another-change".to_string()),
            unblock_condition: Some("another-change is archived".to_string()),
            prerequisite_owner: Some("platform".to_string()),
            origin: Some("analyze".to_string()),
            resumable: true,
        }),
        error_detail: Some("sanitized change error".to_string()),
        actions: ChangeActions {
            set_execution_mark: ActionEligibility::allowed(),
            set_queue_intent: ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::StopPending,
            ),
            retry_change: ActionEligibility::allowed(),
            stop_and_dequeue: ActionEligibility::allowed(),
            resolve_merge: ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::NotMergeWaiting,
            ),
        },
        parallel: ParallelEligibility {
            eligible: false,
            blocked_reason: Some(
                conflux::web::remote_control_api::dto::ParallelBlockedReason::UncommittedChanges,
            ),
        },
        timing: ChangeTiming {
            started_at: Some("2026-08-04T00:00:00Z".to_string()),
            completed_at: Some("2026-08-04T00:01:00Z".to_string()),
            elapsed_ms: Some(60_000),
        },
        latest_activity: Some(ChangeActivity {
            event_type: "processing_started".to_string(),
            timestamp: "2026-08-04T00:00:00Z".to_string(),
            detail: Some("apply".to_string()),
        }),
        worktree: Some(ChangeWorktree {
            path: ".worktrees/a-change".to_string(),
            branch: Some("a-change".to_string()),
        }),
    }
}

fn command_record() -> CommandRecord {
    CommandRecord {
        command_id: "0".repeat(32),
        instance_id: "instance-1".to_string(),
        command_type: "retry_change".to_string(),
        state: CommandState::Failed,
        expected_revision: 7,
        result_revision: Some(8),
        correlation_id: "corr-1".to_string(),
        idempotency_key: "key-1".to_string(),
        created_at: "2026-08-04T00:00:00Z".to_string(),
        completed_at: Some("2026-08-04T00:00:01Z".to_string()),
        detail: Some("sanitized detail".to_string()),
        error_code: Some(ErrorCode::TargetIneligible),
        result: None,
    }
}

/// A settled stop-and-dequeue record, with its typed settlement evidence.
fn settled_stop_record() -> CommandRecord {
    CommandRecord {
        command_type: "stop_and_dequeue".to_string(),
        state: CommandState::Succeeded,
        error_code: None,
        detail: Some("'alpha' was cancelled during acceptance and dequeued".to_string()),
        result: Some(CommandResult::StopAndDequeue {
            cancelled_phase: ExecutionPhase::Acceptance,
            last_completed_phase: Some(ExecutionPhase::Apply),
            apply_commit: ApplyCommitEvidence {
                present: Some(true),
                oid: Some("9f1c0de0c0ffee0000000000000000000000abcd".to_string()),
            },
            effects_rolled_back: false,
        }),
        ..command_record()
    }
}

/// A live execution observation, built field by field so a new published member
/// cannot land without this file changing with it.
fn execution_status() -> ExecutionStatusResponse {
    ExecutionStatusResponse {
        instance_id: "instance-1".to_string(),
        state_revision: 12,
        event_sequence: 30,
        observed_at: "2026-08-04T00:00:02Z".to_string(),
        process: ProcessExecutionStatus {
            app_mode: "running".to_string(),
            scheduler_running: true,
            has_active_work: true,
            active_activities: vec!["dependency_analysis".to_string()],
            latest_log: Some(latest_log()),
        },
        changes: vec![ChangeExecutionStatus {
            id: "a-change".to_string(),
            execution_id: Some("0123456789abcdef0123456789abcdef".to_string()),
            execution_state: ChangeExecutionState::Active,
            current_phase: ExecutionPhase::Acceptance,
            last_completed_phase: Some(ExecutionPhase::Apply),
            iteration: Some(2),
            phase_started_at: Some("2026-08-04T00:00:01Z".to_string()),
            last_completed_at: Some("2026-08-04T00:00:00Z".to_string()),
            run_started_at: Some("2026-08-03T23:59:00Z".to_string()),
            run_completed_at: None,
            latest_activity: Some(ChangeActivity {
                event_type: "acceptance_started".to_string(),
                timestamp: "2026-08-04T00:00:01Z".to_string(),
                detail: None,
            }),
            latest_log: Some(latest_log()),
        }],
    }
}

fn latest_log() -> LatestLogProjection {
    LatestLogProjection {
        message: "acceptance iteration 2".to_string(),
        level: conflux::events::LogLevel::Info,
        operation: Some("acceptance".to_string()),
        iteration: Some(2),
        created_at: "2026-08-04T00:00:01Z".to_string(),
    }
}

fn worktree_response() -> WorktreeResponse {
    WorktreeResponse {
        instance_id: "instance-1".to_string(),
        state_revision: 7,
        worktree: WorktreeResource {
            worktree_id: "f".repeat(32),
            repository_id: "0123456789abcdef".to_string(),
            path: ".worktrees/a-change".to_string(),
            branch: "a-change".to_string(),
            head: "abc123".to_string(),
            is_main: false,
            is_detached: false,
            dirty: Some(true),
            has_commits_ahead: true,
            inspection: conflux::worktree_ops::InspectionState::Checked,
            conflict: Some(WorktreeConflict::new(vec!["src/lib.rs".to_string()])),
            operations: WorktreeEligibility {
                deletable: false,
                mergeable: false,
                delete_blocked_reason: Some("worktree_dirty".to_string()),
                merge_blocked_reason: Some("merge_conflict".to_string()),
            },
        },
    }
}

fn event_envelope() -> EventEnvelope {
    EventEnvelope {
        instance_id: "instance-1".to_string(),
        event_sequence: 12,
        state_revision: 7,
        category: EventCategory::Gap,
        event_type: "gap".to_string(),
        timestamp: "2026-08-04T00:00:00Z".to_string(),
        change_id: Some("a-change".to_string()),
        payload: json!({"reason": "cursor_evicted"}),
    }
}

fn api_error() -> ApiError {
    ApiError::new(ErrorCode::StaleRevision, "revision moved", "corr-1").with_revision(9)
}
