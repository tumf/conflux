//! Legacy compatibility tests.
//!
//! Adding `/api/v2` must be additive. These tests fail if the legacy surface
//! loses a route, changes shape, or starts demanding v2's credentials — which is
//! exactly what would break the dashboard shipped in this binary.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use crate::web::state::WebState;
use crate::web::WebConfig;

use super::{json_body, send};

/// The full single-instance app: legacy routes plus `/api/v2`.
fn full_app(config: &WebConfig) -> axum::Router {
    let state = Arc::new(WebState::new(&[]));
    crate::web::build_app_for_test(config, state)
}

fn request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:8080")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn legacy_monitoring_routes_stay_available_and_unauthenticated() {
    // v2 requires a token here; the legacy surface must be unaffected by that.
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("v2-token".to_string()),
        None,
        Vec::new(),
    );
    let app = full_app(&config);

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/api/health"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["status"], "ok", "legacy health keeps its own shape");

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/api/state"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(
        body.get("changes").is_some() && body.get("app_mode").is_some(),
        "legacy state keeps its own snapshot shape, not the v2 one"
    );

    let response = app
        .oneshot(request(Method::GET, "/api/changes"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn legacy_control_and_worktree_routes_remain_mounted() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = full_app(&config);

    for (method, path) in [
        (Method::POST, "/api/control/start"),
        (Method::POST, "/api/control/stop"),
        (Method::POST, "/api/control/cancel-stop"),
        (Method::POST, "/api/control/force-stop"),
        (Method::POST, "/api/control/retry"),
        (Method::GET, "/api/worktrees"),
    ] {
        let response = app
            .clone()
            .oneshot(request(method.clone(), path))
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{path} must stay mounted"
        );
        assert_ne!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{path} must keep accepting {method}"
        );
    }
}

#[tokio::test]
async fn the_legacy_browser_websocket_and_dashboard_files_remain_available() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("v2-token".to_string()),
        None,
        Vec::new(),
    );
    let app = full_app(&config);

    // The dashboard is a browser client and cannot set an Authorization header,
    // which is precisely why it stays on `/ws` instead of migrating to v2.
    let response = app
        .clone()
        .oneshot(request(Method::GET, "/ws"))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "v2 credentials must not be imposed on the legacy WebSocket"
    );

    for path in ["/", "/style.css", "/app.js"] {
        let response = app
            .clone()
            .oneshot(request(Method::GET, path))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn v2_is_mounted_next_to_the_legacy_surface_not_inside_it() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = full_app(&config);

    let legacy = json_body(
        app.clone()
            .oneshot(request(Method::GET, "/api/health"))
            .await
            .unwrap(),
    )
    .await;
    let v2 = json_body(
        app.clone()
            .oneshot(request(Method::GET, "/api/v2/health"))
            .await
            .unwrap(),
    )
    .await;

    assert!(
        legacy.get("api_version").is_none(),
        "legacy health is unversioned"
    );
    assert_eq!(v2["api_version"], "v2");

    // v1 is server mode's multi-project namespace and is not served here.
    let response = send(&app, request(Method::GET, "/api/v1/projects")).await;
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "single-instance mode must not answer for the multi-project namespace"
    );
}
