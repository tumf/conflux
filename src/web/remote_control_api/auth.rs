//! Bearer authentication and exact-origin CORS for `/api/v2`.
//!
//! Two deliberate restrictions live here:
//!
//! * Credentials only ever travel in the `Authorization` header. Query strings
//!   and WebSocket subprotocols are rejected outright, because both end up in
//!   logs, proxies, and browser history.
//! * A cross origin is allowed only when it matches the *direct* request origin
//!   or an exactly configured one. Forwarded headers are attacker-controllable
//!   in the general case, so they are never consulted; a proxy that rewrites the
//!   external origin has to say so explicitly.

use axum::http::header::{HeaderMap, HeaderValue};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};

use super::dto::{new_hex_id, ApiError, ErrorCode};

/// Query parameter names that must never be accepted as credentials.
const REJECTED_TOKEN_PARAMS: [&str; 4] = ["token", "access_token", "auth", "bearer"];

/// Per-request correlation label, resolved once by the middleware and reused by
/// every handler so a request and the error it produces carry the same ID.
#[derive(Debug, Clone)]
pub struct CorrelationId(pub String);

/// A parsed origin, reduced to the three parts that decide identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOrigin {
    /// Lowercase scheme.
    pub scheme: String,
    /// Lowercase host.
    pub host: String,
    /// Explicit or scheme-default port.
    pub port: u16,
}

/// Parse an origin into scheme/host/port, or reject it.
///
/// Wildcards are rejected here rather than at configuration time as well, so no
/// code path can widen the allowlist by accident.
pub fn normalize_origin(value: &str) -> Option<NormalizedOrigin> {
    let value = value.trim();
    if value.is_empty() || value.contains('*') {
        return None;
    }
    let (scheme, rest) = value.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    // An origin has no path, query, or fragment.
    if rest.contains('/') || rest.contains('?') || rest.contains('#') || rest.is_empty() {
        return None;
    }

    let (host, port) = if let Some(inner) = rest.strip_prefix('[') {
        // IPv6 literal: [::1] or [::1]:8080
        let (host, tail) = inner.split_once(']')?;
        let port = match tail {
            "" => default_port,
            other => other.strip_prefix(':')?.parse().ok()?,
        };
        (host.to_ascii_lowercase(), port)
    } else if let Some((host, port)) = rest.rsplit_once(':') {
        (host.to_ascii_lowercase(), port.parse().ok()?)
    } else {
        (rest.to_ascii_lowercase(), default_port)
    };

    if host.is_empty() {
        return None;
    }
    Some(NormalizedOrigin { scheme, host, port })
}

/// Resolved authentication and origin policy for the v2 router.
#[derive(Debug, Clone, Default)]
pub struct RemoteControlAuth {
    /// Expected bearer token. `None` means authentication is not enforced.
    pub token: Option<String>,
    /// Exact additional origins allowed to make cross-origin v2 requests.
    pub allowed_origins: Vec<NormalizedOrigin>,
}

impl RemoteControlAuth {
    /// Build a policy from a resolved token and raw origin strings.
    ///
    /// Returns the first malformed or wildcard origin as an error so startup can
    /// fail loudly instead of silently narrowing (or widening) the policy.
    pub fn new(token: Option<String>, origins: &[String]) -> Result<Self, String> {
        let mut allowed_origins = Vec::with_capacity(origins.len());
        for origin in origins {
            match normalize_origin(origin) {
                Some(normalized) => allowed_origins.push(normalized),
                None => {
                    return Err(format!(
                        "invalid allowed origin '{origin}': expected an exact \
                         http(s)://host[:port] value with no wildcard or path"
                    ))
                }
            }
        }
        Ok(Self {
            token: token.filter(|t| !t.is_empty()),
            allowed_origins,
        })
    }

    /// True when bearer authentication is enforced.
    pub fn is_enforced(&self) -> bool {
        self.token.is_some()
    }

