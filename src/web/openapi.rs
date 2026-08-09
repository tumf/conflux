//! The single source of truth for the `/api/v2` contract.
//!
//! The contract is generated from this module and never tracked as a file.
//! `cflx openapi` writes it to stdout and `GET /api/v2/openapi.yaml` serves it,
//! both through [`document_yaml`], so a consumer reading either one is reading
//! this module. Nothing in the repository is allowed to describe the API
//! separately: a hand-written second copy cannot be kept honest, and a stale one
//! is worse than none because a generated consumer will believe it.
//!
//! `tests/openapi_contract_tests.rs` holds the generated document against the
//! executable surface, so a route, DTO field, command variant, error code, event
//! envelope, or security declaration cannot change without the contract
//! assertions changing with it.

use utoipa::openapi::path::{HttpMethod, OperationBuilder};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{ResponseBuilder, ResponsesBuilder};
use utoipa::{Modify, OpenApi};

/// Security scheme name referenced by every authenticated operation.
pub const BEARER_SCHEME: &str = "bearer_token";

/// Every path this process serves under `/api/v2`.
///
/// This is the enumerated route surface the contract tests hold the router and
/// the generated document against, so a route can never exist in only two of the
/// three places. Keep it in the order the paths are registered in
/// [`crate::web::remote_control_api::router`].
pub const SUPPORTED_V2_PATHS: &[&str] = &[
    "/api/v2/health",
    "/api/v2/capabilities",
    "/api/v2/instance",
    "/api/v2/state",
    "/api/v2/execution-status",
    "/api/v2/changes",
    "/api/v2/changes/{change_id}",
    "/api/v2/logs",
    "/api/v2/worktrees",
    "/api/v2/worktrees/{worktree_id}",
    "/api/v2/commands",
    "/api/v2/commands/{command_id}",
    "/api/v2/events",
    "/api/v2/ws",
    "/api/v2/openapi.yaml",
    "/api/v2/openapi.json",
    "/api/v2/docs",
];

/// The v2 paths that are served without bearer credentials.
///
/// Health is unauthenticated so an operator can tell "down" apart from
/// "misconfigured credentials", and the contract-discovery routes are
/// unauthenticated because they describe the API rather than expose any instance
/// state. Anything not listed here is behind the bearer gate.
pub const UNAUTHENTICATED_V2_PATHS: &[&str] = &[
    "/api/v2/health",
    "/api/v2/openapi.yaml",
    "/api/v2/openapi.json",
    "/api/v2/docs",
];

/// Whether a served path is exempt from bearer enforcement.
///
/// Exemption is *only* from the bearer check. Every other obligation the gate
/// carries — origin policy, preflight handling, and the refusal of credentials
/// smuggled through a query string or a WebSocket subprotocol — applies to these
/// paths like any other, which is what lets the published contract say
/// "on every path" and mean it.
///
/// [`UNAUTHENTICATED_V2_PATHS`] is the published list; the Swagger UI also serves
/// its own static assets beneath `/api/v2/docs/`, which are part of the same
/// contract-discovery page and must load without a token for it to render at all.
pub fn is_unauthenticated_v2_path(path: &str) -> bool {
    UNAUTHENTICATED_V2_PATHS.contains(&path) || path.starts_with("/api/v2/docs/")
}

/// Long-form contract semantics a generated consumer cannot infer from schemas.
const API_DESCRIPTION: &str = "\
Remote control of one running `cflx` process incarnation.

**Authentication.** When a token is configured, every route except
`/api/v2/health` and the contract-discovery routes requires
`Authorization: Bearer <token>`. Credentials are accepted *only* in that header:
a token in a query string or a WebSocket subprotocol is rejected rather than
honored, on every path, so a client cannot learn a leaky habit on one route.
`GET /api/v2/capabilities` reports whether authentication is enforced at all.

**Process incarnation.** `instance_id` identifies one process lifetime.
`state_revision`, `event_sequence`, and every command record are scoped to it and
are gone after a restart. A client that sees a new `instance_id` must discard its
cursor and re-snapshot from `GET /api/v2/state`.

**Streaming.** `GET /api/v2/events` is Server-Sent Events, but a browser must
read it with `fetch()` response streaming: native `EventSource` cannot attach the
bearer header. `GET /api/v2/ws` is for non-browser clients, since browsers cannot
set request headers on a WebSocket handshake.

**Replay gaps.** Resume with `after_sequence` plus the `instance_id` the cursor
belongs to. If the requested cursor has fallen out of the retained ring, or names
another incarnation, the stream emits a `gap` envelope; the client must re-read
`GET /api/v2/state` rather than assume continuity.

