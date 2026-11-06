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
    LatestLogProjection, NetActiveTiming, ParallelEligibility, ParallelRuntimeState,
    ProcessExecutionStatus, QueueIntent, RunClock, SnapshotTotals, StateResponse,
    ALL_CHANGE_EXECUTION_STATES, ALL_ERROR_CODES, ALL_EXECUTION_PHASES, ALL_TERMINAL_MODES,
    SUPPORTED_COMMANDS,
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

    // The targeted force-stop settlement. A client must be able to tell an
    // immediate kill from a graceful stop, and to read which episode it ended,
    // without inspecting Git or parsing the detail sentence.
    let force_stop_variant = schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants.iter().find(|variant| {
                variant["properties"]["kind"]["enum"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| value == "force_stop_change"))
            })
        })
        .cloned();
    match force_stop_variant {
        None => errors.push(
            "CommandResult must publish the force_stop_change settlement variant".to_string(),
        ),
        Some(variant) => {
            let properties: BTreeSet<&str> = variant["properties"]
                .as_object()
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            for member in [
                "change_id",
                "execution_id",
                "cancelled_phase",
                "last_completed_phase",
                "terminated",
                "apply_commit",
                "effects_rolled_back",
            ] {
                if !properties.contains(member) {
                    errors.push(format!(
                        "the force_stop_change result must publish `{member}`; a client would \
                         otherwise have to re-observe the owner to learn what was killed"
                    ));
                }
            }
        }
    }

    // Per-change eligibility for the targeted kill is published separately from
    // the graceful stop, because the two are refused for different reasons.
    let action_properties: BTreeSet<&str> = schemas(doc)["ChangeActions"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for member in ["stop_and_dequeue", "force_stop_change"] {
        if !action_properties.contains(member) {
            errors.push(format!("ChangeActions must publish `{member}` eligibility"));
        }
    }
    let blocked_reasons = published_enum(doc, "ActionBlockedReason");
    for token in ["not_admitted", "no_managed_process"] {
        if !blocked_reasons.contains(token) {
            errors.push(format!(
                "ActionBlockedReason must publish `{token}`, the targeted force-stop refusal a \
                 client branches on"
            ));
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

/// A generated client can drive the targeted kill and read its settlement.
///
/// The surrounding completeness tests already fail if any of this is dropped;
/// this one states the contract as a client meets it — command in, typed result
/// out, eligibility readable ahead of time — so the change's own verification
/// exercises the published document rather than only the aggregate checks.
#[test]
fn force_stop_change_contract_is_published_end_to_end() {
    let doc = generated();

    // Submitting it: one target, and a `change_id` that is a string. A client
    // generated against this document has no way to spell a target *list*, which
    // is what keeps the targeted kill from widening into the process-wide one.
    let spec = schemas(&doc)["CommandSpec"]["oneOf"]
        .as_array()
        .expect("CommandSpec is a discriminated union")
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"][0] == "force_stop_change")
        .expect("the force_stop_change command variant is published")
        .clone();
    let required: BTreeSet<&str> = spec["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        required.contains("change_id"),
        "the target is mandatory: {spec}"
    );
    assert_eq!(
        spec["properties"]["change_id"]["type"], "string",
        "the target is one change, never a list: {spec}"
    );
    assert!(
        spec["properties"].get("change_ids").is_none(),
        "a target list must not be publishable on this command: {spec}"
    );

    // Reading the settlement: its own `kind`, so a client can tell an immediate
    // kill from a graceful stop without parsing prose, plus the members it needs
    // to know what was ended and what survived.
    let result = schemas(&doc)["CommandResult"]["oneOf"]
        .as_array()
        .expect("CommandResult is a discriminated union")
        .iter()
        .find(|variant| {
            variant["properties"]["kind"]["enum"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == "force_stop_change"))
        })
        .expect("the force_stop_change settlement variant is published")
        .clone();
    let members: BTreeSet<&str> = result["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for member in [
        "change_id",
        "execution_id",
        "cancelled_phase",
        "last_completed_phase",
        "terminated",
        "apply_commit",
        "effects_rolled_back",
    ] {
        assert!(
            members.contains(member),
            "the result must publish `{member}`"
        );
    }

    // Eligibility ahead of time, and the two typed refusals a client branches on.
    // The graceful control keeps its own field: neither answers for the other.
    let actions: BTreeSet<&str> = schemas(&doc)["ChangeActions"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default();
    for member in ["stop_and_dequeue", "force_stop_change"] {
        assert!(
            actions.contains(member),
            "ChangeActions must publish `{member}` eligibility"
        );
    }
    let blocked = published_enum(&doc, "ActionBlockedReason");
    for token in ["not_admitted", "no_managed_process"] {
        assert!(
            blocked.contains(token),
            "ActionBlockedReason must publish `{token}`: {blocked:?}"
        );
    }
}

/// Everything a client of the abort-on-conflict merge branches on.
fn merge_abort_contract_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    let spec = schemas(doc)["CommandSpec"]["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants.iter().find(|variant| {
                variant["properties"]["type"]["enum"][0] == "merge_worktree_abort_on_conflict"
            })
        })
        .cloned();
    match spec {
        None => errors.push("the merge_worktree_abort_on_conflict command is not published".into()),
        Some(spec) => {
            let properties: BTreeSet<&str> = spec["properties"]
                .as_object()
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if properties != BTreeSet::from(["type", "target", "params"]) {
                errors.push(format!(
                    "merge_worktree_abort_on_conflict must accept only target and params: {properties:?}"
                ));
            }
        }
    }

    let result_published = schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .is_some_and(|variants| {
            variants
                .iter()
                .any(|variant| variant.to_string().contains("\"merge_abort\""))
        });
    if !result_published {
        errors.push("the merge_abort result kind is not published".into());
    }
    let result = &schemas(doc)["MergeAbortResult"];
    let required: BTreeSet<&str> = result["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for member in [
        "worktree_id",
        "branch",
        "outcome",
        "premerge_head",
        "conflict_files",
        "abort",
        "recovery",
    ] {
        if !required.contains(member) {
            errors.push(format!("MergeAbortResult must require `{member}`"));
        }
    }
    let evidence: BTreeSet<&str> = schemas(doc)["MergeAbortEvidence"]["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for member in [
        "exit_ok",
        "head_after",
        "merge_head_present",
        "clean_after",
        "restored",
        "detail",
    ] {
        if !evidence.contains(member) {
            errors.push(format!("MergeAbortEvidence must require `{member}`"));
        }
    }
    let restored = published_enum(doc, "MergeAbortRestoration");
    if restored != BTreeSet::from(["confirmed", "failed", "unknown"].map(String::from)) {
        errors.push(format!("MergeAbortRestoration vocabulary: {restored:?}"));
    }

    let codes = published_enum(doc, "ErrorCode");
    for code in [
        "merge_conflict",
        "merge_conflict_predicted",
        "merge_conflict_aborted",
        "merge_abort_failed",
    ] {
        if !codes.contains(code) {
            errors.push(format!("the error code `{code}` is not published"));
        }
    }

    let worktrees = &schemas(doc)["WorktreeCapabilities"];
    if worktrees["properties"]["merge_abort"].is_null() {
        errors.push("capabilities.worktrees.merge_abort is not published".into());
    }
    let worktree_required: BTreeSet<&str> = worktrees["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !worktree_required.contains("merge_conflict_preserves_state") {
        errors.push("merge_conflict_preserves_state must stay required".into());
    }
    if worktree_required.contains("merge_abort") {
        errors.push("merge_abort must decode as absent from an older owner".into());
    }
    errors
}

