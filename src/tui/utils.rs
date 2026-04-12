//! Utility functions for the TUI
//!
//! Contains helper functions used across TUI modules.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use tracing::info;
use unicode_width::UnicodeWidthStr;

use crate::error::{OrchestratorError, Result};

fn find_change_dir(change_id: &str, base_path: Option<&Path>) -> Option<PathBuf> {
    let changes_root = match base_path {
        Some(base) => base.join("openspec/changes"),
        None => Path::new("openspec/changes").to_path_buf(),
    };

    let active_dir = changes_root.join(change_id);
    if active_dir.exists() {
        return Some(active_dir);
    }

    let archive_dir = changes_root.join("archive");
    fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            (name_str == change_id || name_str.ends_with(&format!("-{}", change_id)))
                .then(|| entry.path())
        })
}

enum EditorTarget {
    Proposal(PathBuf),
    Directory(PathBuf),
}

fn resolve_editor_target(change_id: &str, base_path: Option<&Path>) -> Result<EditorTarget> {
    let change_dir = find_change_dir(change_id, base_path)
        .ok_or_else(|| OrchestratorError::ChangeNotFound(change_id.to_string()))?;
    let proposal_path = change_dir.join("proposal.md");

    if proposal_path.exists() {
        Ok(EditorTarget::Proposal(proposal_path))
    } else if change_dir.exists() {
        Ok(EditorTarget::Directory(change_dir))
    } else {
        Err(OrchestratorError::ChangeNotFound(change_id.to_string()))
    }
}

/// Launch editor for the specified change
///
/// Opens the user's configured editor ($EDITOR) with the change's proposal.md file.
/// Archived changes are resolved from `openspec/changes/archive/` as well.
/// If proposal.md does not exist, falls back to opening the change directory.
/// Falls back to "vi" if $EDITOR is not set.
pub fn launch_editor_for_change(change_id: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    match resolve_editor_target(change_id, None)? {
        EditorTarget::Proposal(proposal_path) => {
            info!(
                module = module_path!(),
                "Launching editor: {} (file: {:?})", editor, proposal_path
            );
            Command::new(&editor)
                .arg(&proposal_path)
                .status()
                .map_err(|e| OrchestratorError::EditorLaunchFailed(e.to_string()))?;
        }
        EditorTarget::Directory(change_dir) => {
            info!(
                module = module_path!(),
                "Launching editor: {} (cwd: {:?})", editor, change_dir
            );
            Command::new(&editor)
                .arg(".")
                .current_dir(&change_dir)
                .status()
                .map_err(|e| OrchestratorError::EditorLaunchFailed(e.to_string()))?;
        }
    }

    Ok(())
}

/// Launch editor in the specified directory
///
/// Opens the user's configured editor ($EDITOR) in the given directory.
/// Falls back to "vi" if $EDITOR is not set.
pub fn launch_editor_in_dir(dir_path: &str) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let path = Path::new(dir_path);

    if !path.exists() {
        return Err(OrchestratorError::ChangeNotFound(format!(
            "Directory not found: {}",
            dir_path
        )));
    }

    info!(
        module = module_path!(),
        "Launching editor: {} (cwd: {:?})", editor, path
    );

    Command::new(&editor)
        .arg(".")
        .current_dir(path)
        .status()
        .map_err(|e| OrchestratorError::EditorLaunchFailed(e.to_string()))?;

    Ok(())
}

