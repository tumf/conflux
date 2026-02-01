//! Default values and constants for orchestrator configuration.

use std::path::PathBuf;

/// Project-level configuration file name
pub const PROJECT_CONFIG_FILE: &str = ".cflx.jsonc";

/// Global configuration directory name
pub const GLOBAL_CONFIG_DIR: &str = "cflx";

/// Global configuration file name within the config directory
pub const GLOBAL_CONFIG_FILE: &str = "config.jsonc";

/// Default apply command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_APPLY_COMMAND: &str = "opencode run '/openspec-apply {change_id}'";

/// Default archive command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ARCHIVE_COMMAND: &str = "opencode run '/conflux:archive {change_id}'";

/// Default acceptance command template (OpenCode)
/// Supports `{change_id}` and `{prompt}` placeholders
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ACCEPTANCE_COMMAND: &str = "opencode run '/cflx-accept {change_id} {prompt}'";

/// Default resolve command template (OpenCode)
/// Supports `{prompt}` placeholder for the resolve prompt
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_RESOLVE_COMMAND: &str = "opencode run {prompt}";

/// Default analyze command template (OpenCode)
/// Note: No longer used as fallback. Commands must be explicitly configured.
#[allow(dead_code)]
pub const DEFAULT_ANALYZE_COMMAND: &str = "opencode run --format json {prompt}";

/// Default prompt for apply command - includes path context.
/// The hardcoded system prompt in agent.rs is always appended.
pub const DEFAULT_APPLY_PROMPT: &str = r#"
<system-context>
IMPORTANT: You are running in the repository root directory.
The change you are working on is located at: openspec/changes/{change_id}/
All file paths should be relative to the repository root.
</system-context>
"#;

/// Default prompt for archive command - empty by default
pub const DEFAULT_ARCHIVE_PROMPT: &str = "";

/// Hardcoded acceptance prompt - used only when acceptance_prompt_mode is "full".
/// This is a minimal fallback. The recommended approach is to use "context_only" mode
/// with fixed acceptance instructions in .opencode/commands/cflx-accept.md.
/// Contains `{change_id}` placeholder that must be expanded before use.
pub const ACCEPTANCE_SYSTEM_PROMPT: &str = r###"
You are reviewing the implementation for change: {change_id}

IMPORTANT: Only review the specific change "{change_id}".
- Proposal: openspec/changes/{change_id}/proposal.md
- Tasks: openspec/changes/{change_id}/tasks.md
- Spec deltas: openspec/changes/{change_id}/specs/

Do NOT review or report on other changes in openspec/changes/.

NOTE: When using acceptance_prompt_mode "context_only" (recommended), the fixed acceptance
instructions are provided by the command template (e.g., .opencode/commands/cflx-accept.md).
This prompt is only used as a fallback when acceptance_prompt_mode is "full".
"###;

/// Default prompt for acceptance command - appended after hardcoded prompt
pub const DEFAULT_ACCEPTANCE_PROMPT: &str = "";

/// Default maximum iterations for the orchestration loop
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

/// Default maximum concurrent workspaces for parallel execution
pub const DEFAULT_MAX_CONCURRENT_WORKSPACES: usize = 3;

/// Default workspace base directory (uses system temp)
#[allow(dead_code)]
pub const DEFAULT_WORKSPACE_BASE_DIR: &str = "";

/// Default suppression for repetitive debug logs
pub const DEFAULT_SUPPRESS_REPETITIVE_DEBUG: bool = true;

/// Default interval (seconds) for summary log output
pub const DEFAULT_LOG_SUMMARY_INTERVAL_SECS: u64 = 60;

/// Default enablement for stall detection
pub const DEFAULT_STALL_DETECTION_ENABLED: bool = true;

/// Default threshold for consecutive empty WIP commits before stalling
pub const DEFAULT_STALL_DETECTION_THRESHOLD: u32 = 3;

/// Default delay between command executions (milliseconds)
pub const DEFAULT_STAGGER_DELAY_MS: u64 = 2000;

/// Default maximum number of retries for commands
pub const DEFAULT_MAX_RETRIES: u32 = 2;

/// Default delay between retries (milliseconds)
pub const DEFAULT_RETRY_DELAY_MS: u64 = 5000;

/// Default threshold for retry based on execution duration (seconds)
pub const DEFAULT_RETRY_IF_DURATION_UNDER_SECS: u64 = 5;