#[test]
fn remote_merge_abort_contract_is_published_end_to_end() {
    use conflux::web::remote_control_api::dto::{
        MergeAbortEvidence, MergeAbortOutcome, MergeAbortRestoration, MergeAbortResult,
    };
    use conflux::web::remote_control_api::worktrees::WorktreeCapabilities;

    let doc = generated();
    assert_eq!(merge_abort_contract_errors(&doc), Vec::<String>::new());

    // What the server serializes is what the schema describes.
    let record = CommandRecord {
        command_type: "merge_worktree_abort_on_conflict".to_string(),
        state: CommandState::Failed,
        error_code: Some(ErrorCode::MergeAbortFailed),
        result: Some(CommandResult::MergeAbort(Box::new(MergeAbortResult {
            worktree_id: "0123456789abcdef0123456789abcdef".to_string(),
            branch: "alpha".to_string(),
            outcome: MergeAbortOutcome::AbortFailed,
            premerge_head: Some("a".repeat(40)),
            conflict_files: vec!["src/lib.rs".to_string()],
            abort: Some(MergeAbortEvidence {
                exit_ok: false,
                head_after: Some("a".repeat(40)),
                merge_head_present: Some(true),
                clean_after: None,
                restored: MergeAbortRestoration::Unknown,
                detail: "fatal: injected".to_string(),
            }),
            recovery: Some("local_or_tui_required".to_string()),
            merge: None,
            hook: None,
        }))),
        ..command_record()
    };
    let capabilities = WorktreeCapabilities::default();
    assert!(capabilities.merge_conflict_preserves_state);
    let mut errors = Vec::new();
    for (name, payload) in [
        ("CommandRecord", to_value(&record)),
        ("WorktreeCapabilities", to_value(&capabilities)),
    ] {
        validate(
            &payload,
            &schemas(&doc)[name],
            schemas(&doc),
            name,
            &mut errors,
        );
    }
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        to_value(&capabilities)["merge_conflict_preserves_state"],
        true
    );
    assert_eq!(to_value(&record)["result"]["kind"], "merge_abort");

    // Negative controls: the predicate notices a dropped result kind and a
    // dropped error code.
    let mut without_kind = doc.clone();
    without_kind["components"]["schemas"]["CommandResult"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .retain(|variant| !variant.to_string().contains("\"merge_abort\""));
    assert_ne!(without_kind, doc);
    assert!(merge_abort_contract_errors(&without_kind)
        .iter()
        .any(|error| error.contains("merge_abort result kind")));

    let mut without_code = doc.clone();
    without_code["components"]["schemas"]["ErrorCode"]["enum"]
        .as_array_mut()
        .unwrap()
        .retain(|code| code != "merge_abort_failed");
    assert_ne!(without_code, doc);
    assert!(merge_abort_contract_errors(&without_code)
        .iter()
        .any(|error| error.contains("merge_abort_failed")));
}

/// Everything a client of consent-bound confirmed deletion branches on.
fn confirmed_delete_contract_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let properties_of = |schema: &Value| -> BTreeSet<String> {
        schema["properties"]
            .as_object()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    };
    let required_of = |schema: &Value| -> BTreeSet<String> {
        schema["required"]
            .as_array()
            .map(|r| {
                r.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };

    let spec = schemas(doc)["CommandSpec"]["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants.iter().find(|variant| {
                variant["properties"]["type"]["enum"][0] == "delete_worktree_confirmed"
            })
        })
        .cloned();
    match spec {
        None => errors.push("the delete_worktree_confirmed command is not published".into()),
        Some(spec) => {
            let properties = properties_of(&spec);
            if properties != BTreeSet::from(["type", "target", "params"].map(String::from)) {
                errors.push(format!(
                    "delete_worktree_confirmed must accept only target and params: {properties:?}"
                ));
            }
            if !spec["properties"]["params"]
                .to_string()
                .contains("ConfirmedDeleteParams")
            {
                errors
                    .push("delete_worktree_confirmed params must be ConfirmedDeleteParams".into());
            }
        }
    }

    let params = &schemas(doc)["ConfirmedDeleteParams"];
    let expected_params = BTreeSet::from(
        [
            "skip_teardown",
            "discard_known_dirty",
            "discard_commits_ahead",
            "consent",
        ]
        .map(String::from),
    );
    if properties_of(params) != expected_params || required_of(params) != expected_params {
        errors.push(format!(
            "ConfirmedDeleteParams must require exactly the three permissions and consent: {params}"
        ));
    }
    if params["additionalProperties"] != Value::Bool(false) {
        errors.push("ConfirmedDeleteParams must refuse unknown members".into());
    }
    let consent = &schemas(doc)["DeleteConsent"];
    let consent_fields = BTreeSet::from(
        [
            "branch",
            "head",
            "dirty",
            "has_commits_ahead",
            "has_ignored_content",
            "merge_base",
            "discardable_content_id",
            "ignored_content_id",
        ]
        .map(String::from),
    );
    if properties_of(consent) != consent_fields {
        errors.push(format!(
            "DeleteConsent must hold exactly the eight consent fields: {:?}",
            properties_of(consent)
        ));
    }
    if consent["additionalProperties"] != Value::Bool(false) {
        errors.push("DeleteConsent must refuse unknown members".into());
    }

    let preview = &doc["paths"]["/api/v2/worktrees/{worktree_id}/delete-preview"]["get"];
    if preview.is_null() {
        errors.push("the delete-preview route is not published".into());
    } else if !preview.to_string().contains("DeletePreviewResponse") {
        errors.push("the delete-preview route must answer DeletePreviewResponse".into());
    }
    let response = &schemas(doc)["DeletePreviewResponse"];
    for member in [
        "observed_at",
        "branch",
        "head",
        "is_main",
        "dirty",
        "has_commits_ahead",
        "inspection",
        "content_observation",
        "has_ignored_content",
        "ignored_entries",
        "teardown_script_present",
        "options",
        "consent",
    ] {
        if !required_of(response).contains(member) {
            errors.push(format!("DeletePreviewResponse must require `{member}`"));
        }
    }

    let result_published = schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .is_some_and(|variants| {
            variants.iter().any(|variant| {
                variant
                    .to_string()
                    .contains("\"delete_worktree_confirmed\"")
            })
        });
    if !result_published {
        errors.push("the delete_worktree_confirmed result kind is not published".into());
    }
    let result = &schemas(doc)["ConfirmedDeleteResult"];
    for member in [
        "worktree_id",
        "outcome",
        "teardown",
        "dirty_discarded",
        "commits_discarded",
        "branch_deleted",
        "branch_retained_reason",
        "refusal",
    ] {
        if !required_of(result).contains(member) {
            errors.push(format!("ConfirmedDeleteResult must require `{member}`"));
        }
    }
    let teardown = published_enum(doc, "TeardownDisposition");
    if teardown != BTreeSet::from(["ran", "skipped", "absent", "not_reached"].map(String::from)) {
        errors.push(format!("TeardownDisposition vocabulary: {teardown:?}"));
    }
    let observation = published_enum(doc, "ContentObservationKind");
    if observation
        != BTreeSet::from(
            [
                "observed",
                "status_failed",
                "unbounded",
                "nested_repository",
            ]
            .map(String::from),
        )
    {
        errors.push(format!(
            "ContentObservationKind vocabulary: {observation:?}"
        ));
    }
    let reasons = published_enum(doc, "ConfirmedDeleteReason");
    for reason in [
        "content_changed",
        "content_unbounded",
        "nested_repository",
        "ignored_content",
        "teardown_failed",
    ] {
        if !reasons.contains(reason) {
            errors.push(format!("ConfirmedDeleteReason must publish `{reason}`"));
        }
    }

    if !published_enum(doc, "ErrorCode").contains("delete_consent_stale") {
        errors.push("the error code `delete_consent_stale` is not published".into());
    }

    let worktrees = &schemas(doc)["WorktreeCapabilities"];
    if worktrees["properties"]["confirmed_delete"].is_null() {
        errors.push("capabilities.worktrees.confirmed_delete is not published".into());
    }
    let worktree_required = required_of(worktrees);
    if worktree_required.contains("confirmed_delete") {
        errors.push("confirmed_delete must decode as absent from an older owner".into());
    }
    if !worktree_required.contains("delete_requires_teardown") {
        errors.push("delete_requires_teardown must stay required".into());
    }
    let description = worktrees["properties"]["delete_requires_teardown"]["description"]
        .as_str()
        .unwrap_or_default();
    if !description.contains("`delete_worktree` command") {
        errors.push(format!(
            "delete_requires_teardown must describe the delete_worktree command: {description}"
        ));
    }
    errors
}

