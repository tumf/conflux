use super::*;

/// GET /api/v1/stats/overview - 全プロジェクトの成功/失敗数と平均処理時間を返す
pub(crate) async fn get_stats_overview(State(state): State<AppState>) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    match db.get_stats_overview() {
        Ok(stats) => (
            StatusCode::OK,
            Json(StatsOverviewResponse {
                summary: StatsOverviewSummaryResponse {
                    success_count: stats.summary.success_count,
                    failure_count: stats.summary.failure_count,
                    in_progress_count: stats.summary.in_progress_count,
                    average_duration_ms: stats.summary.average_duration_ms,
                    average_duration_by_operation: stats.summary.average_duration_by_operation,
                },
                recent_events: stats
                    .recent_events
                    .into_iter()
                    .map(|e| StatsRecentEventResponse {
                        project_id: e.project_id,
                        change_id: e.change_id,
                        operation: e.operation,
                        result: e.result,
                        timestamp: e.timestamp,
                    })
                    .collect(),
                project_stats: stats
                    .project_stats
                    .into_iter()
                    .map(|p| StatsProjectResponse {
                        project_id: p.project_id,
                        apply_success_rate: p.apply_success_rate,
                        average_duration_ms: p.average_duration_ms,
                        success_count: p.success_count,
                        failure_count: p.failure_count,
                        in_progress_count: p.in_progress_count,
                    })
                    .collect(),
            }),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to query stats overview");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query stats overview: {}", e),
            )
        }
    }
}

/// GET /api/v1/stats/projects/:id/history - プロジェクト履歴イベントを返す
pub(crate) async fn get_project_history(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    if state.registry.read().await.get(&project_id).is_none() {
        return error_response(StatusCode::NOT_FOUND, "Project not found");
    }

    let limit = query.limit.clamp(1, 1000);
    match db.get_recent_events(&project_id, limit) {
        Ok(events) => (StatusCode::OK, Json(events)).into_response(),
        Err(e) => {
            error!(project_id = %project_id, error = %e, "Failed to query project history");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query project history: {}", e),
            )
        }
    }
}

/// GET /api/v1/logs - 永続化ログの検索
pub(crate) async fn get_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Response {
    let Some(db) = &state.db else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Server database is not enabled",
        );
    };

    let limit = query.limit.clamp(1, 1000);
    match db.query_logs(limit, query.before.as_deref(), query.project_id.as_deref()) {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to query persisted logs");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to query logs: {}", e),
            )
        }
    }
}
