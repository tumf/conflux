//! Route-surface tests for the single-instance web server.
//!
//! The legacy unversioned `/api/*` and `/ws` surface is gone: the embedded
//! console is a `/api/v2` client now, so a second, unauthenticated contract
//! would only be a way around v2's bearer policy, revision checks, and typed
//! errors. These tests assert both halves of that removal — the legacy paths are
//! absent and cannot cause a side effect, and everything the console actually
//! needs is still served.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use crate::web::state::WebState;
use crate::web::WebConfig;

use super::{json_body, send};

/// Every legacy single-instance route, including the mutating ones.
const REMOVED_LEGACY_ROUTES: [(Method, &str); 17] = [
    (Method::GET, "/api/health"),
    (Method::GET, "/api/state"),
    (Method::GET, "/api/changes"),
    (Method::GET, "/api/changes/some-change"),
    (Method::POST, "/api/control/start"),
    (Method::POST, "/api/control/stop"),
    (Method::POST, "/api/control/cancel-stop"),
    (Method::POST, "/api/control/force-stop"),
    (Method::POST, "/api/control/retry"),
    (Method::GET, "/api/worktrees"),
    (Method::POST, "/api/worktrees/refresh"),
    (Method::POST, "/api/worktrees/create"),
    (Method::POST, "/api/worktrees/delete"),
    (Method::POST, "/api/worktrees/merge"),
    (Method::POST, "/api/worktrees/command"),
    (Method::GET, "/ws"),
    (Method::GET, "/api/v1/projects"),
];

fn app_with(state: Arc<WebState>, config: &WebConfig) -> axum::Router {
    crate::web::build_app_for_test(config, state)
}

fn app(config: &WebConfig) -> axum::Router {
    app_with(Arc::new(WebState::new(&[])), config)
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
async fn every_legacy_single_instance_route_is_absent() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = app(&config);

    for (method, path) in REMOVED_LEGACY_ROUTES {
        let response = app
            .clone()
            .oneshot(request(method.clone(), path))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} {path} must be removed, not merely unauthenticated"
        );
    }
}

#[tokio::test]
async fn removed_legacy_routes_are_absent_even_when_v2_requires_a_token() {
    // A 401 here would mean the route still exists behind auth. It must not.
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("v2-token".to_string()),
        None,
        Vec::new(),
    );
    let app = app(&config);

    for (method, path) in REMOVED_LEGACY_ROUTES {
        let response = app
            .clone()
            .oneshot(request(method.clone(), path))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
    }
}

#[tokio::test]
async fn a_request_to_a_removed_mutation_route_has_no_side_effect() {
    let state = Arc::new(WebState::new(&[]));
    // The process-local control channel these routes used to enqueue onto is
    // gone: `/api/v2` executes lifecycle commands through the shared run-control
    // service, so the observable side-effect surface is the v2 projection —
    // its revision and its admitted/settled command registry.
    let projection = state.remote_control().projection();
    let before = (projection.revision(), projection.registry_sizes());

    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = app_with(state, &config);

    for (method, path) in REMOVED_LEGACY_ROUTES {
        if method != Method::POST {
            continue;
        }
        let response = app.clone().oneshot(request(method, path)).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }

    assert_eq!(
        (projection.revision(), projection.registry_sizes()),
        before,
        "no removed route may reach the shared command surface"
    );
}

#[tokio::test]
async fn the_embedded_console_assets_are_served_with_their_own_content_types() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("v2-token".to_string()),
        None,
        Vec::new(),
    );
    let app = app(&config);

    // Static delivery is never gated by the v2 bearer policy: the browser has to
    // load the console before it can ask the user for a token.
    for (path, content_type) in [
        ("/", "text/html"),
        ("/style.css", "text/css"),
        ("/app.js", "application/javascript"),
    ] {
        let response = app
            .clone()
            .oneshot(request(Method::GET, path))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let header = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(
            header.starts_with(content_type),
            "{path} served as '{header}', expected {content_type}"
        );
    }

    let response = app
        .oneshot(request(Method::GET, "/not-a-real-asset.png"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_console_html_references_only_the_retained_assets() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = app(&config);

    let response = app.oneshot(request(Method::GET, "/")).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("/style.css"));
    assert!(html.contains("/app.js"));
}

#[tokio::test]
async fn the_shipped_client_targets_no_legacy_route() {
    // The assertion is over the embedded bytes rather than the working tree, so
    // it also covers a stale build of the console.
    let client = crate::web::static_files::APP_JS;
    for legacy in [
        "/api/health",
        "/api/state",
        "/api/changes",
        "/api/control/",
        "/api/worktrees",
        "'/ws'",
        "\"/ws\"",
    ] {
        assert!(
            !client.contains(legacy),
            "the console must not reference the removed route {legacy}"
        );
    }
    assert!(client.contains("/api/v2/state"));
    assert!(client.contains("/api/v2/commands"));
}

#[tokio::test]
async fn the_versioned_surface_and_its_documents_remain_available() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = app(&config);

    for path in [
        "/api/v2/health",
        "/api/v2/capabilities",
        "/api/v2/instance",
        "/api/v2/state",
        "/api/v2/changes",
        "/api/v2/logs",
    ] {
        let response = app
            .clone()
            .oneshot(request(Method::GET, path))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/api/v2/openapi.json"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document = json_body(response).await;
    assert!(document["paths"].get("/api/v2/commands").is_some());
    assert!(
        document["paths"].get("/api/state").is_none(),
        "the generated document must not describe a removed route"
    );

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/api/v2/openapi.yaml"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/yaml"
    );

    let response = send(&app, request(Method::GET, "/api/v2/docs/")).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn static_delivery_carries_no_permissive_cors_header() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string());
    let app = app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .header("host", "127.0.0.1:8080")
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "the permissive legacy CORS layer must be gone"
    );
}