#[test]
fn remote_confirmed_delete_contract_is_published_end_to_end() {
    use conflux::web::remote_control_api::dto::{
        ConfirmedDeleteRefusalResult, ConfirmedDeleteResult,
    };
    use conflux::web::remote_control_api::worktrees::WorktreeCapabilities;
    use conflux::worktree_ops::consent::{
        ConfirmedDeleteReason, ConsentObservation, ContentObservationKind, TeardownDisposition,
    };

    let doc = generated();
    assert_eq!(confirmed_delete_contract_errors(&doc), Vec::<String>::new());
    assert!(SUPPORTED_V2_PATHS.contains(&"/api/v2/worktrees/{worktree_id}/delete-preview"));
    assert!(SUPPORTED_COMMANDS.contains(&"delete_worktree_confirmed"));

    // What the server serializes is what the schema describes.
    let record = CommandRecord {
        command_type: "delete_worktree_confirmed".to_string(),
        state: CommandState::Failed,
        error_code: Some(ErrorCode::DeleteConsentStale),
        result: Some(CommandResult::DeleteWorktreeConfirmed(Box::new(
            ConfirmedDeleteResult {
                worktree_id: "0123456789abcdef0123456789abcdef".to_string(),
                branch: "alpha".to_string(),
                head: "a".repeat(40),
                outcome: None,
                teardown: TeardownDisposition::NotReached,
                dirty_discarded: false,
                commits_discarded: false,
                branch_deleted: false,
                branch_retained_reason: None,
                refusal: Some(ConfirmedDeleteRefusalResult {
                    reason: Some(ConfirmedDeleteReason::ContentChanged),
                    teardown_ran: false,
                    observed: Some(ConsentObservation {
                        branch: "alpha".to_string(),
                        head: "a".repeat(40),
                        dirty: Some(true),
                        has_commits_ahead: Some(false),
                        has_ignored_content: Some(false),
                        merge_base: Some("b".repeat(40)),
                        discardable_content_id: Some(format!("sha256:{}", "c".repeat(64))),
                        ignored_content_id: Some(format!("sha256:{}", "d".repeat(64))),
                        content_observation: ContentObservationKind::Observed,
                    }),
                }),
            },
        ))),
        ..command_record()
    };
    let capabilities = WorktreeCapabilities::default();
    let mut errors = Vec::new();
    for (name, payload) in [
        ("CommandRecord", to_value(&record)),
        ("WorktreeCapabilities", to_value(&capabilities)),
    ] {
        validate(
            &payload,
            &schemas(&doc)[name],
            schemas(&doc),
            name,
            &mut errors,
        );
    }
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        to_value(&record)["result"]["kind"],
        "delete_worktree_confirmed"
    );
    // `delete_requires_teardown` keeps its value; only its description narrowed.
    assert_eq!(to_value(&capabilities)["delete_requires_teardown"], true);
    assert_eq!(
        to_value(&capabilities)["confirmed_delete"]["available"],
        true
    );

    // An older owner omits the block: it decodes as unavailable, and its
    // `delete_requires_teardown: true` still reads the same.
    let mut older = to_value(&capabilities);
    older.as_object_mut().unwrap().remove("confirmed_delete");
    let decoded: WorktreeCapabilities = serde_json::from_value(older).unwrap();
    assert!(!decoded.confirmed_delete.available);
    assert!(decoded.delete_requires_teardown);

    // Negative controls: the predicate notices a dropped route, a dropped
    // result kind, and a delete_requires_teardown that stopped being required.
    let mut without_route = doc.clone();
    without_route["paths"]
        .as_object_mut()
        .unwrap()
        .remove("/api/v2/worktrees/{worktree_id}/delete-preview");
    assert_ne!(without_route, doc);
    assert!(confirmed_delete_contract_errors(&without_route)
        .iter()
        .any(|error| error.contains("delete-preview route")));

    let mut without_kind = doc.clone();
    without_kind["components"]["schemas"]["CommandResult"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .retain(|variant| {
            !variant
                .to_string()
                .contains("\"delete_worktree_confirmed\"")
        });
    assert_ne!(without_kind, doc);
    assert!(confirmed_delete_contract_errors(&without_kind)
        .iter()
        .any(|error| error.contains("delete_worktree_confirmed result kind")));

    let mut optional_teardown = doc.clone();
    optional_teardown["components"]["schemas"]["WorktreeCapabilities"]["required"]
        .as_array_mut()
        .unwrap()
        .retain(|member| member != "delete_requires_teardown");
    assert!(confirmed_delete_contract_errors(&optional_teardown)
        .iter()
        .any(|error| error.contains("delete_requires_teardown must stay required")));
}

/// The server always publishes refresh eligibility even though the client
/// decoder accepts its absence from owners built before refresh existed.
#[test]
fn refresh_eligibility_remains_required_and_non_null_in_published_schema() {
    let doc = generated();
    let actions = &schemas(&doc)["ChangeActions"];
    let required = actions["required"]
        .as_array()
        .expect("ChangeActions publishes a required-member list");
    assert!(
        required.iter().any(|member| member == "refresh_proposal"),
        "the server's refresh eligibility is required in the published contract"
    );
    let property = &actions["properties"]["refresh_proposal"];
    assert_eq!(
        property["$ref"], "#/components/schemas/ActionEligibility",
        "refresh eligibility remains a non-null ActionEligibility reference"
    );
    assert!(
        property
            .get("nullable")
            .is_none_or(|nullable| nullable == false),
        "refresh eligibility must not be nullable"
    );
    assert!(
        property.get("type").is_none_or(|kind| {
            kind.as_array()
                .is_none_or(|types| !types.iter().any(|t| t == "null"))
        }),
        "refresh eligibility must not admit a null type"
    );
    assert!(property.get("anyOf").is_none() && property.get("oneOf").is_none());
}

