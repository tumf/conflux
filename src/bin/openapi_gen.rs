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
        conflux::web::api::approve_change,
        conflux::web::api::unapprove_change,
        conflux::web::api::control_start,
        conflux::web::api::control_stop,
        conflux::web::api::control_cancel_stop,
        conflux::web::api::control_force_stop,
        conflux::web::api::control_retry,
        conflux::web::websocket::ws_handler,
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
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "state", description = "State management endpoints"),
        (name = "changes", description = "Change management endpoints"),
        (name = "control", description = "Orchestrator control endpoints"),
        (name = "websocket", description = "WebSocket endpoints for real-time updates")
    )
)]
struct ApiDoc;

fn main() {
    let openapi = ApiDoc::openapi();
    let yaml = serde_yaml::to_string(&openapi).expect("Failed to serialize OpenAPI spec to YAML");

    println!("{}", yaml);
}
