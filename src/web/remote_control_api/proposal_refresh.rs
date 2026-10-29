//! `GET /api/v2/proposals/{change_id}/refresh-preview`.
//!
//! The read half of the candidate proposal refresh command. It is a *read*: it
//! creates no command record, submits no intent, advances no revision, and
//! writes nothing to any worktree. It is served on both listeners and by a
//! headless `cflx run`, which has no command executor but does know where its
//! candidate worktrees are.
//!
//! Lifecycle eligibility is the projected row's own `actions.refresh_proposal`,
//! so the preview and the published action can never disagree. The Git facts
//! come from the same [`crate::orchestration::proposal_refresh::preview`] the
//! shared transaction's apply path is built from.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

use crate::orchestration::proposal_refresh::{RefreshPreview, RefreshReason};

use super::auth::CorrelationId;
use super::dto::{ActionBlockedReason, ApiError, ErrorCode, RefreshPreviewResponse};
use super::RemoteControlState;

/// Route template of the preview resource, as capabilities advertise it.
pub const PREVIEW_ROUTE: &str = "/api/v2/proposals/{change_id}/refresh-preview";

/// Where the project repository and a change's candidate worktree live.
///
/// Implemented by the monitoring state, which already tracks both for the
/// worktree relation it projects, so the path a preview reads is the path the
/// row's `worktree` field describes.
#[async_trait]
pub trait RefreshPathSource: Send + Sync {
    /// `(project_root, candidate_worktree)` for one change, when both are known.
    async fn locate(&self, change_id: &str) -> Option<(PathBuf, PathBuf)>;
}

/// Query parameters of the preview.
#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    /// Explicit source ref. Required: there is no default branch.
    pub source_ref: Option<String>,
}

/// Map a published lifecycle block back onto the shared reason vocabulary.
fn lifecycle_reason(reason: Option<ActionBlockedReason>) -> RefreshReason {
    match reason {
        Some(ActionBlockedReason::UnknownTarget) => RefreshReason::UnknownTarget,
        Some(ActionBlockedReason::NoCandidateWorktree) => RefreshReason::NoCandidateWorktree,
        Some(ActionBlockedReason::TerminalTarget) => RefreshReason::TerminalTarget,
        Some(ActionBlockedReason::TargetActive) => RefreshReason::TargetActive,
        Some(ActionBlockedReason::MergeWait) => RefreshReason::MergeWait,
        Some(ActionBlockedReason::ResolveWait) => RefreshReason::ResolveWait,
        Some(ActionBlockedReason::RejectWait) => RefreshReason::RejectWait,
        _ => RefreshReason::TargetAdmitted,
    }
}

/// Preview a candidate proposal refresh without mutating anything.
#[utoipa::path(
    get,
    path = "/api/v2/proposals/{change_id}/refresh-preview",
    tag = "remote-control",
    params(
        ("change_id" = String, Path, description = "Change ID"),
        ("source_ref" = String, Query, description = "Explicit source ref; there is no default")
    ),
    responses(
        (status = 200, description = "Preview facts; `eligible` and `refusal` say whether an apply would proceed", body = RefreshPreviewResponse),
        (status = 404, description = "No such change in this incarnation", body = ApiError),
        (status = 422, description = "`source_ref` is missing or empty", body = ApiError)
    )
)]
pub async fn preview(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    Path(change_id): Path<String>,
    Query(query): Query<PreviewQuery>,
) -> Response {
    let source_ref = match query.source_ref.as_deref().map(str::trim) {
        Some(source_ref) if !source_ref.is_empty() => source_ref.to_string(),
        _ => {
            return ApiError::new(
                ErrorCode::ValidationFailed,
                "refresh-preview requires a non-empty source_ref query parameter",
                &correlation.0,
            )
            .into_response()
        }
    };

    let (snapshot, state_revision, _) = state.projection.snapshot();
    let Some(change) = snapshot
        .changes
        .iter()
        .find(|change| change.id == change_id)
    else {
        return ApiError::new(
            ErrorCode::NotFound,
            format!("change '{change_id}' is not present in this instance"),
            &correlation.0,
        )
        .with_revision(state_revision)
        .into_response();
    };

    let eligibility = change.actions.refresh_proposal;
    let preview = if !eligibility.allowed {
        RefreshPreview::refused(
            &change_id,
            &source_ref,
            lifecycle_reason(eligibility.blocked_reason),
        )
    } else {
        match locate(&state, &change_id).await {
            Some((root, worktree)) => {
                crate::orchestration::proposal_refresh::preview(
                    &root,
                    &worktree,
                    &change_id,
                    &source_ref,
                )
                .await
            }
            None => {
                RefreshPreview::refused(&change_id, &source_ref, RefreshReason::NoCandidateWorktree)
            }
        }
    };

    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(RefreshPreviewResponse::from_preview(
            preview,
            state.projection.instance_id().to_string(),
            state_revision,
        )),
    )
        .into_response()
}

async fn locate(state: &RemoteControlState, change_id: &str) -> Option<(PathBuf, PathBuf)> {
    let source: Arc<dyn RefreshPathSource> = state.refresh_paths.clone()?;
    source.locate(change_id).await
}
