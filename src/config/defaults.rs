//! Default values and constants for orchestrator configuration.

use std::path::PathBuf;

/// Project-level configuration file name
pub const PROJECT_CONFIG_FILE: &str = ".cflx.jsonc";

/// Global configuration directory name
pub const GLOBAL_CONFIG_DIR: &str = "cflx";

/// Global configuration file name within the config directory
pub const GLOBAL_CONFIG_FILE: &str = "config.jsonc";

/// Default apply command template (OpenCode)
pub const DEFAULT_APPLY_COMMAND: &str = "opencode run '/openspec-apply {change_id}'";

/// Default archive command template (OpenCode)
pub const DEFAULT_ARCHIVE_COMMAND: &str = "opencode run '/conflux:archive {change_id}'";

/// Default resolve command template (OpenCode)
/// Supports `{prompt}` placeholder for the resolve prompt
pub const DEFAULT_RESOLVE_COMMAND: &str = "opencode run {prompt}";

/// Default analyze command template (OpenCode)
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

/// Default prompt for archive command - includes path context
pub const DEFAULT_ARCHIVE_PROMPT: &str = r#"
<system-context>
IMPORTANT: You are running in the repository root directory.
To archive the change, move the directory from:
  openspec/changes/{change_id}/
to:
  openspec/specs/{change_id}/

All file paths should be relative to the repository root.
</system-context>
"#;

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

/// Returns the default workspace base directory based on the OS and environment.
///
/// - **macOS**: Uses `${XDG_DATA_HOME}/openspec/worktrees` if `XDG_DATA_HOME` is set,
///   otherwise falls back to `~/Library/Application Support/openspec/worktrees`.
/// - **Linux**: Uses `${XDG_DATA_HOME}/openspec/worktrees` if set,
///   otherwise `~/.local/share/openspec/worktrees`.
/// - **Windows**: Uses `%APPDATA%\OpenSpec\worktrees`.
/// - **Other**: Falls back to system temp directory with `openspec-workspaces-fallback`.
pub fn default_workspace_base_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        // Check XDG_DATA_HOME first
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg_data_home)
                .join("openspec")
                .join("worktrees");
        }
        // Fall back to macOS standard Application Support
        if let Some(home) = dirs::home_dir() {
            return home
                .join("Library")
                .join("Application Support")
                .join("openspec")
                .join("worktrees");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use XDG_DATA_HOME or fall back to ~/.local/share
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg_data_home)
                .join("openspec")
                .join("worktrees");
        }
        if let Some(home) = dirs::home_dir() {
            return home
                .join(".local")
                .join("share")
                .join("openspec")
                .join("worktrees");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Use APPDATA directory
        if let Some(appdata) = dirs::data_dir() {
            return appdata.join("OpenSpec").join("worktrees");
        }
    }

    // Fallback for unsupported platforms or when home directory is not available
    std::env::temp_dir().join("openspec-workspaces-fallback")
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_workspace_base_dir_returns_path() {
        // Test that the function returns a valid PathBuf
        let path = default_workspace_base_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_workspace_base_dir_contains_openspec() {
        // Test that the path contains "openspec" or is the fallback
        let path = default_workspace_base_dir();
        let path_str = path.to_string_lossy();

        // Should contain "openspec" (case-insensitive) or be the fallback
        let is_openspec_path = path_str.to_lowercase().contains("openspec");
        let is_fallback = path_str.contains("openspec-workspaces-fallback");

        assert!(
            is_openspec_path || is_fallback,
            "Path should contain 'openspec' or be fallback: {:?}",
            path
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

            let result = default_workspace_base_dir();

            // Should use XDG_DATA_HOME
            assert!(
                result.starts_with(test_path),
                "Expected path to start with {}, got {:?}",
                test_path,
                result
            );
            assert!(
                result.ends_with("openspec/worktrees"),
                "Expected path to end with openspec/worktrees, got {:?}",
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
        use std::env;

        // Save and remove XDG_DATA_HOME to test fallback
        let original = env::var("XDG_DATA_HOME").ok();
        env::remove_var("XDG_DATA_HOME");

        let result = default_workspace_base_dir();
        let path_str = result.to_string_lossy();

        // Should use Application Support on macOS when XDG_DATA_HOME is not set
        let expected_contains = vec!["Library/Application Support", "openspec", "worktrees"];
        for part in expected_contains {
            assert!(
                path_str.contains(part),
                "Expected path to contain '{}', got {:?}",
                part,
                result
            );
        }

        // Restore original value
        if let Some(val) = original {
            env::set_var("XDG_DATA_HOME", val);
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_default_workspace_base_dir_linux_fallback() {
        use std::env;

        // Save and remove XDG_DATA_HOME to test fallback
        let original = env::var("XDG_DATA_HOME").ok();
        env::remove_var("XDG_DATA_HOME");

        let result = default_workspace_base_dir();
        let path_str = result.to_string_lossy();

        // Should use .local/share on Linux when XDG_DATA_HOME is not set
        let expected_contains = vec![".local/share", "openspec", "worktrees"];
        for part in expected_contains {
            assert!(
                path_str.contains(part),
                "Expected path to contain '{}', got {:?}",
                part,
                result
            );
        }

        // Restore original value
        if let Some(val) = original {
            env::set_var("XDG_DATA_HOME", val);
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_default_workspace_base_dir_windows() {
        let result = default_workspace_base_dir();
        let path_str = result.to_string_lossy();

        // Should use APPDATA on Windows
        assert!(
            path_str.contains("OpenSpec") && path_str.contains("worktrees"),
            "Expected path to contain 'OpenSpec' and 'worktrees', got {:?}",
            result
        );
    }
}