/// Archive completion and both execution clocks, as a client meets them.
///
/// Each top-level member is required and non-null: this server always writes
/// it, and only an older owner omits it. The nested members are required too —
/// always serialized — with the instants and the frozen duration nullable.
/// `ChangeTiming` keeps exactly its three per-run members.
fn progress_timing_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let defs = schemas(doc);

    for (schema, member, reference) in [
        ("ChangeResource", "archive_complete", None),
        ("ChangeResource", "net_active", Some("NetActiveTiming")),
        ("InstanceSnapshot", "run_clock", Some("RunClock")),
    ] {
        let published = &defs[schema];
        let required = published["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|name| name == member));
        if !required {
            errors.push(format!("{schema}.{member} must be published as required"));
        }
        let property = &published["properties"][member];
        if property.is_null() {
            errors.push(format!("{schema}.{member} is not published"));
            continue;
        }
        let admits_null = property.get("nullable").is_some_and(|n| n == true)
            || property["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|t| t == "null"))
            || property.get("anyOf").is_some()
            || property.get("oneOf").is_some();
        if admits_null {
            errors.push(format!("{schema}.{member} must not be nullable"));
        }
        match reference {
            Some(target) => {
                if property["$ref"] != format!("#/components/schemas/{target}") {
                    errors.push(format!("{schema}.{member} must reference {target}"));
                }
            }
            None => {
                if property["type"] != "boolean" {
                    errors.push(format!("{schema}.{member} must be a boolean"));
                }
            }
        }
    }

    for (schema, members, nullable) in [
        (
            "NetActiveTiming",
            &["accumulated_ms", "active_since", "has_history"][..],
            &["active_since"][..],
        ),
        (
            "RunClock",
            &["started_at", "ended_at", "elapsed_ms"][..],
            &["started_at", "ended_at", "elapsed_ms"][..],
        ),
    ] {
        let published = &defs[schema];
        if published.is_null() {
            errors.push(format!("{schema} is not published"));
            continue;
        }
        let required: BTreeSet<&str> = published["required"]
            .as_array()
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let properties: BTreeSet<&str> = published["properties"]
            .as_object()
            .map(|props| props.keys().map(String::as_str).collect())
            .unwrap_or_default();
        let expected: BTreeSet<&str> = members.iter().copied().collect();
        if properties != expected || required != expected {
            errors.push(format!(
                "{schema} must publish exactly {expected:?} as required (properties {properties:?}, required {required:?})"
            ));
        }
        for member in members {
            let admits_null = published["properties"][member]["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|t| t == "null"));
            if admits_null != nullable.contains(member) {
                errors.push(format!(
                    "{schema}.{member} nullability must be {}",
                    nullable.contains(member)
                ));
            }
        }
    }

    let timing = &defs["ChangeTiming"];
    let timing_members: BTreeSet<&str> = timing["properties"]
        .as_object()
        .map(|props| props.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if timing_members != BTreeSet::from(["started_at", "completed_at", "elapsed_ms"]) {
        errors.push(format!("ChangeTiming members changed: {timing_members:?}"));
    }
    if !timing["description"]
        .as_str()
        .is_some_and(|description| description.contains("not cumulative"))
    {
        errors.push("ChangeTiming must say it is per run and not cumulative".to_string());
    }
    errors
}

#[test]
fn remote_progress_timing_members_are_published_required_and_non_null() {
    let doc = generated();
    assert_eq!(progress_timing_errors(&doc), Vec::<String>::new());

    // The DTO values constructed field by field validate against the document.
    let payload = to_value(&state_response());
    assert_eq!(payload["snapshot"]["changes"][0]["archive_complete"], true);
    assert!(payload["snapshot"]["changes"][0]["net_active"].is_object());
    assert!(payload["snapshot"]["run_clock"].is_object());
    let mut errors = Vec::new();
    validate(
        &payload,
        &schemas(&doc)["StateResponse"],
        schemas(&doc),
        "StateResponse",
        &mut errors,
    );
    assert_eq!(errors, Vec::<String>::new());
}

/// Negative control: a document that drops `net_active` fails both the
/// publication check and payload validation, so the positive case is not
/// passing vacuously.
#[test]
fn remote_progress_timing_omitting_a_member_fails_the_contract() {
    let mut doc = generated();
    let change = doc["components"]["schemas"]["ChangeResource"]
        .as_object_mut()
        .expect("ChangeResource schema");
    change["required"]
        .as_array_mut()
        .expect("required list")
        .retain(|member| member != "net_active");
    change["properties"]
        .as_object_mut()
        .expect("properties")
        .remove("net_active")
        .expect("net_active was published");

    let errors = progress_timing_errors(&doc);
    assert!(
        errors.iter().any(|error| error.contains("net_active")),
        "{errors:?}"
    );
    let mut payload_errors = Vec::new();
    validate(
        &to_value(&state_response()),
        &schemas(&doc)["StateResponse"],
        schemas(&doc),
        "StateResponse",
        &mut payload_errors,
    );
    assert!(
        payload_errors
            .iter()
            .any(|error| error.contains("`net_active` is serialized but unpublished")),
        "{payload_errors:?}"
    );
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
                    // The `refresh_proposal` variant is an `allOf` over its own
                    // result schema and carries no Apply-commit member.
                    if let Some(properties) = variant["properties"].as_object_mut() {
                        properties.remove("apply_commit");
                    }
                }
            }),
        ),
        (
            "the typed targeted force-stop result is dropped",
            "force_stop_change",
            Box::new(|doc| {
                let variants = doc["components"]["schemas"]["CommandResult"]["oneOf"]
                    .as_array_mut()
                    .unwrap();
                variants.retain(|variant| {
                    variant["properties"]["kind"]["enum"]
                        .as_array()
                        .is_none_or(|values| {
                            !values.iter().any(|value| value == "force_stop_change")
                        })
                });
            }),
        ),
        (
            "the per-change targeted force-stop eligibility is dropped",
            "force_stop_change",
            Box::new(|doc| {
                doc["components"]["schemas"]["ChangeActions"]["properties"]
                    .as_object_mut()
                    .unwrap()
                    .remove("force_stop_change")
                    .expect("the field was published");
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
            run_clock: Some(RunClock {
                started_at: Some("2026-08-04T00:00:00Z".to_string()),
                ended_at: Some("2026-08-04T00:15:03Z".to_string()),
                elapsed_ms: Some(903_000),
            }),
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
            dependencies: vec!["another-change".to_string()],
        }),
        error_detail: Some("sanitized change error".to_string()),
        actions: ChangeActions {
            set_execution_mark: ActionEligibility::allowed(),
            set_queue_intent: ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::StopPending,
            ),
            retry_change: ActionEligibility::allowed(),
            stop_and_dequeue: ActionEligibility::allowed(),
            force_stop_change: ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::NoManagedProcess,
            ),
            resolve_merge: ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::NotMergeWaiting,
            ),
            refresh_proposal: Some(ActionEligibility::blocked(
                conflux::web::remote_control_api::dto::ActionBlockedReason::NoCandidateWorktree,
            )),
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
        // A populated record, so every schema validation over this fixture
        // also validates the provenance shape the server serializes.
        acceptance_provenance: Some(
            conflux::orchestration::acceptance::provenance::AcceptanceProvenance {
                decision_source:
                    conflux::orchestration::acceptance::provenance::DecisionSource::NativeVerification,
                reviewer_started: false,
                reason_code: conflux::orchestration::acceptance::provenance::ReasonCode::GateFailed,
                defect_code: None,
                verification_id: Some("focused-gate".to_string()),
                declaration_path: Some("openspec/changes/a-change/proposal.md".to_string()),
                candidate_commit: Some("a".repeat(40)),
                proposal_content_id: Some("b".repeat(40)),
                change_input_digest: Some("c".repeat(64)),
                program: Some("/usr/bin/cargo".to_string()),
                argument_count: Some(2),
                env_names: vec!["RUST_LOG".to_string()],
                remediation:
                    conflux::orchestration::acceptance::provenance::Remediation::RepairCandidate,
                candidate_divergence:
                    conflux::orchestration::acceptance::provenance::CandidateDivergence::unknown(
                        conflux::orchestration::acceptance::provenance::DivergenceUnknownReason::BaseUnresolvable,
                    ),
            },
        ),
        archive_complete: Some(true),
        net_active: Some(NetActiveTiming {
            accumulated_ms: 125_000,
            active_since: Some("2026-08-04T00:14:45.123Z".to_string()),
            has_history: true,
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
            worktree_sessions_running: 1,
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

// ============================================================================
// Bounded remote text edit
// ============================================================================

fn variant<'a>(doc: &'a Value, schema: &str, tag: &str, value: &str) -> Option<&'a Value> {
    schemas(doc)[schema]["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants
                .iter()
                .find(|variant| variant["properties"][tag]["enum"][0] == value)
        })
}