/// Default error patterns that trigger automatic retry
pub fn default_retry_patterns() -> Vec<String> {
    vec![
        // Module resolution errors
        r"Cannot find module".to_string(),
        r"ResolveMessage:".to_string(),
        // npm/bun registry errors
        r"ENOTFOUND registry\.npmjs\.org".to_string(),
        r"ETIMEDOUT.*registry".to_string(),
        // File lock errors
        r"EBADF.*lock".to_string(),
        r"Lock acquisition failed".to_string(),
    ]
}

/// Default enablement for error circuit breaker
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_ENABLED: bool = true;

/// Default threshold for consecutive same errors before opening circuit
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_THRESHOLD: usize = 5;

/// Default maximum number of acceptance CONTINUE retries before treating as FAIL
pub const DEFAULT_ACCEPTANCE_MAX_CONTINUES: u32 = 10;

/// Default enablement for merge stall detection
pub const DEFAULT_MERGE_STALL_DETECTION_ENABLED: bool = true;

/// Default threshold for merge stall detection (minutes)
pub const DEFAULT_MERGE_STALL_THRESHOLD_MINUTES: u64 = 30;

/// Default check interval for merge stall detection (seconds)
pub const DEFAULT_MERGE_STALL_CHECK_INTERVAL_SECONDS: u64 = 60;

/// Generates a project slug from the repository root path.
/// Format: `{repo_basename}-{hash8}` where hash8 is the first 8 chars of the SHA256 hash
/// of the absolute repository path.
///
/// Example: `/Users/alice/projects/conflux` → `conflux-a1b2c3d4`
fn generate_project_slug(repo_root: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Get repository basename
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Generate hash from absolute path
    let absolute_path = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut hasher = DefaultHasher::new();
    absolute_path.hash(&mut hasher);
    let hash = hasher.finish();
    let hash_str = format!("{:016x}", hash);
    let hash8 = &hash_str[..8];

    format!("{}-{}", repo_name, hash8)
}

