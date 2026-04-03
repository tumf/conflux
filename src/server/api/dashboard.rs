use super::*;

// ─────────────────────────────── Dashboard handlers ────────────────────────────

/// Dashboard index HTML - returns the built dashboard HTML file
/// This handler serves the dashboard SPA. In production, Vite's `base: "/dashboard/"`
/// directive ensures correct asset paths for nested routing.
pub(super) async fn dashboard_index() -> Response {
    // The dashboard is built into dashboard/dist/index.html during cargo build
    // We embed it as a static string for maximum portability
    let html = include_str!("../../../dashboard/dist/index.html");
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        html,
    )
        .into_response()
}

/// Dashboard asset handler - serves CSS, JS, and other static files
/// Vite generates assets with hashed filenames in the assets/ directory
pub(super) async fn dashboard_assets(Path(filename): Path<String>) -> Response {
    // Map asset filenames to embedded content
    let content_type = if filename.ends_with(".js") {
        "application/javascript"
    } else if filename.ends_with(".css") {
        "text/css"
    } else if filename.ends_with(".svg") {
        "image/svg+xml"
    } else if filename.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    };

    // This simple approach requires manual asset mapping.
    // For production, prefer a build.rs that generates asset routes dynamically.
    let response = match filename.as_str() {
        env!("DASHBOARD_CSS") => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
            include_str!(concat!(
                "../../../dashboard/dist/assets/",
                env!("DASHBOARD_CSS")
            )),
        )
            .into_response(),
        env!("DASHBOARD_JS") => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
            include_str!(concat!(
                "../../../dashboard/dist/assets/",
                env!("DASHBOARD_JS")
            )),
        )
            .into_response(),
        _ => {
            error!("Dashboard asset not found: {}", filename);
            (StatusCode::NOT_FOUND, "Asset not found").into_response()
        }
    };

    response
}

/// Dashboard favicon.svg
pub(super) async fn dashboard_favicon() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )],
        include_str!("../../../dashboard/dist/favicon.svg"),
    )
        .into_response()
}

/// Dashboard icons.svg
pub(super) async fn dashboard_icons() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )],
        include_str!("../../../dashboard/dist/icons.svg"),
    )
        .into_response()
}