fn property_names(schema: &Value) -> BTreeSet<&str> {
    schema["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

fn required_names(schema: &Value) -> BTreeSet<&str> {
    schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Every element of the text surface a generated client needs, by name.
fn text_edit_contract_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    for route in [
        "/api/v2/changes/{change_id}/text",
        "/api/v2/worktrees/{worktree_id}/text",
    ] {
        let operation = &doc["paths"][route]["get"];
        if operation.is_null() {
            errors.push(format!("the text read route {route} is not published"));
            continue;
        }
        let params: BTreeSet<&str> = operation["parameters"]
            .as_array()
            .map(|p| p.iter().filter_map(|p| p["name"].as_str()).collect())
            .unwrap_or_default();
        if !params.contains("path") {
            errors.push(format!("{route} must publish its `path` query parameter"));
        }
        if !operation.to_string().contains("TextReadResponse") {
            errors.push(format!("{route} must answer with TextReadResponse"));
        }
    }

    for command in ["write_change_text", "write_worktree_text"] {
        match variant(doc, "CommandSpec", "type", command) {
            None => errors.push(format!("the {command} command is not published")),
            Some(spec) => {
                if property_names(spec) != BTreeSet::from(["type", "target", "params"]) {
                    errors.push(format!("{command} must accept only target and params"));
                }
                if !spec["properties"]["params"]
                    .to_string()
                    .contains("TextWriteParams")
                {
                    errors.push(format!("{command} params must be TextWriteParams"));
                }
            }
        }
    }
    let params = &schemas(doc)["TextWriteParams"];
    if required_names(params) != BTreeSet::from(["path", "expected", "content"]) {
        errors.push("TextWriteParams must require exactly path, expected, and content".into());
    }
    for kind in ["content", "absent"] {
        if variant(doc, "TextExpectation", "kind", kind).is_none() {
            errors.push(format!("the `{kind}` expectation is not published"));
        }
    }

    let result_published = schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .is_some_and(|variants| {
            variants
                .iter()
                .any(|variant| variant.to_string().contains("\"text_write\""))
        });
    if !result_published {
        errors.push("the text_write result kind is not published".into());
    }
    let result_required = required_names(&schemas(doc)["TextWriteResult"]);
    for member in [
        "scope",
        "path",
        "outcome",
        "reason",
        "detail",
        "content_id_before",
        "content_id_after",
        "current_content_id",
        "bytes_written",
        "mode",
        "effects",
    ] {
        if !result_required.contains(member) {
            errors.push(format!("TextWriteResult must require `{member}`"));
        }
    }
    let read_required = required_names(&schemas(doc)["TextReadResponse"]);
    for member in [
        "content",
        "content_id",
        "editable",
        "edit_blocked_reason",
        "edit_blocked_detail",
        "limits",
    ] {
        if !read_required.contains(member) {
            errors.push(format!("TextReadResponse must require `{member}`"));
        }
    }
    let reasons = published_enum(doc, "TextEditBlockedReason");
    if reasons
        != BTreeSet::from(
            [
                "change_active",
                "change_admitted",
                "managed_process_live",
                "root_busy",
            ]
            .map(String::from),
        )
    {
        errors.push(format!("TextEditBlockedReason vocabulary: {reasons:?}"));
    }

    let codes = published_enum(doc, "ErrorCode");
    for code in ["text_stale", "text_not_editable"] {
        if !codes.contains(code) {
            errors.push(format!("the error code `{code}` is not published"));
        }
    }

    let capabilities = &schemas(doc)["CapabilitiesResponse"];
    if capabilities["properties"]["text_edit"].is_null() {
        errors.push("capabilities.text_edit is not published".into());
    }
    if required_names(capabilities).contains("text_edit") {
        errors.push("text_edit must decode as absent from an older owner".into());
    }
    errors
}

#[test]
fn remote_text_edit_contract_is_published_end_to_end() {
    use conflux::web::remote_control_api::dto::{
        TextEditBlockedReason, TextReadResponse, TextScope, TextWriteEffects, TextWriteOutcome,
        TextWriteResult,
    };

    let doc = generated();
    assert_eq!(text_edit_contract_errors(&doc), Vec::<String>::new());
    for route in [
        "/api/v2/changes/{change_id}/text",
        "/api/v2/worktrees/{worktree_id}/text",
    ] {
        assert!(SUPPORTED_V2_PATHS.contains(&route), "{route}");
    }
    assert!(SUPPORTED_COMMANDS.contains(&"write_change_text"));
    assert!(SUPPORTED_COMMANDS.contains(&"write_worktree_text"));

    // What the server serializes is what the schemas describe, field by field.
    let record = CommandRecord {
        command_type: "write_worktree_text".to_string(),
        state: CommandState::Failed,
        error_code: Some(ErrorCode::TextStale),
        result: Some(CommandResult::TextWrite(Box::new(TextWriteResult {
            scope: TextScope::Worktree {
                worktree_id: "0123456789abcdef0123456789abcdef".to_string(),
            },
            path: "src/lib.rs".to_string(),
            outcome: None,
            reason: Some("stale".to_string()),
            detail: None,
            content_id_before: None,
            content_id_after: None,
            current_content_id: Some(format!("sha256:{}", "a".repeat(64))),
            bytes_written: None,
            mode: None,
            effects: TextWriteEffects {
                file_written: false,
            },
        }))),
        ..command_record()
    };
    let written = CommandRecord {
        command_type: "write_change_text".to_string(),
        state: CommandState::Succeeded,
        error_code: None,
        result: Some(CommandResult::TextWrite(Box::new(TextWriteResult {
            scope: TextScope::Change {
                change_id: "alpha".to_string(),
            },
            path: "tasks.md".to_string(),
            outcome: Some(TextWriteOutcome::Written),
            reason: None,
            detail: None,
            content_id_before: Some(format!("sha256:{}", "a".repeat(64))),
            content_id_after: Some(format!("sha256:{}", "b".repeat(64))),
            current_content_id: Some(format!("sha256:{}", "b".repeat(64))),
            bytes_written: Some(12),
            mode: Some("0644".to_string()),
            effects: TextWriteEffects { file_written: true },
        }))),
        ..command_record()
    };
    let read = TextReadResponse {
        instance_id: "instance-1".to_string(),
        state_revision: 41,
        scope: TextScope::Worktree {
            worktree_id: "0123456789abcdef0123456789abcdef".to_string(),
        },
        path: "src/lib.rs".to_string(),
        exists: true,
        content: Some("fn main() {}\n".to_string()),
        content_id: Some(format!("sha256:{}", "c".repeat(64))),
        size_bytes: Some(13),
        mode: Some("0644".to_string()),
        editable: false,
        edit_blocked_reason: Some(TextEditBlockedReason::ManagedProcessLive),
        edit_blocked_detail: Some("managed writer coverage is unknown".to_string()),
        limits: conflux::web::remote_control_api::text_edit::limits(),
    };
    let capability = conflux::web::remote_control_api::text_edit::capability(true);
    let mut errors = Vec::new();
    for (name, payload) in [
        ("CommandRecord", to_value(&record)),
        ("CommandRecord", to_value(&written)),
        ("TextReadResponse", to_value(&read)),
        ("TextEditCapability", to_value(&capability)),
    ] {
        validate(
            &payload,
            &schemas(&doc)[name],
            schemas(&doc),
            name,
            &mut errors,
        );
    }
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(to_value(&record)["result"]["kind"], "text_write");
    assert_eq!(
        to_value(&read)["edit_blocked_reason"],
        "managed_process_live"
    );

    // The route surface and removed-path predicates still hold with the two
    // new routes in place.
    assert_eq!(route_surface_errors(&doc), Vec::<String>::new());
    assert_eq!(removed_path_errors(&doc), Vec::<String>::new());
}

#[test]
fn remote_text_edit_contract_omission_is_detected() {
    let doc = generated();

    let mut without_kind = doc.clone();
    without_kind["components"]["schemas"]["CommandResult"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .retain(|variant| !variant.to_string().contains("\"text_write\""));
    assert_ne!(without_kind, doc);
    assert!(text_edit_contract_errors(&without_kind)
        .iter()
        .any(|error| error.contains("text_write result kind")));

    let mut without_route = doc.clone();
    without_route["paths"]
        .as_object_mut()
        .unwrap()
        .remove("/api/v2/worktrees/{worktree_id}/text");
    assert_ne!(without_route, doc);
    assert!(text_edit_contract_errors(&without_route)
        .iter()
        .any(|error| error.contains("/api/v2/worktrees/{worktree_id}/text")));
    assert!(!route_surface_errors(&without_route).is_empty());

    let mut without_command = doc.clone();
    without_command["components"]["schemas"]["CommandSpec"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .retain(|variant| variant["properties"]["type"]["enum"][0] != "write_change_text");
    assert!(text_edit_contract_errors(&without_command)
        .iter()
        .any(|error| error.contains("write_change_text")));

    let mut without_code = doc.clone();
    without_code["components"]["schemas"]["ErrorCode"]["enum"]
        .as_array_mut()
        .unwrap()
        .retain(|code| code != "text_stale");
    assert!(text_edit_contract_errors(&without_code)
        .iter()
        .any(|error| error.contains("text_stale")));
}

// ============================================================================
// Configured-command worktree sessions
// ============================================================================

/// The `kind` discriminants a `CommandResult` variant publishes, whether the
/// variant is inline or an `allOf` over its own result schema.
fn result_kinds(doc: &Value) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    for variant in schemas(doc)["CommandResult"]["oneOf"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let parts: Vec<Value> = match variant["allOf"].as_array() {
            Some(parts) => parts.clone(),
            None => vec![variant.clone()],
        };
        for part in parts {
            for kind in part["properties"]["kind"]["enum"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                if let Some(kind) = kind.as_str() {
                    kinds.insert(kind.to_string());
                }
            }
        }
    }
    kinds
}

fn command_variant<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    schemas(doc)["CommandSpec"]["oneOf"]
        .as_array()?
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"][0] == name)
}

fn session_resource() -> conflux::web::remote_control_api::dto::WorktreeSessionResource {
    use conflux::web::remote_control_api::dto::{
        SessionCleanupFacts, SessionOutput, SessionOutputLine, SessionSetupFacts,
        WorktreeSessionResource,
    };
    WorktreeSessionResource {
        session_id: "a".repeat(32),
        instance_id: "instance-1".to_string(),
        kind: "session".to_string(),
        worktree_id: Some("b".repeat(32)),
        path: "../ws/proposal-1730000000000".to_string(),
        branch: "ws-session-ab12cd34ef".to_string(),
        state: "stopped".to_string(),
        attempt: 1,
        started_at: "2026-08-04T00:00:00Z".to_string(),
        finished_at: Some("2026-08-04T00:00:05Z".to_string()),
        exit_code: None,
        signal: Some("SIGTERM".to_string()),
        stop_requested: true,
        setup: Some(SessionSetupFacts {
            ran: true,
            succeeded: true,
        }),
        cleanup: Some(SessionCleanupFacts {
            outcome: "retained".to_string(),
            removed: false,
            reason: Some("device busy".to_string()),
        }),
        output: SessionOutput {
            lines: vec![SessionOutputLine {
                stream: "stdout".to_string(),
                text: "hello".to_string(),
            }],
            retained_bytes: 5,
            truncated_bytes: 0,
        },
        runner_notices: Some(Vec::new()),
        runner_notices_dropped: Some(0),
    }
}

/// Everything the session surface publishes, as one predicate so the same
/// check can be run against a deliberately incomplete copy.
fn session_contract_errors(doc: &Value) -> Vec<String> {
    use conflux::web::remote_control_api::dto::{
        SessionCleanupFacts, SessionSetupFacts, WorktreeSessionCapability,
        WorktreeSessionEventType, WorktreeSessionResponse, WorktreeSessionResult,
        WorktreeSessionsResponse,
    };
    let mut errors = Vec::new();

    // Three commands, each closed over its own target.
    for (name, target) in [
        ("run_worktree_command", Some("WorktreeTarget")),
        ("create_session_worktree", None),
        ("stop_worktree_session", Some("SessionTarget")),
    ] {
        let Some(variant) = command_variant(doc, name) else {
            errors.push(format!("the command `{name}` is not published"));
            continue;
        };
        let reference = variant["properties"]["target"]["$ref"].as_str();
        match target {
            Some(schema) => {
                if reference != Some(&format!("#/components/schemas/{schema}")) {
                    errors.push(format!("`{name}` must target `{schema}`: {variant}"));
                }
            }
            None => {
                if variant["properties"].get("target").is_some() {
                    errors.push(format!("`{name}` must take no target: {variant}"));
                }
            }
        }
        for smuggled in ["command", "argv", "env", "path", "branch", "change_id"] {
            if variant["properties"].get(smuggled).is_some() {
                errors.push(format!("`{name}` must not publish `{smuggled}`"));
            }
        }
    }
    if schemas(doc)["SessionTarget"]["properties"]
        .as_object()
        .map(|p| p.keys().cloned().collect::<Vec<_>>())
        != Some(vec!["session_id".to_string()])
    {
        errors.push("SessionTarget must carry exactly `session_id`".into());
    }

    // Two read routes, both bearer-gated GETs.
    for route in [
        "/api/v2/worktree-sessions",
        "/api/v2/worktree-sessions/{session_id}",
    ] {
        if doc["paths"][route]["get"].is_null() {
            errors.push(format!("the route `{route}` is not published"));
        }
        if UNAUTHENTICATED_V2_PATHS.contains(&route) {
            errors.push(format!("the route `{route}` must be authenticated"));
        }
    }

    // Two result kinds and three error codes.
    let kinds = result_kinds(doc);
    for kind in ["worktree_session", "worktree_session_stop"] {
        if !kinds.contains(kind) {
            errors.push(format!("the result kind `{kind}` is not published"));
        }
    }
    let codes = published_enum(doc, "ErrorCode");
    for code in [
        "worktree_session_unavailable",
        "worktree_session_busy",
        "worktree_session_setup_failed",
    ] {
        if !codes.contains(code) {
            errors.push(format!("the error code `{code}` is not published"));
        }
    }

    // The capability, the event vocabulary, and the execution-status counter.
    if schemas(doc)["CapabilitiesResponse"]["properties"]["worktree_sessions"].is_null() {
        errors.push("CapabilitiesResponse.worktree_sessions is not published".into());
    }
    let events = published_enum(doc, "WorktreeSessionEventType");
    for event in ["worktree_session_started", "worktree_session_finished"] {
        if !events.contains(event) {
            errors.push(format!("the event type `{event}` is not published"));
        }
    }
    if schemas(doc)["ProcessExecutionStatus"]["properties"]["worktree_sessions_running"].is_null() {
        errors.push("ProcessExecutionStatus.worktree_sessions_running is not published".into());
    }

    // DTO values constructed field by field validate against what is published.
    let launch = CommandRecord {
        command_type: "create_session_worktree".to_string(),
        state: CommandState::Failed,
        error_code: Some(ErrorCode::WorktreeSessionSetupFailed),
        result: Some(CommandResult::WorktreeSession(Box::new(
            WorktreeSessionResult {
                session_id: None,
                session_kind: "session".to_string(),
                worktree_id: None,
                branch: "ws-session-ab12cd34ef".to_string(),
                path: "../ws/proposal-1730000000000".to_string(),
                setup: Some(SessionSetupFacts {
                    ran: true,
                    succeeded: false,
                }),
                cleanup: Some(SessionCleanupFacts {
                    outcome: "removed".to_string(),
                    removed: true,
                    reason: None,
                }),
            },
        ))),
        ..command_record()
    };
    let stop = CommandRecord {
        command_type: "stop_worktree_session".to_string(),
        state: CommandState::Succeeded,
        error_code: None,
        result: Some(CommandResult::WorktreeSessionStop {
            session_id: "a".repeat(32),
            terminated: true,
            state_after: "stopped".to_string(),
        }),
        ..command_record()
    };
    let capability = WorktreeSessionCapability {
        available: true,
        commands: vec![
            "run_worktree_command".to_string(),
            "create_session_worktree".to_string(),
            "stop_worktree_session".to_string(),
        ],
        routes: vec![
            "/api/v2/worktree-sessions".to_string(),
            "/api/v2/worktree-sessions/{session_id}".to_string(),
        ],
        max_running: 4,
        max_retained: 32,
        max_output_bytes: 65_536,
        max_runner_notices: Some(32),
        stdin: "null".to_string(),
        reasons: vec!["capacity".to_string()],
        event_types: vec![
            WorktreeSessionEventType::WorktreeSessionStarted,
            WorktreeSessionEventType::WorktreeSessionFinished,
        ],
    };
    let cases: Vec<(&str, Value)> = vec![
        ("CommandRecord", to_value(&launch)),
        ("CommandRecord", to_value(&stop)),
        ("WorktreeSessionCapability", to_value(&capability)),
        (
            "WorktreeSessionResponse",
            to_value(&WorktreeSessionResponse {
                instance_id: "instance-1".to_string(),
                state_revision: 3,
                session: session_resource(),
            }),
        ),
        (
            "WorktreeSessionsResponse",
            to_value(&WorktreeSessionsResponse {
                instance_id: "instance-1".to_string(),
                state_revision: 3,
                sessions: vec![session_resource()],
            }),
        ),
        ("ExecutionStatusResponse", to_value(&execution_status())),
    ];
    for (name, payload) in cases {
        let schema = &schemas(doc)[name];
        if schema.is_null() {
            errors.push(format!("{name} is served but is not published"));
            continue;
        }
        validate(&payload, schema, schemas(doc), name, &mut errors);
    }
    errors
}

#[test]
fn remote_worktree_session_contract_is_published_end_to_end() {
    let doc = generated();
    let errors = session_contract_errors(&doc);
    assert!(errors.is_empty(), "{errors:#?}");

    // The worktree capability names the session operation.
    assert!(conflux::web::remote_control_api::worktrees::WORKTREE_OPERATIONS.contains(&"session"));

    // Negative control: a copy missing one route and one result kind must fail,
    // and the failure must name exactly what is missing.
    let mut incomplete = doc.clone();
    incomplete["paths"]
        .as_object_mut()
        .unwrap()
        .remove("/api/v2/worktree-sessions/{session_id}")
        .expect("the route was published");
    incomplete["components"]["schemas"]["CommandResult"]["oneOf"]
        .as_array_mut()
        .unwrap()
        .retain(|variant| !variant.to_string().contains("worktree_session_stop"));
    let errors = session_contract_errors(&incomplete);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("/api/v2/worktree-sessions/{session_id}")),
        "{errors:#?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("worktree_session_stop")),
        "{errors:#?}"
    );
}