    /// Check bearer credentials for a protected request.
    pub fn check_bearer(&self, headers: &HeaderMap, correlation_id: &str) -> Result<(), ApiError> {
        let Some(expected) = self.token.as_deref() else {
            return Ok(());
        };
        let provided = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or("");

        if provided.is_empty() || !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
            return Err(ApiError::new(
                ErrorCode::Unauthorized,
                "missing or invalid Authorization: Bearer credentials",
                correlation_id,
            ));
        }
        Ok(())
    }

    /// Reject credentials smuggled through a query string or subprotocol.
    ///
    /// These transports exist because browsers cannot set headers on
    /// `EventSource`/`WebSocket`. Supporting them would put the bearer token in
    /// URLs and handshake headers, so v2 refuses them and leaves browser clients
    /// on the legacy `/ws` path instead.
    pub fn reject_out_of_band_credentials(
        &self,
        query: Option<&str>,
        headers: &HeaderMap,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        if let Some(query) = query {
            for pair in query.split('&') {
                let name = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
                if REJECTED_TOKEN_PARAMS.contains(&name.as_str()) {
                    return Err(ApiError::new(
                        ErrorCode::Unauthorized,
                        "credentials in query parameters are not accepted; use Authorization: Bearer",
                        correlation_id,
                    ));
                }
            }
        }
        if headers.contains_key("sec-websocket-protocol") {
            return Err(ApiError::new(
                ErrorCode::Unauthorized,
                "credentials in Sec-WebSocket-Protocol are not accepted; use Authorization: Bearer",
                correlation_id,
            ));
        }
        Ok(())
    }

    /// Decide whether a request's `Origin` may receive a cross-origin response.
    ///
    /// `Ok(None)` means the request carried no `Origin` (not a browser CORS
    /// request); `Ok(Some(origin))` is the value to echo back.
    pub fn check_origin(
        &self,
        headers: &HeaderMap,
        correlation_id: &str,
    ) -> Result<Option<String>, ApiError> {
        let Some(origin_header) = headers
            .get(axum::http::header::ORIGIN)
            .and_then(|value| value.to_str().ok())
        else {
            return Ok(None);
        };

        let deny = || {
            ApiError::new(
                ErrorCode::Forbidden,
                "origin is not allowed for /api/v2; configure an exact allowed origin",
                correlation_id,
            )
        };

        let origin = normalize_origin(origin_header).ok_or_else(deny)?;

        // Same-origin uses the *direct* request authority only. `Host` is the
        // authority this process was actually addressed on; `X-Forwarded-*` is
        // not consulted at all.
        if let Some(direct) = direct_origin(headers) {
            if direct == origin {
                return Ok(Some(origin_header.to_string()));
            }
        }
        if self.allowed_origins.contains(&origin) {
            return Ok(Some(origin_header.to_string()));
        }
        Err(deny())
    }
}

/// The origin this process was directly addressed on.
///
/// The v2 listener terminates plain HTTP, so the direct scheme is always `http`.
fn direct_origin(headers: &HeaderMap) -> Option<NormalizedOrigin> {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())?;
    normalize_origin(&format!("http://{host}"))
}

/// Length-independent comparison so a token cannot be recovered byte by byte.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Correlation ID for a request, validating a caller-supplied header.
///
/// An invalid label is a hard failure rather than a silent replacement: the
/// caller asked for a specific trace label, and quietly using a different one
/// would make their logs lie.
pub fn resolve_correlation_id(headers: &HeaderMap) -> Result<String, ApiError> {
    match headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
    {
        None => Ok(new_hex_id()),
        Some(value) if super::dto::is_valid_correlation_id(value) => Ok(value.to_string()),
        Some(_) => Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "correlation_id must be 1-64 characters matching [A-Za-z0-9._:-]",
            &new_hex_id(),
        )),
    }
}

/// Build the CORS response headers for an allowed origin.
pub fn cors_headers(allowed_origin: Option<&str>) -> Vec<(axum::http::HeaderName, HeaderValue)> {
    let Some(origin) = allowed_origin else {
        return Vec::new();
    };
    let Ok(origin_value) = HeaderValue::from_str(origin) else {
        return Vec::new();
    };
    vec![
        (
            axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            origin_value,
        ),
        (axum::http::header::VARY, HeaderValue::from_static("Origin")),
        (
            axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        ),
        (
            axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Authorization, Content-Type, X-Correlation-Id"),
        ),
    ]
}

/// Preflight response for an allowed origin.
pub fn preflight_response(allowed_origin: Option<&str>) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    for (name, value) in cors_headers(allowed_origin) {
        response.headers_mut().insert(name, value);
    }
    response
}

/// True when the request is a CORS preflight.
pub fn is_preflight(method: &Method) -> bool {
    method == Method::OPTIONS
}
