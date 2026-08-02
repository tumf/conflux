use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Conflux Remote Control API",
        version = env!("CARGO_PKG_VERSION"),
        description = "API for monitoring and controlling a running Conflux TUI instance"
    ),
    paths(
        crate::web::remote_control_api::reads::health,
        crate::web::remote_control_api::reads::capabilities,
        crate::web::remote_control_api::reads::instance,
        crate::web::remote_control_api::reads::state,
        crate::web::remote_control_api::reads::list_changes,
        crate::web::remote_control_api::reads::get_change,
        crate::web::remote_control_api::reads::logs,
        crate::web::remote_control_api::reads::list_worktrees,
        crate::web::remote_control_api::reads::get_worktree,
        crate::web::remote_control_api::commands::submit_command,
        crate::web::remote_control_api::commands::get_command,
        crate::web::remote_control_api::stream::events,
        crate::web::remote_control_api::stream::ws,
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
            crate::web::remote_control_api::dto::ParallelBlockedReason,
            crate::web::remote_control_api::dto::ParallelEligibility,
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
        (name = "remote-control", description = "Single-instance remote-control API")
    )
)]
pub struct ApiDoc;

pub fn document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