/// The exported document and the live one are the same bytes, and both carry
/// the session surface.
#[tokio::test]
async fn remote_worktree_session_cli_export_equals_the_live_document() {
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
        .unwrap();
    let served = String::from_utf8(
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let exported = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .arg("openapi")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("the cflx binary under test is runnable");
    assert!(exported.status.success());
    let exported = String::from_utf8(exported.stdout).unwrap();
    assert_eq!(exported, served);
    for needle in [
        "/api/v2/worktree-sessions/{session_id}",
        "run_worktree_command",
        "worktree_session_finished",
    ] {
        assert!(exported.contains(needle), "{needle}");
    }
}

// ============================================================================
// Public API contract gaps
// ============================================================================

/// Every member the six contract-gap fixes publish, as one predicate so the
/// same check can be run against a deliberately incomplete copy.
fn contract_gap_errors(doc: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let s = schemas(doc);
    let names = |schema: &Value, key: &str| -> BTreeSet<String> {
        schema[key]
            .as_array()
            .map(|r| {
                r.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .or_else(|| schema[key].as_object().map(|p| p.keys().cloned().collect()))
            .unwrap_or_default()
    };
    let description = |value: &Value| {
        value["description"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };

    if !published_enum(doc, "ErrorCode").contains("merge_hook_failed") {
        errors.push("the error code `merge_hook_failed` is not published".into());
    }
    let outcomes = published_enum(doc, "MergeAbortOutcome");
    for outcome in ["merged_hook_failed", "merge_failed"] {
        if !outcomes.contains(outcome) {
            errors.push(format!("MergeAbortOutcome lacks `{outcome}`"));
        }
    }
    if published_enum(doc, "MergeLanded")
        != BTreeSet::from(["confirmed", "not_landed", "unknown"].map(String::from))
    {
        errors.push("MergeLanded vocabulary is not confirmed/not_landed/unknown".into());
    }
    let landing = names(&s["MergeLandingEvidence"], "required");
    for member in [
        "landed",
        "head_after",
        "merge_head_present",
        "clean_after",
        "detail",
    ] {
        if !landing.contains(member) {
            errors.push(format!("MergeLandingEvidence must require `{member}`"));
        }
    }
    let hook = names(&s["MergeHookEvidence"], "required");
    for member in ["ran", "exit_ok", "detail"] {
        if !hook.contains(member) {
            errors.push(format!("MergeHookEvidence must require `{member}`"));
        }
    }
    let result_properties = names(&s["MergeAbortResult"], "properties");
    let result_required = names(&s["MergeAbortResult"], "required");
    for member in ["merge", "hook"] {
        if !result_properties.contains(member) {
            errors.push(format!("MergeAbortResult does not publish `{member}`"));
        }
        if result_required.contains(member) {
            errors.push(format!(
                "MergeAbortResult.{member} must decode as absent from an older owner"
            ));
        }
    }

    let session = &s["WorktreeSessionResource"];
    let session_properties = names(session, "properties");
    let session_required = names(session, "required");
    for member in ["runner_notices", "runner_notices_dropped"] {
        if !session_properties.contains(member) {
            errors.push(format!(
                "WorktreeSessionResource does not publish `{member}`"
            ));
        }
        if session_required.contains(member) {
            errors.push(format!(
                "WorktreeSessionResource.{member} must decode as absent from an older owner"
            ));
        }
    }
    let notice = names(&s["SessionRunnerNotice"], "required");
    for member in ["kind", "attempt", "detail"] {
        if !notice.contains(member) {
            errors.push(format!("SessionRunnerNotice must require `{member}`"));
        }
    }
    if !description(&session["properties"]["signal"]).contains("terminated the leader as reaped") {
        errors.push("WorktreeSessionResource.signal must describe the reaped signal".into());
    }
    if !description(&s["SessionOutputLine"]).contains("bytes the child wrote") {
        errors.push("SessionOutputLine must describe child-only bytes".into());
    }
    if !names(&s["WorktreeSessionCapability"], "properties").contains("max_runner_notices") {
        errors.push("WorktreeSessionCapability does not publish `max_runner_notices`".into());
    }

    if !published_enum(doc, "ConfirmedDeleteReason").contains("detached_head") {
        errors.push("ConfirmedDeleteReason lacks `detached_head`".into());
    }
    if !description(&s["ConfirmedDeleteCapability"]).contains("max_listing_name_bytes") {
        errors.push("the confirmed-delete budget does not name `max_listing_name_bytes`".into());
    }
    let preview_properties = names(&s["DeletePreviewResponse"], "properties");
    let preview_required = names(&s["DeletePreviewResponse"], "required");
    if !preview_properties.contains("is_detached") || preview_required.contains("is_detached") {
        errors.push("DeletePreviewResponse must publish an optional `is_detached`".into());
    }
    errors
}

#[test]
fn remote_contract_gap_contract_is_published_end_to_end() {
    use conflux::web::remote_control_api::dto::{
        MergeAbortOutcome, MergeAbortResult, MergeHookEvidence, MergeLanded, MergeLandingEvidence,
        SessionRunnerNotice,
    };

    let doc = generated();
    assert_eq!(contract_gap_errors(&doc), Vec::<String>::new());

    // What the server serializes is what the schema describes.
    let record = CommandRecord {
        command_type: "merge_worktree_abort_on_conflict".to_string(),
        state: CommandState::Failed,
        error_code: Some(ErrorCode::MergeHookFailed),
        result: Some(CommandResult::MergeAbort(Box::new(MergeAbortResult {
            worktree_id: "0123456789abcdef0123456789abcdef".to_string(),
            branch: "alpha".to_string(),
            outcome: MergeAbortOutcome::MergedHookFailed,
            premerge_head: Some("a".repeat(40)),
            conflict_files: Vec::new(),
            abort: None,
            recovery: Some("local_or_tui_required".to_string()),
            merge: Some(MergeLandingEvidence {
                landed: MergeLanded::Confirmed,
                head_after: Some("b".repeat(40)),
                merge_head_present: Some(false),
                clean_after: Some(true),
                detail: None,
            }),
            hook: Some(MergeHookEvidence {
                ran: true,
                exit_ok: Some(false),
                detail: Some("hook exited 1".to_string()),
            }),
        }))),
        ..command_record()
    };
    let mut session = session_resource();
    session.signal = Some("SIGKILL".to_string());
    session.runner_notices = Some(vec![SessionRunnerNotice {
        kind: "retry_scheduled".to_string(),
        attempt: Some(2),
        detail: Some("cause=crash, max_attempts=2, delay_ms=0".to_string()),
    }]);
    let mut errors = Vec::new();
    for (name, payload) in [
        ("CommandRecord", to_value(&record)),
        ("WorktreeSessionResource", to_value(&session)),
    ] {
        validate(
            &payload,
            &schemas(&doc)[name],
            schemas(&doc),
            name,
            &mut errors,
        );
    }
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(to_value(&record)["error_code"], "merge_hook_failed");
    assert_eq!(to_value(&record)["result"]["merge"]["landed"], "confirmed");

    // Negative controls: removing any one published member is noticed.
    type Removal = Box<dyn Fn(&mut Value)>;
    let removals: Vec<(&str, Removal)> = vec![
        (
            "merge_hook_failed",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["ErrorCode"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c != "merge_hook_failed")
            }),
        ),
        (
            "merged_hook_failed",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["MergeAbortOutcome"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c != "merged_hook_failed")
            }),
        ),
        (
            "merge_failed",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["MergeAbortOutcome"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c != "merge_failed")
            }),
        ),
        (
            "MergeLandingEvidence",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]
                    .as_object_mut()
                    .unwrap()
                    .remove("MergeLandingEvidence");
            }),
        ),
        (
            "MergeHookEvidence",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]
                    .as_object_mut()
                    .unwrap()
                    .remove("MergeHookEvidence");
            }),
        ),
        (
            "runner_notices",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["WorktreeSessionResource"]["properties"]
                    .as_object_mut()
                    .unwrap()
                    .remove("runner_notices");
            }),
        ),
        (
            "SessionRunnerNotice",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]
                    .as_object_mut()
                    .unwrap()
                    .remove("SessionRunnerNotice");
            }),
        ),
        (
            "signal",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["WorktreeSessionResource"]["properties"]["signal"]
                    ["description"] = json!("Terminating signal name.");
            }),
        ),
        (
            "SessionOutputLine",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["SessionOutputLine"]["description"] =
                    json!("One output line.");
            }),
        ),
        (
            "detached_head",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["ConfirmedDeleteReason"]["enum"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c != "detached_head")
            }),
        ),
        (
            "max_listing_name_bytes",
            Box::new(|d: &mut Value| {
                d["components"]["schemas"]["ConfirmedDeleteCapability"]["description"] =
                    json!("Confirmed delete.");
            }),
        ),
    ];
    for (what, remove) in removals {
        let mut copy = doc.clone();
        remove(&mut copy);
        assert_ne!(copy, doc, "{what}: the removal changed nothing");
        assert!(
            !contract_gap_errors(&copy).is_empty(),
            "removing {what} was not noticed"
        );
    }
}