**Revisions and idempotency.** Every command is side-effecting, so
`expected_revision` and `idempotency_key` are both mandatory. A stale
`expected_revision` fails with `stale_revision`. Replaying a key bound to a
different typed command identity fails with `idempotency_mismatch`; replaying the
same identity returns the original record instead of acting twice.

**Worktree safety.** Worktrees are addressed only by the opaque process-local
`worktree_id` handed out by a v2 read. Paths, branches, and base commits are
server-derived and are rejected as mutation input, so no client can steer where a
worktree lands or what it is cut from.";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Conflux Remote Control API",
        version = env!("CARGO_PKG_VERSION"),
        description = API_DESCRIPTION
    ),
    modifiers(&SecurityAddon, &ContractRoutesAddon),
    security(("bearer_token" = [])),
    paths(
        crate::web::remote_control_api::reads::health,
        crate::web::remote_control_api::reads::capabilities,
        crate::web::remote_control_api::reads::instance,
        crate::web::remote_control_api::reads::state,
        crate::web::remote_control_api::reads::execution_status,
        crate::web::remote_control_api::reads::list_changes,
        crate::web::remote_control_api::reads::get_change,
        crate::web::remote_control_api::reads::logs,
        crate::web::remote_control_api::reads::list_worktrees,
        crate::web::remote_control_api::reads::get_worktree,
        crate::web::remote_control_api::commands::submit_command,
        crate::web::remote_control_api::commands::get_command,
        crate::web::remote_control_api::stream::events,
        crate::web::remote_control_api::stream::ws,
        crate::web::remote_control_api::openapi_yaml,
    ),
    components(
        schemas(
            crate::web::remote_control_api::dto::ActionBlockedReason,
            crate::web::remote_control_api::dto::ActionEligibility,
            crate::web::remote_control_api::dto::ApiError,
            crate::web::remote_control_api::dto::AttentionState,
            crate::web::remote_control_api::dto::BlockerKind,
            crate::web::remote_control_api::dto::ChangeActions,
            crate::web::remote_control_api::dto::ChangeActivity,
            crate::web::remote_control_api::dto::ChangeBlocker,
            crate::web::remote_control_api::dto::ChangeTiming,
            crate::web::remote_control_api::dto::ChangeWorktree,
            crate::web::remote_control_api::dto::ErrorCode,
            // Execution observability: the closed vocabularies and the resource
            // that joins them.
            crate::web::remote_control_api::dto::ApplyCommitEvidence,
            crate::web::remote_control_api::dto::ChangeExecutionState,
            crate::web::remote_control_api::dto::ChangeExecutionStatus,
            crate::web::remote_control_api::dto::CommandResult,
            crate::web::remote_control_api::dto::ExecutionPhase,
            crate::web::remote_control_api::dto::ExecutionStatusResponse,
            crate::web::remote_control_api::dto::LatestLogProjection,
            crate::web::remote_control_api::dto::ProcessExecutionStatus,
            crate::web::remote_control_api::dto::ParallelBlockedReason,
            crate::web::remote_control_api::dto::ParallelCapabilities,
            crate::web::remote_control_api::dto::ParallelEligibility,
            crate::web::remote_control_api::dto::ParallelRuntimeState,
            crate::web::remote_control_api::dto::QueueIntent,
            crate::web::remote_control_api::dto::CapabilitiesResponse,
            crate::web::remote_control_api::dto::CapabilityLimits,
            crate::web::remote_control_api::dto::ChangeResource,
            crate::web::remote_control_api::dto::ChangeResponse,
            crate::web::remote_control_api::dto::ChangesResponse,
            crate::web::remote_control_api::dto::CommandRecord,
            crate::web::remote_control_api::dto::CommandRequest,
            crate::web::remote_control_api::dto::CommandSpec,
            crate::web::remote_control_api::dto::CommandState,
            crate::web::remote_control_api::dto::ChangeTarget,
            crate::web::remote_control_api::dto::WorktreeTarget,
            crate::web::remote_control_api::dto::EmptyParams,
            crate::events::LogLevel,
            crate::web::remote_control_api::dto::EventCategory,
            crate::web::remote_control_api::dto::EventEnvelope,
            crate::web::remote_control_api::dto::HealthResponse,
            crate::web::remote_control_api::dto::InstanceResponse,
            crate::web::remote_control_api::dto::InstanceSnapshot,
            crate::web::remote_control_api::dto::LogsResponse,
            crate::web::remote_control_api::dto::SnapshotTotals,
            crate::web::remote_control_api::dto::StateResponse,
            crate::web::remote_control_api::dto::TransportDescriptor,
            crate::web::remote_control_api::worktrees::WorktreeCapabilities,
            crate::web::remote_control_api::worktrees::WorktreeConflict,
            crate::web::remote_control_api::worktrees::WorktreeEligibility,
            crate::web::remote_control_api::worktrees::WorktreeResource,
            crate::web::remote_control_api::worktrees::WorktreeResponse,
            crate::web::remote_control_api::worktrees::WorktreesResponse,
        )
    ),
    tags(
        (name = "remote-control", description = "Single-instance remote-control API"),
        (name = "contract", description = "Unauthenticated contract discovery for this build")
    )
)]
pub struct ApiDoc;

