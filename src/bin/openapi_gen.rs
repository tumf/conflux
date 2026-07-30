//! OpenAPI specification generator for Conflux web monitoring API.
//!
//! This binary generates an OpenAPI 3.1 specification from the web API code
//! using utoipa annotations. The generated spec is written to docs/openapi.yaml.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Conflux Web Monitoring API",
        version = env!("CARGO_PKG_VERSION"),
        description = "REST API for monitoring and controlling Conflux orchestration",
        contact(
            name = "Conflux",
            url = "https://github.com/anomalyco/conflux"
        )
    ),
    paths(
        conflux::web::api::health,
        conflux::web::api::get_state,
        conflux::web::api::list_changes,
        conflux::web::api::get_change,
        conflux::web::api::control_start,
        conflux::web::api::control_stop,
        conflux::web::api::control_cancel_stop,
        conflux::web::api::control_force_stop,
        conflux::web::api::control_retry,
        conflux::web::websocket::ws_handler,
        // Single-instance remote-control API (`/api/v2`)
        conflux::web::remote_control_api::reads::health,
        conflux::web::remote_control_api::reads::capabilities,
        conflux::web::remote_control_api::reads::instance,
        conflux::web::remote_control_api::reads::state,
        conflux::web::remote_control_api::reads::list_changes,
        conflux::web::remote_control_api::reads::get_change,
        conflux::web::remote_control_api::reads::logs,
        conflux::web::remote_control_api::commands::submit_command,
        conflux::web::remote_control_api::commands::get_command,
        conflux::web::remote_control_api::stream::events,
        conflux::web::remote_control_api::stream::ws,
    ),
    components(
        schemas(
            conflux::web::api::HealthResponse,
            conflux::web::api::ErrorResponse,
            conflux::web::api::ControlResponse,
            conflux::web::state::ChangeStatus,
            conflux::web::state::OrchestratorStateSnapshot,
            conflux::web::state::StateUpdate,
            conflux::events::LogEntry,
            conflux::events::LogLevel,
            conflux::tui::types::WorktreeInfo,
            conflux::tui::types::MergeConflictInfo,
            // Single-instance remote-control API (`/api/v2`)
            conflux::web::remote_control_api::dto::ApiError,
            conflux::web::remote_control_api::dto::ErrorCode,
            conflux::web::remote_control_api::dto::CapabilitiesResponse,
            conflux::web::remote_control_api::dto::CapabilityLimits,
            conflux::web::remote_control_api::dto::ChangeResource,
            conflux::web::remote_control_api::dto::ChangeResponse,
            conflux::web::remote_control_api::dto::ChangesResponse,
            conflux::web::remote_control_api::dto::CommandRecord,
            conflux::web::remote_control_api::dto::CommandRequest,
            conflux::web::remote_control_api::dto::CommandSpec,
            conflux::web::remote_control_api::dto::CommandState,
            conflux::web::remote_control_api::dto::EventCategory,
            conflux::web::remote_control_api::dto::EventEnvelope,
            conflux::web::remote_control_api::dto::HealthResponse,
            conflux::web::remote_control_api::dto::InstanceResponse,
            conflux::web::remote_control_api::dto::InstanceSnapshot,
            conflux::web::remote_control_api::dto::LogsResponse,
            conflux::web::remote_control_api::dto::SnapshotTotals,
            conflux::web::remote_control_api::dto::StateResponse,
            conflux::web::remote_control_api::dto::TransportDescriptor,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "state", description = "State management endpoints"),
        (name = "changes", description = "Change management endpoints"),
        (name = "control", description = "Orchestrator control endpoints"),
        (name = "websocket", description = "WebSocket endpoints for real-time updates"),
        (name = "remote-control", description = "Single-instance remote-control API (/api/v2)")
    )
)]
struct ApiDoc;

fn main() {
    let openapi = ApiDoc::openapi();
    let yaml = serde_yaml::to_string(&openapi).expect("Failed to serialize OpenAPI spec to YAML");

    // Print YAML without trailing newline, then add exactly one newline
    // This ensures compatibility with end-of-file-fixer pre-commit hook
    print!("{}", yaml.trim_end());
    println!();
}
