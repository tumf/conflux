//! Contract assertions against the canonical tracked OpenAPI artifact.
//!
//! `docs/openapi.yaml` is the one artifact a generated consumer is allowed to
//! read, so this file behaves like that consumer: it loads the *tracked file*
//! rather than the in-memory document wherever the question is "would a client
//! reading the repository get the right answer?".
//!
//! Two independent properties are enforced here, and they fail for different
//! reasons on purpose:
//!
//! * **The artifact is current.** Regenerating must reproduce the tracked bytes
//!   exactly. `make check-openapi` proves this from a shell too; the test proves
//!   it inside `cargo test` so a plain `make test` cannot miss it.
//! * **The artifact is complete.** Every supported route, command variant, error
//!   code, event category, security declaration, and required response field is
//!   present — and nothing removed has crept back in. A regenerated-but-wrong
//!   artifact passes the first property and fails this one.
//!
//! The payload checks are the "generated consumer compiles" property expressed in
//! Rust: the DTO values below are constructed field by field from the real types,
//! so adding a field to a published response forces this file to change, and the
//! structural validator then proves the published schema describes what those
//! types actually serialize to.

#![cfg(feature = "web-monitoring")]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use conflux::web::openapi::{
    document_yaml, BEARER_SCHEME, GENERATED_BANNER, SUPPORTED_V2_PATHS, UNAUTHENTICATED_V2_PATHS,
};
use conflux::web::remote_control_api::dto::{
    ActionEligibility, ApiError, AttentionState, ChangeActions, ChangeActivity, ChangeBlocker,
    ChangeResource, ChangeTiming, ChangeWorktree, CommandRecord, CommandState, ErrorCode,
    EventCategory, EventEnvelope, InstanceSnapshot, ParallelEligibility, ParallelRuntimeState,
    QueueIntent, SnapshotTotals, StateResponse, ALL_ERROR_CODES, SUPPORTED_COMMANDS,
};
use conflux::web::remote_control_api::worktrees::{
    WorktreeConflict, WorktreeEligibility, WorktreeResource, WorktreeResponse,
};

/// The path a consumer is told to read, relative to the repository root.
const CANONICAL_ARTIFACT: &str = "docs/openapi.yaml";

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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn artifact_text() -> String {
    let path = repo_root().join(CANONICAL_ARTIFACT);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("canonical artifact {} is unreadable: {e}", path.display()))
}

/// The tracked artifact parsed as JSON, which is what a consumer works with.
fn artifact() -> Value {
    serde_yaml::from_str(&artifact_text()).expect("canonical artifact must be valid YAML")
}

fn schemas(doc: &Value) -> &Value {
    &doc["components"]["schemas"]
}

// ============================================================================
// The artifact is current
// ============================================================================

#[test]
fn tracked_artifact_matches_the_generated_document() {
    assert_eq!(
        artifact_text(),
        document_yaml(),
        "{CANONICAL_ARTIFACT} is stale. Run `make openapi` and commit the result."
    );
}

#[test]
fn generation_is_byte_for_byte_deterministic() {
    assert_eq!(
        document_yaml(),
        document_yaml(),
        "OpenAPI generation must not depend on iteration or hashing order"
    );
}

#[test]
fn the_artifact_declares_itself_generated() {
    assert!(
        artifact_text().starts_with(GENERATED_BANNER),
        "{CANONICAL_ARTIFACT} must open with the generated-file banner so it is never hand-edited"
    );
}

