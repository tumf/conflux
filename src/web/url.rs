//! URL conversion utilities for Web UI access.
//!
//! This module provides functions to convert server bind addresses
//! into accessible URLs for mobile devices.

/// Get the local IP address for network access.
///
/// Returns the local network IP address (e.g., 192.168.1.100)
/// that can be used to access the server from other devices on the network.
#[cfg(feature = "web-monitoring")]
pub fn get_local_ip() -> Option<String> {
    local_ip_address::local_ip().ok().map(|ip| ip.to_string())
}

/// Fallback when web-monitoring feature is disabled.
#[cfg(not(feature = "web-monitoring"))]
pub fn get_local_ip() -> Option<String> {
    None
}

/// Build an accessible URL from bind address and actual port.
///
/// Converts server bind configuration into a URL that can be accessed
/// from mobile devices:
/// - `0.0.0.0` is converted to the local network IP address
/// - `127.0.0.1` or `localhost` remains as `localhost`
/// - Other addresses are used as-is
///
/// # Arguments
/// * `bind_addr` - The address the server is bound to
/// * `actual_port` - The actual port number (important when port 0 is used for auto-assignment)
///
/// # Returns
/// A URL string like `http://192.168.1.100:8080`
pub fn build_access_url(bind_addr: &str, actual_port: u16) -> String {
    let host = match bind_addr {
        "0.0.0.0" => get_local_ip().unwrap_or_else(|| {
            tracing::warn!("Could not determine local IP, using localhost (limited accessibility)");
            "localhost".to_string()
        }),
        "127.0.0.1" | "localhost" => "localhost".to_string(),
        addr => addr.to_string(),
    };
    format!("http://{}:{}", host, actual_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_access_url_localhost() {
        let url = build_access_url("127.0.0.1", 8080);
        assert_eq!(url, "http://localhost:8080");

        let url = build_access_url("localhost", 3000);
        assert_eq!(url, "http://localhost:3000");
    }

    #[test]
    fn test_build_access_url_specific_address() {
        let url = build_access_url("192.168.1.50", 9000);
        assert_eq!(url, "http://192.168.1.50:9000");
    }

    #[test]
    fn test_build_access_url_zero_address() {
        // When bound to 0.0.0.0, should try to get local IP
        let url = build_access_url("0.0.0.0", 8080);
        // Either returns local IP or falls back to localhost
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(":8080"));
        // Should not contain 0.0.0.0 in the URL
        assert!(!url.contains("0.0.0.0"));
    }

    #[test]
    #[cfg(feature = "web-monitoring")]
    fn test_get_local_ip_returns_valid_ip() {
        // This test may fail in environments without network interfaces
        // but should work in most development environments
        if let Some(ip) = get_local_ip() {
            // Should be a valid IP address format (not 127.0.0.1)
            assert!(!ip.is_empty());
            // Should not be localhost
            assert_ne!(ip, "127.0.0.1");
        }
    }

    #[test]
    #[cfg(not(feature = "web-monitoring"))]
    fn test_get_local_ip_without_feature() {
        // Without the feature, should return None
        assert!(get_local_ip().is_none());
    }
}
