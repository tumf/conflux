//! JSONC parser (JSON with Comments).
//!
//! This module provides utilities for parsing JSONC format, which extends
//! standard JSON with:
//! - Single-line comments (`// ...`)
//! - Multi-line comments (`/* ... */`)
//! - Trailing commas before `]` or `}`

use crate::error::{OrchestratorError, Result};
use serde::de::DeserializeOwned;

/// Parse JSONC content into a deserializable type.
///
/// This function strips comments and trailing commas from the input,
/// then parses it as standard JSON.
///
/// # Example
///
/// ```ignore
/// use crate::config::jsonc::parse;
///
/// let jsonc = r#"{
///     // This is a comment
///     "key": "value",
/// }"#;
///
/// let config: MyConfig = parse(jsonc)?;
/// ```
pub fn parse<T: DeserializeOwned>(content: &str) -> Result<T> {
    let json = strip_jsonc_features(content);
    serde_json::from_str(&json)
        .map_err(|e| OrchestratorError::ConfigParse(format!("Failed to parse config: {}", e)))
}

/// Strip JSONC features (comments and trailing commas) from content.
///
/// This function handles:
/// - Single-line comments (`// ...`)
/// - Multi-line comments (`/* ... */`)
/// - Trailing commas before `]` or `}`
///
/// Strings containing comment-like sequences (e.g., URLs) are preserved.
pub fn strip_jsonc_features(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if in_string {
            result.push(c);
            if c == '\\' {
                escape_next = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                result.push(c);
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    // Single-line comment: skip until end of line
                    chars.next(); // consume second '/'
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            break;
                        }
                        chars.next();
                    }
                } else if chars.peek() == Some(&'*') {
                    // Multi-line comment: skip until '*/'
                    chars.next(); // consume '*'
                    while let Some(next) = chars.next() {
                        if next == '*' && chars.peek() == Some(&'/') {
                            chars.next(); // consume '/'
                            break;
                        }
                    }
                } else {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }

    // Remove trailing commas before ] or }
    remove_trailing_commas(&result)
}

/// Remove trailing commas before `]` or `}`.
fn remove_trailing_commas(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == ',' {
            // Look ahead, skipping whitespace, for ] or }
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == ']' || chars[j] == '}') {
                // Skip the comma (trailing comma)
                i += 1;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestConfig {
        apply_command: Option<String>,
        archive_command: Option<String>,
        analyze_command: Option<String>,
        url: Option<String>,
    }

    #[test]
    fn test_parse_simple_json() {
        let json = r#"{
            "apply_command": "test apply {change_id}"
        }"#;
        let config: TestConfig = parse(json).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_single_line_comments() {
        let jsonc = r#"{
            // This is a comment
            "apply_command": "test apply {change_id}"
        }"#;
        let config: TestConfig = parse(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_multi_line_comments() {
        let jsonc = r#"{
            /* This is a
               multi-line comment */
            "apply_command": "test apply {change_id}"
        }"#;
        let config: TestConfig = parse(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_trailing_comma() {
        let jsonc = r#"{
            "apply_command": "test apply {change_id}",
        }"#;
        let config: TestConfig = parse(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_full_example() {
        let jsonc = r#"{
            // Apply command configuration
            "apply_command": "codex run 'openspec-apply {change_id}'",

            /* Archive command - used after change completion */
            "archive_command": "codex run 'openspec-archive {change_id}'",

            // Dependency analysis command
            "analyze_command": "claude '{prompt}'",
        }"#;
        let config: TestConfig = parse(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("codex run 'openspec-apply {change_id}'".to_string())
        );
        assert_eq!(
            config.archive_command,
            Some("codex run 'openspec-archive {change_id}'".to_string())
        );
        assert_eq!(
            config.analyze_command,
            Some("claude '{prompt}'".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_preserves_strings_with_slashes() {
        let jsonc = r#"{
            "apply_command": "opencode run '/openspec-apply {change_id}'"
        }"#;
        let config: TestConfig = parse(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("opencode run '/openspec-apply {change_id}'".to_string())
        );
    }

    #[test]
    fn test_strip_jsonc_preserves_url_in_string() {
        let jsonc = r#"{"url": "https://example.com/path"}"#;
        let stripped = strip_jsonc_features(jsonc);
        assert!(stripped.contains("https://example.com/path"));
    }
}