/// Everything else here proves the artifact is *currently* right. This proves
/// the check would notice if it stopped being right — the one property a suite
/// of positive assertions can never establish about itself.
///
/// A scratch copy of the artifact is drifted by dropping a required field, then
/// run through the exact predicate `make check-openapi` uses (`diff -u tracked
/// generated`, whose exit status is the target's pass/fail). The failure has to
/// name the field that moved, because a diff that does not is not a useful
/// error message. Regenerating over the fixture — the byte-for-byte effect of
/// `make openapi` — then has to clear it.
///
/// The fixture is a private copy in a scratch directory: the check must be
/// provable without the tracked artifact ever being written to.
#[test]
fn the_drift_check_fails_on_a_mutated_schema_and_passes_after_regeneration() {
    let dir = scratch_dir("drift-fixture");
    let tracked = dir.join("openapi.yaml");
    let generated = dir.join("generated.yaml");
    std::fs::write(&generated, document_yaml()).expect("scratch generated artifact");

    // An omitted required field is the drift a reviewer is least likely to
    // catch by eye and a generated client is most likely to trust.
    let drifted = without_required_event_sequence(&artifact_text());
    assert_ne!(drifted, document_yaml(), "the fixture must actually drift");
    std::fs::write(&tracked, &drifted).expect("scratch drifted artifact");

    let (matches, diff) = unified_diff(&tracked, &generated);
    assert!(
        !matches,
        "the drift check accepted an artifact whose EventEnvelope no longer requires event_sequence"
    );
    assert!(
        diff.contains("event_sequence"),
        "the drift failure must name what moved, got:\n{diff}"
    );

    std::fs::write(&tracked, document_yaml()).expect("regenerate scratch artifact");
    let (matches, diff) = unified_diff(&tracked, &generated);
    assert!(matches, "regeneration must clear the drift, got:\n{diff}");

    std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
}

/// The comparison `check-openapi` performs, run the same way the target runs it.
fn unified_diff(tracked: &Path, generated: &Path) -> (bool, String) {
    let output = std::process::Command::new("diff")
        .arg("-u")
        .arg(tracked)
        .arg(generated)
        .output()
        .expect("diff(1) is available; check-openapi depends on it too");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cflx-openapi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory is creatable");
    dir
}

/// Drop `event_sequence` from `EventEnvelope`'s required list, touching no other
/// byte so the resulting diff is exactly the one line that drifted.
fn without_required_event_sequence(text: &str) -> String {
    const SCHEMA: &str = "\n    EventEnvelope:\n";
    const ENTRY: &str = "\n      - event_sequence\n";
    let schema_at = text
        .find(SCHEMA)
        .expect("EventEnvelope is a published schema");
    let entry_at = text[schema_at..]
        .find(ENTRY)
        .map(|offset| schema_at + offset)
        .expect("EventEnvelope requires event_sequence");
    let mut drifted = String::with_capacity(text.len());
    drifted.push_str(&text[..=entry_at]);
    drifted.push_str(&text[entry_at + ENTRY.len()..]);
    drifted
}

#[test]
fn the_repository_tracks_exactly_one_openapi_artifact() {
    // A second file cannot be kept honest, and a stale one is worse than none
    // because a generated consumer will believe it.
    let stray = repo_root().join("openapi.yaml");
    assert!(
        !stray.exists(),
        "{} duplicates the canonical contract; {CANONICAL_ARTIFACT} is the only OpenAPI artifact",
        stray.display()
    );

    let mut found = Vec::new();
    collect_openapi_files(&repo_root(), 0, &mut found);
    assert_eq!(
        found,
        vec![repo_root().join(CANONICAL_ARTIFACT)],
        "exactly one tracked OpenAPI artifact is allowed"
    );
}

/// Walk the working tree for OpenAPI-looking files, skipping build and vendor
/// output that is not part of the tracked contract.
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
// The route surface
// ============================================================================

#[test]
fn the_artifact_publishes_exactly_the_supported_route_surface() {
    let doc = artifact();
    let published: BTreeSet<String> = doc["paths"]
        .as_object()
        .expect("paths object")
        .keys()
        .cloned()
        .collect();
    let supported: BTreeSet<String> = SUPPORTED_V2_PATHS.iter().map(|p| p.to_string()).collect();

    assert_eq!(
        published, supported,
        "the published paths and the declared v2 route surface disagree"
    );
}