/// Payloads in the shape an owner built at `10da8ac45` serializes: the same
/// DTOs without any member this change adds. Each decodes with the new members
/// absent.
#[test]
fn remote_contract_gap_contract_old_owner_payloads_decode() {
    use conflux::web::remote_control_api::dto::{
        MergeAbortOutcome, WorktreeSessionCapability, WorktreeSessionResource,
    };
    use conflux::web::remote_control_api::worktrees::{
        ConfirmedDeleteCapability, DeletePreviewResponse,
    };

    let merge_abort: CommandResult = serde_json::from_value(json!({
        "kind": "merge_abort",
        "worktree_id": "0123456789abcdef0123456789abcdef",
        "branch": "alpha",
        "outcome": "abort_failed",
        "premerge_head": "a".repeat(40),
        "conflict_files": ["src/lib.rs"],
        "abort": {
            "exit_ok": false,
            "head_after": "a".repeat(40),
            "merge_head_present": true,
            "clean_after": null,
            "restored": "unknown",
            "detail": "fatal: injected"
        },
        "recovery": "local_or_tui_required"
    }))
    .expect("an older merge_abort result decodes");
    let CommandResult::MergeAbort(result) = merge_abort else {
        panic!("decoded as another kind");
    };
    assert_eq!(result.outcome, MergeAbortOutcome::AbortFailed);
    assert_eq!(result.merge, None);
    assert_eq!(result.hook, None);

    let session: WorktreeSessionResource = serde_json::from_value(json!({
        "session_id": "a".repeat(32),
        "instance_id": "instance-1",
        "kind": "existing",
        "worktree_id": "b".repeat(32),
        "path": "../ws/alpha",
        "branch": "alpha",
        "state": "stopped",
        "attempt": 1,
        "started_at": "2026-08-04T00:00:00Z",
        "finished_at": "2026-08-04T00:00:05Z",
        "exit_code": null,
        "signal": "SIGTERM",
        "stop_requested": true,
        "setup": null,
        "cleanup": null,
        "output": {"lines": [], "retained_bytes": 0, "truncated_bytes": 0}
    }))
    .expect("an older session resource decodes");
    assert_eq!(session.runner_notices, None, "unpublished, not empty");
    assert_eq!(session.runner_notices_dropped, None);

    let capability: WorktreeSessionCapability = serde_json::from_value(json!({
        "available": true,
        "commands": ["run_worktree_command"],
        "routes": ["/api/v2/worktree-sessions"],
        "max_running": 4,
        "max_retained": 32,
        "max_output_bytes": 65536,
        "stdin": "null",
        "reasons": ["capacity"],
        "event_types": ["worktree_session_started", "worktree_session_finished"]
    }))
    .expect("an older session capability decodes");
    assert_eq!(capability.max_runner_notices, None);

    let confirmed: ConfirmedDeleteCapability = serde_json::from_value(json!({
        "available": true,
        "command": "delete_worktree_confirmed",
        "preview_route": "/api/v2/worktrees/{worktree_id}/delete-preview",
        "permissions": ["skip_teardown", "discard_known_dirty", "discard_commits_ahead"],
        "consent_fields": ["branch", "head", "dirty", "has_commits_ahead",
                           "has_ignored_content", "merge_base",
                           "discardable_content_id", "ignored_content_id"],
        "content_id_algorithm": "sha256",
        "binds_ignored_content": true,
        "reasons": ["branch_changed", "teardown_failed"]
    }))
    .expect("an older confirmed-delete capability decodes");
    assert!(!confirmed.reasons.iter().any(|r| r == "detached_head"));

    let option = json!({"eligible": false, "blocked_reason": "ahead_unknown"});
    let preview: DeletePreviewResponse = serde_json::from_value(json!({
        "instance_id": "instance-1",
        "state_revision": 3,
        "worktree_id": "b".repeat(32),
        "observed_at": "2026-08-04T00:00:00Z",
        "branch": "",
        "head": "a".repeat(40),
        "is_main": false,
        "dirty": false,
        "has_commits_ahead": null,
        "inspection": "not_inspected",
        "content_observation": "observed",
        "has_ignored_content": false,
        "ignored_entries": 0,
        "teardown_script_present": false,
        "options": {
            "ordinary": option,
            "skip_teardown": option,
            "discard_known_dirty": option,
            "discard_commits_ahead": option
        },
        "consent": {
            "branch": "",
            "head": "a".repeat(40),
            "dirty": false,
            "has_commits_ahead": null,
            "has_ignored_content": false,
            "merge_base": null,
            "discardable_content_id": null,
            "ignored_content_id": null
        }
    }))
    .expect("an older preview decodes");
    assert!(!preview.is_detached, "absent decodes as attached");
}