/// Returns the default workspace base directory based on the OS and environment.
///
/// - **macOS**: Uses `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>` if `XDG_DATA_HOME` is set,
///   otherwise falls back to `~/.local/share/cflx/worktrees/<project_slug>`.
/// - **Linux**: Uses `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>` if set,
///   otherwise `~/.local/share/cflx/worktrees/<project_slug>`.
/// - **Windows**: Uses `%APPDATA%\cflx\worktrees\<project_slug>`.
/// - **Other**: Falls back to system temp directory with `cflx-workspaces-fallback/<project_slug>`.
///
/// If `repo_root` is provided, the path includes a project-specific slug to avoid conflicts.
/// If `repo_root` is None, returns a generic path without project slug (for backwards compatibility).
pub fn default_workspace_base_dir(repo_root: Option<&std::path::Path>) -> PathBuf {
    // Generate project slug if repo_root is provided
    let project_slug = repo_root.map(generate_project_slug);

    #[cfg(target_os = "macos")]
    {
        // Check XDG_DATA_HOME first
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let mut path = PathBuf::from(xdg_data_home).join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
        // Fall back to ~/.local/share (same as Linux)
        if let Some(home) = dirs::home_dir() {
            let mut path = home
                .join(".local")
                .join("share")
                .join("cflx")
                .join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use XDG_DATA_HOME or fall back to ~/.local/share
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let mut path = PathBuf::from(xdg_data_home).join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
        if let Some(home) = dirs::home_dir() {
            let mut path = home
                .join(".local")
                .join("share")
                .join("cflx")
                .join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use APPDATA directory
        if let Some(appdata) = dirs::data_dir() {
            let mut path = appdata.join("cflx").join("worktrees");
            if let Some(slug) = &project_slug {
                path = path.join(slug);
            }
            return path;
        }
    }

    // Fallback for unsupported platforms or when home directory is not available
    let mut path = std::env::temp_dir().join("cflx-workspaces-fallback");
    if let Some(slug) = &project_slug {
        path = path.join(slug);
    }
    path
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_workspace_base_dir_returns_path() {
        // Test that the function returns a valid PathBuf without repo_root
        let path = default_workspace_base_dir(None);
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_workspace_base_dir_contains_cflx() {
        // Test that the path contains "cflx" or is the fallback
        let path = default_workspace_base_dir(None);
        let path_str = path.to_string_lossy();

        // Should contain "cflx" (case-insensitive) or be the fallback
        let is_cflx_path = path_str.to_lowercase().contains("cflx");
        let is_fallback = path_str.contains("cflx-workspaces-fallback");

        assert!(
            is_cflx_path || is_fallback,
            "Path should contain 'cflx' or be fallback: {:?}",
            path
        );
    }

    #[test]
    fn test_default_workspace_base_dir_with_repo_root() {
        // Test with repo_root parameter
        let repo_root = PathBuf::from("/Users/alice/projects/conflux");
        let path = default_workspace_base_dir(Some(&repo_root));
        let path_str = path.to_string_lossy();

        // Should contain project slug (repo name + hash)
        assert!(
            path_str.contains("conflux-"),
            "Path should contain project slug: {:?}",
            path
        );
    }

    #[test]
    fn test_generate_project_slug() {
        let repo_root = PathBuf::from("/Users/alice/projects/my-repo");
        let slug = generate_project_slug(&repo_root);

        // Should have format: {name}-{hash8}
        assert!(
            slug.starts_with("my-repo-"),
            "Slug should start with repo name"
        );
        assert_eq!(
            slug.len(),
            "my-repo-".len() + 8,
            "Slug should have 8-char hash"
        );
    }

    #[test]
    fn test_default_workspace_base_dir_with_xdg_data_home() {
        // Test XDG_DATA_HOME override (only on Unix-like systems)
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use std::env;

            // Save original value
            let original = env::var("XDG_DATA_HOME").ok();

            // Set XDG_DATA_HOME
            let test_path = "/tmp/test-xdg-data";
            env::set_var("XDG_DATA_HOME", test_path);

            let repo_root = PathBuf::from("/tmp/test-repo");
            let result = default_workspace_base_dir(Some(&repo_root));

            // Should use XDG_DATA_HOME
            assert!(
                result.starts_with(test_path),
                "Expected path to start with {}, got {:?}",
                test_path,
                result
            );
            assert!(
                result.to_string_lossy().contains("cflx/worktrees"),
                "Expected path to contain cflx/worktrees, got {:?}",
                result
            );

            // Restore original value
            match original {
                Some(val) => env::set_var("XDG_DATA_HOME", val),
                None => env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_default_workspace_base_dir_macos_fallback() {
        // NOTE: This test runs in parallel with other tests that may set XDG_DATA_HOME,
        // so we can't rely on environment isolation. We just verify the path contains
        // expected components (cflx/worktrees) and project slug.
        let repo_root = PathBuf::from("/tmp/test-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should contain cflx and worktrees (either from XDG_DATA_HOME or ~/.local/share)
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            result
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_default_workspace_base_dir_linux_fallback() {
        // NOTE: This test may run in parallel with other tests that set XDG_DATA_HOME,
        // so we can't rely on environment isolation. We just check that the path
        // contains the expected components (cflx/worktrees) and project slug.
        let repo_root = PathBuf::from("/tmp/test-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should contain cflx and worktrees
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            result
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_default_workspace_base_dir_windows() {
        let repo_root = PathBuf::from("C:\\Users\\test\\projects\\my-repo");
        let result = default_workspace_base_dir(Some(&repo_root));
        let path_str = result.to_string_lossy();

        // Should use APPDATA on Windows
        assert!(
            path_str.contains("cflx") && path_str.contains("worktrees"),
            "Expected path to contain 'cflx' and 'worktrees', got {:?}",
            result
        );
    }

    #[test]
    fn test_acceptance_system_prompt_is_minimal() {
        // After single-source refactoring, ACCEPTANCE_SYSTEM_PROMPT should be minimal
        // and contain only basic context (used for "full" mode fallback).
        // The detailed instructions are now in .opencode/commands/cflx-accept.md.
        assert!(
            ACCEPTANCE_SYSTEM_PROMPT.contains("{change_id}"),
            "ACCEPTANCE_SYSTEM_PROMPT should contain change_id placeholder"
        );
        assert!(
            ACCEPTANCE_SYSTEM_PROMPT.contains("openspec/changes/{change_id}"),
            "ACCEPTANCE_SYSTEM_PROMPT should reference the change paths"
        );
        assert!(
            ACCEPTANCE_SYSTEM_PROMPT.contains("context_only"),
            "ACCEPTANCE_SYSTEM_PROMPT should mention context_only mode as recommended"
        );
    }
}