/// Declares the bearer scheme the global `security` requirement refers to.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // `components` already exists because schemas are registered, but the
        // addon must not depend on that ordering.
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            BEARER_SCHEME,
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "Bearer token in the `Authorization` header. Credentials are \
                         refused anywhere else, including query strings and WebSocket \
                         subprotocols. Absent when the instance runs without a \
                         configured token; `GET /api/v2/capabilities` reports which.",
                    ))
                    .build(),
            ),
        );
    }
}

/// Declares the contract-discovery routes that `utoipa-swagger-ui` serves.
///
/// `/api/v2/openapi.yaml` has a handler of ours and carries a `#[utoipa::path]`
/// like every other route. The JSON document and the Swagger UI page are served
/// by `utoipa-swagger-ui`'s own router, so there is no function here to attach an
/// attribute to — they are declared explicitly instead, because a reachable v2
/// route that the artifact does not mention is exactly the drift this contract
/// exists to prevent.
struct ContractRoutesAddon;

impl Modify for ContractRoutesAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let json = OperationBuilder::new()
            .tag("contract")
            .operation_id(Some("openapi_json"))
            .summary(Some("This document as JSON."))
            .description(Some(
                "Served by the embedded Swagger UI router. Unauthenticated: it \
                 describes the API and exposes no instance state.",
            ))
            .securities(Some(Vec::new()))
            .responses(
                ResponsesBuilder::new()
                    .response(
                        "200",
                        ResponseBuilder::new().description("OpenAPI document (JSON)"),
                    )
                    .build(),
            )
            .build();

        let docs = OperationBuilder::new()
            .tag("contract")
            .operation_id(Some("openapi_docs"))
            .summary(Some("Swagger UI for this build."))
            .description(Some(
                "Human-facing HTML console for the contract. Unauthenticated; \
                 requests it issues still need a bearer token of their own.",
            ))
            .securities(Some(Vec::new()))
            .responses(
                ResponsesBuilder::new()
                    .response("200", ResponseBuilder::new().description("Swagger UI page"))
                    .build(),
            )
            .build();

        openapi
            .paths
            .add_path_operation("/api/v2/openapi.json", vec![HttpMethod::Get], json);
        openapi
            .paths
            .add_path_operation("/api/v2/docs", vec![HttpMethod::Get], docs);
    }
}

/// Build the contract document.
///
/// The debug assertion is the cheapest place to catch a route added to the
/// router and the `paths(...)` list but not to [`SUPPORTED_V2_PATHS`]: it fires
/// in every test build and in a debug `cflx openapi`, before a half-declared
/// surface can reach a consumer.
pub fn document() -> utoipa::openapi::OpenApi {
    let document = ApiDoc::openapi();
    debug_assert_eq!(
        document
            .paths
            .paths
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<String>>(),
        SUPPORTED_V2_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect::<std::collections::BTreeSet<String>>(),
        "SUPPORTED_V2_PATHS and the generated document disagree about the route surface"
    );
    document
}

/// Banner carried by every copy of the document — the `cflx openapi` export and
/// the live `/api/v2/openapi.yaml` body alike — so whoever holds an exported
/// file is told the same ownership rule as whoever fetched it from a server.
pub const GENERATED_BANNER: &str = "\
# GENERATED DOCUMENT — DO NOT EDIT, DO NOT COMMIT.
# Source of truth: src/web/openapi.rs and the #[utoipa::path] attributes it names.
# Export: cflx openapi   Live: GET /api/v2/openapi.yaml
# This repository tracks no OpenAPI file; regenerate instead of editing a copy.
";

/// The canonical contract's exact bytes.
///
/// `cflx openapi` writes this to stdout and `GET /api/v2/openapi.yaml` returns
/// it. One function so an exported copy and the document a running instance
/// serves cannot disagree.
pub fn document_yaml() -> String {
    let body = serde_yaml::to_string(&document()).expect("OpenAPI document must serialize as YAML");
    format!("{GENERATED_BANNER}{}\n", body.trim_end())
}