#[tokio::test]
async fn every_published_path_is_bound_with_the_authentication_it_declares() {
    // Closes the third side of the triangle: the constant and the artifact could
    // agree with each other and still describe a route the server never binds.
    //
    // The probe is credential-free against a token-protected instance, which
    // separates the two answers cleanly. A gated route refuses with 401 before
    // any handler runs, so a bound route cannot be mistaken for an unbound one
    // just because its handler happens to answer 404 for a missing resource;
    // an unmatched path never reaches the gate at all and falls through to 404.
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

#[test]
fn no_removed_path_is_presented_as_supported() {
    let doc = artifact();
    let paths = doc["paths"].as_object().expect("paths object");
    for removed in REMOVED_PATHS {
        assert!(
            !paths.contains_key(*removed),
            "{removed} was removed from this server but is still published as supported API"
        );
    }
    for path in paths.keys() {
        assert!(
            path.starts_with("/api/v2/"),
            "{path} is outside the only namespace this process serves"
        );
    }
}

// ============================================================================
// Security declarations
// ============================================================================

#[test]
fn bearer_authentication_is_declared_and_scoped() {
    let doc = artifact();

    let scheme = &doc["components"]["securitySchemes"][BEARER_SCHEME];
    assert_eq!(scheme["type"], "http", "bearer scheme must be declared");
    assert_eq!(scheme["scheme"], "bearer");
    let description = scheme["description"].as_str().unwrap_or_default();
    assert!(
        description.contains("Authorization"),
        "the scheme must name the header credentials are accepted in"
    );

    let global = doc["security"]
        .as_array()
        .expect("a document-wide security requirement");
    assert!(
        global.iter().any(|req| req.get(BEARER_SCHEME).is_some()),
        "the document default must require {BEARER_SCHEME}"
    );

    let paths = doc["paths"].as_object().expect("paths object");
    for (path, item) in paths {
        for (method, operation) in item.as_object().expect("path item") {
            let declared = operation.get("security").and_then(Value::as_array);
            if UNAUTHENTICATED_V2_PATHS.contains(&path.as_str()) {
                assert_eq!(
                    declared.map(Vec::len),
                    Some(0),
                    "{method} {path} is unauthenticated and must override the default with `security: []`"
                );
            } else {
                assert!(
                    declared.is_none(),
                    "{method} {path} must inherit the document-wide bearer requirement"
                );
            }
        }
    }
}

// ============================================================================
// Closed vocabularies
// ============================================================================

#[test]
fn the_command_schema_covers_exactly_the_supported_command_set() {
    let doc = artifact();
    let variants = schemas(&doc)["CommandSpec"]["oneOf"]
        .as_array()
        .expect("CommandSpec must be a discriminated union");

    let published: BTreeSet<String> = variants
        .iter()
        .map(|variant| {
            variant["properties"]["type"]["enum"][0]
                .as_str()
                .expect("each variant must pin its `type` discriminant")
                .to_string()
        })
        .collect();
    let supported: BTreeSet<String> = SUPPORTED_COMMANDS.iter().map(|c| c.to_string()).collect();

    assert_eq!(
        published, supported,
        "the published command union and the advertised command set disagree"
    );
}

#[test]
fn every_command_variant_requires_its_discriminant() {
    let doc = artifact();
    for variant in schemas(&doc)["CommandSpec"]["oneOf"]
        .as_array()
        .expect("CommandSpec union")
    {
        let required: Vec<&str> = variant["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        assert!(
            required.contains(&"type"),
            "a command variant that does not require `type` cannot be dispatched: {variant}"
        );
    }
}

#[test]
fn the_error_code_enum_covers_every_typed_error() {
    let doc = artifact();
    let published: BTreeSet<String> = schemas(&doc)["ErrorCode"]["enum"]
        .as_array()
        .expect("ErrorCode enum")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let known: BTreeSet<String> = ALL_ERROR_CODES
        .iter()
        .map(|c| c.as_str().to_string())
        .collect();

    assert_eq!(
        published, known,
        "the published error vocabulary and the codes the server can emit disagree"
    );
}

#[test]
fn command_outcome_and_event_vocabularies_are_complete() {
    let doc = artifact();

    let states: BTreeSet<String> = schemas(&doc)["CommandState"]["enum"]
        .as_array()
        .expect("CommandState enum")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let known_states: BTreeSet<String> = [
        CommandState::Running,
        CommandState::Succeeded,
        CommandState::NoOp,
        CommandState::Failed,
    ]
    .iter()
    .map(|s| {
        serde_json::to_value(s)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    })
    .collect();
    assert_eq!(
        states, known_states,
        "command outcomes must all be published"
    );

    let categories: BTreeSet<String> = schemas(&doc)["EventCategory"]["enum"]
        .as_array()
        .expect("EventCategory enum")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let known_categories: BTreeSet<String> =
        [EventCategory::State, EventCategory::Log, EventCategory::Gap]
            .iter()
            .map(|c| {
                serde_json::to_value(c)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
    assert_eq!(
        categories, known_categories,
        "event categories must all be published, including the replay `gap`"
    );
    assert!(
        known_categories.contains("gap"),
        "the replay-gap category is what tells a client to resnapshot"
    );
}

// ============================================================================
// Schema integrity
// ============================================================================

#[test]
fn every_reference_resolves_and_no_schema_is_unreachable() {
    let doc = artifact();
    let defined: BTreeSet<String> = schemas(&doc)
        .as_object()
        .expect("components.schemas")
        .keys()
        .cloned()
        .collect();

    let mut referenced = BTreeSet::new();
    collect_refs(&doc, &mut referenced);

    let dangling: Vec<&String> = referenced.difference(&defined).collect();
    assert!(
        dangling.is_empty(),
        "these schemas are referenced but never defined: {dangling:?}"
    );

    let orphans: Vec<&String> = defined.difference(&referenced).collect();
    assert!(
        orphans.is_empty(),
        "these schemas are defined but unreachable from any operation: {orphans:?}"
    );
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

// ============================================================================
// The generated consumer
// ============================================================================

#[test]
fn published_schemas_describe_what_the_server_actually_serializes() {
    let doc = artifact();

    // Each pair is a schema name and a value produced by the real DTO types.
    // Constructing them field by field is the compile-time half of the check:
    // a new published field cannot land without this file being updated.
    let cases: Vec<(&str, Value)> = vec![
        ("StateResponse", to_value(&state_response())),
        ("CommandRecord", to_value(&command_record())),
        ("WorktreeResponse", to_value(&worktree_response())),
        ("EventEnvelope", to_value(&event_envelope())),
        ("ApiError", to_value(&api_error())),
    ];

    for (name, payload) in cases {
        let schema = &schemas(&doc)[name];
        assert!(
            !schema.is_null(),
            "{name} is served by this API but is not published"
        );
        let mut errors = Vec::new();
        validate(&payload, schema, schemas(&doc), name, &mut errors);
        assert!(
            errors.is_empty(),
            "{name} does not match its published schema:\n  {}",
            errors.join("\n  ")
        );
    }
}

#[test]
fn the_authoritative_snapshot_publishes_every_operator_field() {
    // The snapshot is the field set a controller decides from, so a silently
    // dropped member is the drift most likely to go unnoticed: the payload still
    // validates, it just says less.
    let doc = artifact();
    let payload = to_value(&state_response());
    let change = &payload["snapshot"]["changes"][0];

    let published: BTreeSet<&str> = schemas(&doc)["ChangeResource"]["properties"]
        .as_object()
        .expect("ChangeResource properties")
        .keys()
        .map(String::as_str)
        .collect();
    let serialized: BTreeSet<&str> = change
        .as_object()
        .expect("serialized change")
        .keys()
        .map(String::as_str)
        .collect();

    assert!(
        serialized.is_subset(&published),
        "the server serializes change fields the contract omits: {:?}",
        serialized.difference(&published).collect::<Vec<_>>()
    );

    for expected in [
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
    ] {
        assert!(
            published.contains(expected),
            "the authoritative snapshot must publish `{expected}`"
        );
    }
}

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
            is_resolving: false,
            process_error: Some("sanitized process failure".to_string()),
            parallel: ParallelRuntimeState {
                mode: conflux::web::remote_control_api::dto::ParallelMode::Parallel,
                available: true,
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
