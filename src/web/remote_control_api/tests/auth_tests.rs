//! Authentication, credential-transport, origin, and startup-safety tests.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};

use crate::web::remote_control_api::auth::{normalize_origin, RemoteControlAuth};
use crate::web::WebConfig;

use super::{get, harness, send, status_and_json};

const TOKEN: &str = "s3cret-token";

// ── Bearer enforcement ───────────────────────────────────────────────────────

#[tokio::test]
async fn health_is_public_even_when_authentication_is_enforced() {
    let h = harness(Some(TOKEN), &[]);
    let (status, body) = status_and_json(send(&h.router, get("/api/v2/health", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["api_version"], "v2");
}

#[tokio::test]
async fn every_other_v2_resource_requires_bearer_credentials() {
    let h = harness(Some(TOKEN), &[]);
    for path in [
        "/api/v2/capabilities",
        "/api/v2/instance",
        "/api/v2/state",
        "/api/v2/changes",
        "/api/v2/changes/c1",
        "/api/v2/logs",
        "/api/v2/commands/abc",
        "/api/v2/events",
        "/api/v2/ws",
    ] {
        let (status, body) = status_and_json(send(&h.router, get(path, None)).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} must be protected");
        assert_eq!(body["error_code"], "unauthorized", "{path}");
        assert!(
            !body["correlation_id"].as_str().unwrap_or("").is_empty(),
            "{path} error must carry a correlation ID"
        );
    }
}

#[tokio::test]
async fn a_wrong_token_is_refused() {
    let h = harness(Some(TOKEN), &[]);
    let (status, body) =
        status_and_json(send(&h.router, get("/api/v2/state", Some("wrong"))).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "unauthorized");
}

#[tokio::test]
async fn loopback_deployments_may_run_without_a_token() {
    let h = harness(None, &[]);
    let (status, body) = status_and_json(send(&h.router, get("/api/v2/state", None)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state_revision"], 0);
}

// ── Credential transport ─────────────────────────────────────────────────────

#[tokio::test]
async fn websocket_query_token_is_rejected_and_creates_no_subscription() {
    let h = harness(Some(TOKEN), &[]);
    let (status, body) =
        status_and_json(send(&h.router, get(&format!("/api/v2/ws?token={TOKEN}"), None)).await)
            .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "unauthorized");
    assert!(
        body["message"].as_str().unwrap().contains("query"),
        "the refusal must name the unsupported transport"
    );
}

#[tokio::test]
async fn websocket_subprotocol_token_is_rejected() {
    let h = harness(Some(TOKEN), &[]);
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/ws")
        .header("host", "127.0.0.1:8080")
        .header("sec-websocket-protocol", format!("bearer.{TOKEN}"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = status_and_json(send(&h.router, request).await).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Sec-WebSocket-Protocol"));
}

#[tokio::test]
async fn query_credentials_are_refused_on_every_path_including_health() {
    let h = harness(Some(TOKEN), &[]);
    for path in ["/api/v2/health", "/api/v2/events", "/api/v2/state"] {
        let uri = format!("{path}?access_token={TOKEN}");
        let (status, _) = status_and_json(send(&h.router, get(&uri, Some(TOKEN))).await).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{path} must not teach clients to put tokens in URLs"
        );
    }
}

#[tokio::test]
async fn ordinary_query_parameters_are_still_accepted() {
    let h = harness(Some(TOKEN), &[]);
    let response = send(
        &h.router,
        get("/api/v2/events?after_sequence=0", Some(TOKEN)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

// ── Correlation IDs ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_valid_correlation_header_is_echoed_back() {
    let h = harness(None, &[]);
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/state")
        .header("host", "127.0.0.1:8080")
        .header("x-correlation-id", "trace.A-1:b")
        .body(Body::empty())
        .unwrap();

    let response = send(&h.router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-correlation-id").unwrap(),
        "trace.A-1:b"
    );
}

#[tokio::test]
async fn an_invalid_correlation_header_fails_validation() {
    let h = harness(None, &[]);
    for bad in ["", &"x".repeat(65), "has space"] {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v2/state")
            .header("host", "127.0.0.1:8080")
            .header("x-correlation-id", bad)
            .body(Body::empty())
            .unwrap();
        let (status, body) = status_and_json(send(&h.router, request).await).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad:?}");
        assert_eq!(body["error_code"], "validation_failed");
    }
}

// ── Origin normalization ─────────────────────────────────────────────────────

#[test]
fn origins_normalize_scheme_host_and_default_port() {
    let http = normalize_origin("HTTP://Example.COM").unwrap();
    assert_eq!(http.scheme, "http");
    assert_eq!(http.host, "example.com");
    assert_eq!(http.port, 80);

    assert_eq!(normalize_origin("https://example.com").unwrap().port, 443);
    assert_eq!(
        normalize_origin("http://example.com:8080").unwrap().port,
        8080
    );
    assert_eq!(normalize_origin("http://[::1]:9000").unwrap().host, "::1");
    assert_eq!(normalize_origin("http://[::1]").unwrap().port, 80);
}

#[test]
fn wildcards_paths_and_unknown_schemes_are_not_origins() {
    for bad in [
        "*",
        "http://*.example.com",
        "https://*",
        "http://example.com/path",
        "http://example.com?q=1",
        "ftp://example.com",
        "example.com",
        "http://",
        "",
    ] {
        assert!(normalize_origin(bad).is_none(), "{bad:?} must be rejected");
    }
}

#[test]
fn an_invalid_configured_origin_is_a_policy_error_not_a_silent_drop() {
    let error = RemoteControlAuth::new(None, &["https://*.example.com".to_string()]).unwrap_err();
    assert!(error.contains("https://*.example.com"));
}

// ── CORS behavior ────────────────────────────────────────────────────────────

fn with_origin(uri: &str, origin: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("host", "127.0.0.1:8080")
        .header("origin", origin);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn a_request_without_origin_is_allowed_and_gets_no_cors_grant() {
    let h = harness(None, &[]);
    let response = send(&h.router, get("/api/v2/state", None)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}

#[tokio::test]
async fn a_direct_same_origin_request_is_allowed() {
    let h = harness(None, &[]);
    let response = send(
        &h.router,
        with_origin("/api/v2/state", "http://127.0.0.1:8080", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://127.0.0.1:8080"
    );
}

#[tokio::test]
async fn a_foreign_origin_is_denied() {
    let h = harness(None, &[]);
    let (status, body) = status_and_json(
        send(
            &h.router,
            with_origin("/api/v2/state", "https://evil.example", None),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error_code"], "forbidden");
}

#[tokio::test]
async fn an_exactly_configured_proxy_origin_is_allowed() {
    let h = harness(None, &["https://ops.example.com"]);
    let response = send(
        &h.router,
        with_origin("/api/v2/state", "https://ops.example.com", None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "https://ops.example.com"
    );
    assert_eq!(response.headers().get("vary").unwrap(), "Origin");
}

#[tokio::test]
async fn forwarded_headers_never_widen_the_allowed_origin_set() {
    let h = harness(None, &[]);
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v2/state")
        .header("host", "127.0.0.1:8080")
        .header("origin", "https://ops.example.com")
        .header("x-forwarded-host", "ops.example.com")
        .header("x-forwarded-proto", "https")
        .header("forwarded", "host=ops.example.com;proto=https")
        .body(Body::empty())
        .unwrap();

    let (status, body) = status_and_json(send(&h.router, request).await).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an attacker-controllable header must not create a same-origin match"
    );
    assert_eq!(body["error_code"], "forbidden");
}

#[tokio::test]
async fn a_preflight_from_an_allowed_origin_is_answered_without_reaching_a_handler() {
    let h = harness(Some(TOKEN), &["https://ops.example.com"]);
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v2/commands")
        .header("host", "127.0.0.1:8080")
        .header("origin", "https://ops.example.com")
        .body(Body::empty())
        .unwrap();

    let response = send(&h.router, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "https://ops.example.com"
    );
    assert!(response
        .headers()
        .get("access-control-allow-headers")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Authorization"));
    assert_eq!(h.executor.call_count(), 0);
}

// ── Startup safety ───────────────────────────────────────────────────────────

#[test]
fn non_loopback_binding_without_a_token_is_rejected_before_listening() {
    let config = WebConfig::enabled(9000, "0.0.0.0".to_string());
    let error = config.validate().unwrap_err();
    assert!(error.contains("0.0.0.0"));
    assert!(error.contains("--web-auth-token"));
}

#[test]
fn non_loopback_binding_with_a_token_is_accepted() {
    let config = WebConfig::enabled(9000, "0.0.0.0".to_string()).with_auth(
        Some(TOKEN.to_string()),
        None,
        Vec::new(),
    );
    assert!(config.validate().is_ok());
}

#[test]
fn loopback_binding_without_a_token_is_accepted() {
    for bind in ["127.0.0.1", "127.0.0.5", "::1", "[::1]", "localhost"] {
        let config = WebConfig::enabled(0, bind.to_string());
        assert!(config.is_loopback_bind(), "{bind} must count as loopback");
        assert!(config.validate().is_ok(), "{bind}");
    }
}

#[test]
fn non_loopback_addresses_are_recognized() {
    for bind in ["0.0.0.0", "192.168.1.10", "::", "10.0.0.1"] {
        let config = WebConfig::enabled(0, bind.to_string());
        assert!(
            !config.is_loopback_bind(),
            "{bind} must not count as loopback"
        );
    }
}

#[test]
fn token_and_token_env_are_mutually_exclusive() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("literal".to_string()),
        Some("CFLX_WEB_TOKEN".to_string()),
        Vec::new(),
    );
    let error = config.validate().unwrap_err();
    assert!(error.contains("mutually exclusive"));
}

#[test]
fn the_environment_form_resolves_the_token_and_fails_closed_when_unset() {
    let var = "CFLX_TEST_WEB_TOKEN_REMOTE_CONTROL";
    // SAFETY: single-threaded test process mutation of a uniquely named variable.
    unsafe { std::env::set_var(var, "from-env") };
    let config = WebConfig::enabled(9000, "0.0.0.0".to_string()).with_auth(
        None,
        Some(var.to_string()),
        Vec::new(),
    );
    assert_eq!(config.resolve_auth_token().as_deref(), Some("from-env"));
    assert!(config.validate().is_ok());

    unsafe { std::env::remove_var(var) };
    assert_eq!(
        config.resolve_auth_token(),
        None,
        "an unset variable must not silently fall back to another secret"
    );
    assert!(config.validate().is_err());
}

#[test]
fn an_empty_token_does_not_satisfy_the_non_loopback_requirement() {
    let config = WebConfig::enabled(9000, "0.0.0.0".to_string()).with_auth(
        Some(String::new()),
        None,
        Vec::new(),
    );
    assert!(config.validate().is_err());
}

#[test]
fn wildcard_and_malformed_allowed_origins_are_rejected_at_startup() {
    for origin in ["*", "http://*.example.com", "example.com", "http://a/b"] {
        let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
            None,
            None,
            vec![origin.to_string()],
        );
        let error = config.validate().unwrap_err();
        assert!(
            error.contains(origin),
            "{origin} must be named in the error"
        );
    }
}

#[test]
fn repeatable_exact_origins_are_accepted() {
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        None,
        None,
        vec![
            "https://ops.example.com".to_string(),
            "http://localhost:5173".to_string(),
        ],
    );
    assert!(config.validate().is_ok());
    assert_eq!(config.allowed_origins.len(), 2);
}
