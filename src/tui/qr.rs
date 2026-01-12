//! QR code generation for Web UI URL display
//!
//! This module provides functionality to generate QR codes
//! for displaying Web UI URLs in the TUI.

#[cfg(feature = "web-monitoring")]
use qrcode::{render::unicode, QrCode};

/// Generate a QR code string from a URL
///
/// Returns the QR code as a unicode string suitable for terminal display,
/// or an error message if generation fails.
#[cfg(feature = "web-monitoring")]
pub fn generate_qr_string(url: &str) -> Result<String, String> {
    let code = QrCode::new(url).map_err(|e| format!("Failed to generate QR code: {}", e))?;

    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();

    Ok(image)
}

/// Fallback when web-monitoring feature is disabled
#[cfg(not(feature = "web-monitoring"))]
pub fn generate_qr_string(_url: &str) -> Result<String, String> {
    Err("QR code generation requires web-monitoring feature".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "web-monitoring")]
    fn test_generate_qr_string_valid_url() {
        let result = generate_qr_string("http://localhost:8080");
        assert!(result.is_ok());
        let qr = result.unwrap();
        // QR code should contain unicode block characters
        assert!(!qr.is_empty());
        assert!(qr.contains('█') || qr.contains('▀') || qr.contains('▄') || qr.contains(' '));
    }

    #[test]
    #[cfg(feature = "web-monitoring")]
    fn test_generate_qr_string_short_url() {
        let result = generate_qr_string("http://127.0.0.1:3000");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(feature = "web-monitoring")]
    fn test_generate_qr_string_empty_url() {
        // Empty URLs should still work (QR code can encode empty strings)
        let result = generate_qr_string("");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(not(feature = "web-monitoring"))]
    fn test_generate_qr_string_without_feature() {
        let result = generate_qr_string("http://localhost:8080");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("web-monitoring feature"));
    }
}