/// Truncate a string to fit within a specified display width with a custom suffix.
///
/// This function respects Unicode character display widths, where CJK characters
/// (e.g., Japanese, Chinese) typically occupy 2 terminal columns, while ASCII
/// characters occupy 1 column. It ensures that character boundaries are not broken,
/// preventing panics on multi-byte UTF-8 characters.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_width` - The maximum display width in terminal columns
/// * `suffix` - The suffix to append if truncation occurred (e.g., "...", "…")
///
/// # Returns
/// A truncated string with the custom suffix appended if truncation occurred
pub fn truncate_to_display_width_with_suffix(s: &str, max_width: usize, suffix: &str) -> String {
    let display_width = s.width();
    if display_width <= max_width {
        return s.to_string();
    }

    // Calculate suffix width
    let suffix_width = suffix.width();

    // Reserve space for suffix
    let target_width = max_width.saturating_sub(suffix_width);
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

    result.push_str(suffix);
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
    format!(
        "cflx v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_NUMBER")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cflx-{}-{}", prefix, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_find_change_dir_prefers_active_change() {
        let temp_dir = make_temp_dir("active-change");

        let change_id = "active-change";
        let active_dir = temp_dir.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&active_dir).unwrap();

        let found = find_change_dir(change_id, Some(&temp_dir));
        assert_eq!(found, Some(active_dir));

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_find_change_dir_falls_back_to_archived_change() {
        let temp_dir = make_temp_dir("archived-change");

        let change_id = "archived-change";
        let archive_dir = temp_dir
            .join("openspec/changes/archive")
            .join(format!("2026-04-11-{}", change_id));
        std::fs::create_dir_all(&archive_dir).unwrap();

        let found = find_change_dir(change_id, Some(&temp_dir));
        assert_eq!(found, Some(archive_dir));

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_resolve_editor_target_prefers_proposal_file() {
        let temp_dir = make_temp_dir("proposal-target");
        let change_id = "proposal-target";
        let change_dir = temp_dir.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        let proposal_path = change_dir.join("proposal.md");
        std::fs::write(&proposal_path, "# proposal\n").unwrap();

        let target = resolve_editor_target(change_id, Some(&temp_dir)).unwrap();
        match target {
            EditorTarget::Proposal(path) => assert_eq!(path, proposal_path),
            EditorTarget::Directory(path) => {
                panic!("expected proposal target, got directory target: {:?}", path)
            }
        }

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_resolve_editor_target_falls_back_to_directory_when_missing_proposal() {
        let temp_dir = make_temp_dir("directory-target");
        let change_id = "directory-target";
        let archive_dir = temp_dir
            .join("openspec/changes/archive")
            .join(format!("2026-04-11-{}", change_id));
        std::fs::create_dir_all(&archive_dir).unwrap();

        let target = resolve_editor_target(change_id, Some(&temp_dir)).unwrap();
        match target {
            EditorTarget::Directory(path) => assert_eq!(path, archive_dir),
            EditorTarget::Proposal(path) => {
                panic!("expected directory target, got proposal target: {:?}", path)
            }
        }

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_truncate_to_display_width_with_suffix_short_string() {
        let s = "hello";
        let result = truncate_to_display_width_with_suffix(s, 10, "...");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_to_display_width_with_suffix_exact_fit() {
        let s = "hello";
        let result = truncate_to_display_width_with_suffix(s, 5, "...");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_to_display_width_with_suffix_needs_truncation() {
        let s = "hello world";
        let result = truncate_to_display_width_with_suffix(s, 8, "...");
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_to_display_width_with_suffix_cjk() {
        // CJK characters are 2 columns wide
        let s = "日本語テスト";
        // 6 characters * 2 columns = 12 columns
        let result = truncate_to_display_width_with_suffix(s, 8, "...");
        // Should fit "日本" (4 columns) + "..." (3 columns) = 7 columns
        assert_eq!(result, "日本...");
    }

    #[test]
    fn test_truncate_to_display_width_with_suffix_emoji() {
        let s = "日本😀語";
        let result = truncate_to_display_width_with_suffix(s, 6, "...");
        assert_eq!(result, "日...");
    }

    #[test]
    fn test_get_version_string() {
        let version = get_version_string();
        // Format is "cflx v{version} ({build_number})"
        assert!(version.starts_with("cflx v"));
        // Should contain build number in parentheses
        assert!(version.contains('('));
        assert!(version.contains(')'));
        // Build number should be 14 digits (YYYYMMDDHHmmss)
        let parts: Vec<&str> = version.split('(').collect();
        assert_eq!(parts.len(), 2);
        let build = parts[1].trim_end_matches(')');
        assert_eq!(build.len(), 14);
        assert!(build.chars().all(|c| c.is_ascii_digit()));
    }
}