// ============================================================================
// Session stop during a retry delay: one more notice kind, nothing else
// ============================================================================

/// The generated notice description names the refusal kind and its detail.
#[test]
fn session_retry_stop_contract_documents_the_refusal_kind() {
    let doc = generated();
    let notice = &schemas(&doc)["SessionRunnerNotice"]["properties"];
    let kind = notice["kind"]["description"].as_str().unwrap_or_default();
    assert!(
        kind.contains("stop_refused_attempt"),
        "SessionRunnerNotice.kind must document stop_refused_attempt: {kind}"
    );
    for existing in ["retry_scheduled", "shutdown_refused"] {
        assert!(
            kind.contains(existing),
            "{existing} is still documented: {kind}"
        );
    }
    let detail = notice["detail"]["description"].as_str().unwrap_or_default();
    assert!(
        detail.contains("stage=retry"),
        "SessionRunnerNotice.detail documents the refusal stage: {detail}"
    );
    assert_eq!(
        notice["kind"]["type"], "string",
        "kind stays a free string, not an enum"
    );
}

/// A session resource in the shape an owner built at `f64f34d7b` serializes
/// decodes unchanged and re-serializes to the same value: no member was added
/// or moved, only a documented notice kind.
#[test]
fn session_retry_stop_contract_old_owner_session_decodes() {
    use conflux::web::remote_control_api::dto::WorktreeSessionResource;

    let published = json!({
        "session_id": "a".repeat(32),
        "instance_id": "instance-1",
        "kind": "existing",
        "worktree_id": "b".repeat(32),
        "path": "../ws/alpha",
        "branch": "alpha",
        "state": "stopped",
        "attempt": 2,
        "started_at": "2026-11-05T00:00:00Z",
        "finished_at": "2026-11-05T00:00:05Z",
        "exit_code": null,
        "signal": "SIGTERM",
        "stop_requested": true,
        "setup": null,
        "cleanup": null,
        "output": {
            "lines": [{"stream": "stdout", "text": "first"}],
            "retained_bytes": 5,
            "truncated_bytes": 0
        },
        "runner_notices": [{
            "kind": "retry_scheduled",
            "attempt": 2,
            "detail": "cause=crash, max_attempts=3, delay_ms=60000"
        }],
        "runner_notices_dropped": 0
    });
    let session: WorktreeSessionResource =
        serde_json::from_value(published.clone()).expect("an f64f34d7b session decodes");
    assert_eq!(session.attempt, 2);
    assert_eq!(
        serde_json::to_value(&session).unwrap(),
        published,
        "nothing else moved"
    );
}
