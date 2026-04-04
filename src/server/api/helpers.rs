use super::*;

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

pub(super) fn error_response(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(ErrorResponse { error: msg.into() })).into_response()
}

pub(super) fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Response for `GET /api/v1/projects/state` including top-level metadata.
#[derive(Debug, Serialize)]
pub(super) struct ProjectsStateResponse {
    pub(super) projects: Vec<RemoteProject>,
    /// Whether git/sync is available (resolve_command is configured)
    pub(super) sync_available: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct StatsOverviewResponse {
    pub(super) summary: StatsOverviewSummaryResponse,
    pub(super) recent_events: Vec<StatsRecentEventResponse>,
    pub(super) project_stats: Vec<StatsProjectResponse>,
}

#[derive(Debug, Serialize)]
pub(super) struct StatsOverviewSummaryResponse {
    pub(super) success_count: i64,
    pub(super) failure_count: i64,
    pub(super) in_progress_count: i64,
    pub(super) average_duration_ms: Option<f64>,
    pub(super) average_duration_by_operation: Option<std::collections::HashMap<String, f64>>,
}

#[derive(Debug, Serialize)]
pub(super) struct StatsRecentEventResponse {
    pub(super) project_id: String,
    pub(super) change_id: String,
    pub(super) operation: String,
    pub(super) result: String,
    pub(super) timestamp: String,
}

#[derive(Debug, Serialize)]
pub(super) struct StatsProjectResponse {
    pub(super) project_id: String,
    pub(super) apply_success_rate: f64,
    pub(super) average_duration_ms: Option<f64>,
    pub(super) success_count: i64,
    pub(super) failure_count: i64,
    pub(super) in_progress_count: i64,
}
