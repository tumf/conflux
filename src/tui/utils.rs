//! Utility functions for the TUI
//!
//! Contains helper functions used across TUI modules.

use std::path::Path;
use std::process::Command;

use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use tracing::info;
use unicode_width::UnicodeWidthStr;

use crate::error::{OrchestratorError, Result};

/// Launch editor in the specified change directory
///
/// Opens the user's configured editor ($EDITOR) in the change directory.
/// Falls back to "vi" if $EDITOR is not set.
pub fn launch_editor_for_change(change_id: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let change_dir = Path::new("openspec/changes").join(change_id);

    if !change_dir.exists() {
        return Err(OrchestratorError::ChangeNotFound(change_id.to_string()));
    }

    info!(
        module = module_path!(),
        "Launching editor: {} (cwd: {:?})", editor, change_dir
    );
    Command::new(&editor)
        .arg(".")
        .current_dir(&change_dir)
        .status()
        .map_err(|e| OrchestratorError::EditorLaunchFailed(e.to_string()))?;

    Ok(())
}

/// Truncate a string to fit within a specified display width.
///
/// This function respects Unicode character display widths, where CJK characters
/// (e.g., Japanese, Chinese) typically occupy 2 terminal columns, while ASCII
/// characters occupy 1 column.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_width` - The maximum display width in terminal columns
///
/// # Returns
/// A truncated string with "..." appended if truncation occurred
pub fn truncate_to_display_width(s: &str, max_width: usize) -> String {
    let display_width = s.width();
    if display_width <= max_width {
        return s.to_string();
    }

    // Reserve space for "..." (3 columns)
    let target_width = max_width.saturating_sub(3);
    let mut result = String::new();
    let mut current_width = 0;

    for ch in s.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + char_width > target_width {
            break;
        }
        result.push(ch);
        current_width += char_width;
    }

    result.push_str("...");
    result
}

/// Clear the terminal screen
pub fn clear_screen() -> Result<()> {
    use std::io::stdout;

    execute!(stdout(), Clear(ClearType::All))?;

    Ok(())
}

/// Get version string for display
pub fn get_version_string() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_display_width_short_string() {
        let s = "hello";
        let result = truncate_to_display_width(s, 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_to_display_width_exact_fit() {
        let s = "hello";
        let result = truncate_to_display_width(s, 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_to_display_width_needs_truncation() {
        let s = "hello world";
        let result = truncate_to_display_width(s, 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_to_display_width_cjk() {
        // CJK characters are 2 columns wide
        let s = "日本語テスト";
        // 6 characters * 2 columns = 12 columns
        let result = truncate_to_display_width(s, 8);
        // Should fit "日本" (4 columns) + "..." (3 columns) = 7 columns
        assert_eq!(result, "日本...");
    }

    #[test]
    fn test_get_version_string() {
        let version = get_version_string();
        assert!(version.starts_with("v"));
    }
}
