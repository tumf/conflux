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

/// Acceptance system prompt - now empty to enforce template-only approach.
/// All acceptance instructions must come from the command template (e.g., .opencode/commands/cflx-accept.md).
/// The acceptance_prompt_mode "full" is now deprecated and behaves identically to "context_only".
#[allow(dead_code)]
pub const ACCEPTANCE_SYSTEM_PROMPT: &str = "";

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

/// Default inactivity timeout for commands (seconds)
/// 0 = disabled
pub const DEFAULT_COMMAND_INACTIVITY_TIMEOUT_SECS: u64 = 900;

/// Default grace period before force-killing inactive commands (seconds)
pub const DEFAULT_COMMAND_INACTIVITY_KILL_GRACE_SECS: u64 = 5;

/// Default maximum number of retries after inactivity timeout (0 = disabled; set to 0 to opt out)
pub const DEFAULT_COMMAND_INACTIVITY_TIMEOUT_MAX_RETRIES: u32 = 3;

/// Default enablement for stream-json output textification
pub const DEFAULT_STREAM_JSON_TEXTIFY: bool = true;

/// Default enablement for strict post-completion process-group cleanup.
/// When true, the orchestrator always runs a SIGTERM→SIGKILL sweep on the
/// spawned process group after a command completes (regardless of exit status).
pub const DEFAULT_COMMAND_STRICT_PROCESS_CLEANUP: bool = true;

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

// ── Proposal session defaults ──────────────────────────────────────────────

/// Default transport command for proposal sessions (ACP subprocess)
pub const DEFAULT_PROPOSAL_TRANSPORT_COMMAND: &str = "opencode";

/// Default transport arguments for proposal sessions (ACP stdio mode)
pub const DEFAULT_PROPOSAL_TRANSPORT_ARGS: &[&str] = &["acp"];

/// Default inactivity timeout for proposal sessions (seconds)
pub const DEFAULT_PROPOSAL_SESSION_INACTIVITY_TIMEOUT_SECS: u64 = 1800;

// ── Server defaults ───────────────────────────────────────────────────────

/// Default server bind address
pub const DEFAULT_SERVER_BIND: &str = "127.0.0.1";

/// Default server port
pub const DEFAULT_SERVER_PORT: u16 = 39876;

/// Default maximum concurrent project executions (server mode)
pub const DEFAULT_SERVER_MAX_CONCURRENT_TOTAL: usize = 4;

/// Default server data directory (relative path component, under XDG_DATA_HOME/cflx/)
pub const DEFAULT_SERVER_DATA_SUBDIR: &str = "server";

/// Returns the default server data directory path.
/// Uses ${XDG_DATA_HOME}/cflx/server, falling back to ~/.local/share/cflx/server.
pub fn default_server_data_dir() -> std::path::PathBuf {
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        if !xdg_data_home.is_empty() {
            return std::path::PathBuf::from(xdg_data_home)
                .join("cflx")
                .join(DEFAULT_SERVER_DATA_SUBDIR);
        }
    }
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".local")
            .join("share")
            .join("cflx")
            .join(DEFAULT_SERVER_DATA_SUBDIR);
    }
    std::env::temp_dir()
        .join("cflx-server-fallback")
        .join(DEFAULT_SERVER_DATA_SUBDIR)
}

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

/// Returns server service log path using XDG_STATE_HOME-compliant defaults.
///
/// - Uses `${XDG_STATE_HOME}/cflx/server.log` when XDG_STATE_HOME is set and non-empty
/// - Falls back to `~/.local/state/cflx/server.log` when home directory is available
/// - Falls back to `{temp_dir}/cflx-server.log` when home directory is unavailable
pub fn get_server_log_path() -> PathBuf {
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok();
    get_server_log_path_from(xdg_state_home.as_deref(), dirs::home_dir())
}

fn get_server_log_path_from(xdg_state_home: Option<&str>, home_dir: Option<PathBuf>) -> PathBuf {
    if let Some(xdg_state_home) = xdg_state_home {
        if !xdg_state_home.is_empty() {
            return PathBuf::from(xdg_state_home)
                .join("cflx")
                .join("server.log");
        }
    }

    if let Some(home) = home_dir {
        return home
            .join(".local")
            .join("state")
            .join("cflx")
            .join("server.log");
    }

    std::env::temp_dir().join("cflx-server.log")
}

/// Generates log file path using XDG_STATE_HOME with project_slug and date.
/// Format: `{XDG_STATE_HOME}/cflx/logs/<project_slug>/<YYYY-MM-DD>.log`
///
/// - **All platforms**: Uses `${XDG_STATE_HOME}/cflx/logs/<project_slug>/<YYYY-MM-DD>.log` if set,
///   otherwise falls back to `~/.local/state/cflx/logs/<project_slug>/<YYYY-MM-DD>.log`.
/// - Fallback: system temp directory with `cflx-logs-fallback/<project_slug>/<YYYY-MM-DD>.log`.
///
/// If `repo_root` is provided, the path includes a project-specific slug to avoid conflicts.
pub fn get_log_file_path(repo_root: Option<&std::path::Path>) -> PathBuf {
    use chrono::Local;

    // Generate project slug if repo_root is provided
    let project_slug = repo_root
        .map(generate_project_slug)
        .unwrap_or_else(|| "unknown".to_string());

    // Get current date in YYYY-MM-DD format
    let date_str = Local::now().format("%Y-%m-%d").to_string();
    let log_filename = format!("{}.log", date_str);

    // Check XDG_STATE_HOME first
    if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(xdg_state_home)
            .join("cflx")
            .join("logs")
            .join(&project_slug)
            .join(&log_filename);
    }

    // Fall back to ~/.local/state
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".local")
            .join("state")
            .join("cflx")
            .join("logs")
            .join(&project_slug)
            .join(&log_filename);
    }

    // Fallback for unsupported platforms or when home directory is not available
    std::env::temp_dir()
        .join("cflx-logs-fallback")
        .join(&project_slug)
        .join(&log_filename)
}

/// Cleans up old log files, retaining only the last N days.
/// Returns the number of files deleted.
pub fn cleanup_old_logs(
    repo_root: Option<&std::path::Path>,
    retain_days: u32,
) -> std::io::Result<usize> {
    use chrono::{Duration, Local};

    let project_slug = repo_root
        .map(generate_project_slug)
        .unwrap_or_else(|| "unknown".to_string());

    // Determine log directory
    let log_dir = if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg_state_home)
            .join("cflx")
            .join("logs")
            .join(&project_slug)
    } else if let Some(home) = dirs::home_dir() {
        home.join(".local")
            .join("state")
            .join("cflx")
            .join("logs")
            .join(&project_slug)
    } else {
        std::env::temp_dir()
            .join("cflx-logs-fallback")
            .join(&project_slug)
    };

    // If log directory doesn't exist, nothing to clean
    if !log_dir.exists() {
        return Ok(0);
    }

    // Calculate cutoff date
    // retain_days = 7 means keep today + previous 6 days (7 total)
    // So cutoff is (retain_days - 1) days ago
    let cutoff_date = Local::now() - Duration::days(i64::from(retain_days - 1));
    let cutoff_str = cutoff_date.format("%Y-%m-%d").to_string();

    let mut deleted_count = 0;

    // Iterate over files in log directory
    for entry in std::fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .log files
        if path.extension().and_then(|s| s.to_str()) != Some("log") {
            continue;
        }

        // Extract date from filename (YYYY-MM-DD.log)
        if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
            // Compare filename (date) with cutoff
            if filename < cutoff_str.as_str() {
                std::fs::remove_file(&path)?;
                deleted_count += 1;
            }
        }
    }

    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_env_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
        unsafe { std::env::set_var(key, value) }
    }

    fn remove_env_var<K: AsRef<std::ffi::OsStr>>(key: K) {
        unsafe { std::env::remove_var(key) }
    }

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
            set_env_var("XDG_DATA_HOME", test_path);

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
                Some(val) => set_env_var("XDG_DATA_HOME", val),
                None => remove_env_var("XDG_DATA_HOME"),
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
    fn test_acceptance_system_prompt_is_empty() {
        // After template-only refactoring, ACCEPTANCE_SYSTEM_PROMPT should be empty.
        // All acceptance instructions must come from the command template.
        assert!(
            ACCEPTANCE_SYSTEM_PROMPT.is_empty(),
            "ACCEPTANCE_SYSTEM_PROMPT should be empty to enforce template-only approach"
        );
    }

    #[test]
    fn test_get_log_file_path_format() {
        let repo_root = PathBuf::from("/tmp/test-repo");
        let log_path = get_log_file_path(Some(&repo_root));
        let path_str = log_path.to_string_lossy();

        // Should contain cflx/logs
        assert!(
            path_str.contains("cflx") && path_str.contains("logs"),
            "Expected path to contain 'cflx/logs', got {:?}",
            log_path
        );

        // Should contain project slug
        assert!(
            path_str.contains("test-repo-"),
            "Expected path to contain project slug 'test-repo-', got {:?}",
            log_path
        );

        // Should end with .log
        assert!(
            path_str.ends_with(".log"),
            "Expected path to end with '.log', got {:?}",
            log_path
        );
    }

    #[test]
    fn test_get_server_log_path_with_xdg_state_home() {
        let result = get_server_log_path_from(
            Some("/custom/state"),
            Some(PathBuf::from("/home/test-user")),
        );

        assert_eq!(result, PathBuf::from("/custom/state/cflx/server.log"));
    }

    #[test]
    fn test_get_server_log_path_without_xdg_state_home() {
        use std::env;

        let original = env::var("XDG_STATE_HOME").ok();
        remove_env_var("XDG_STATE_HOME");

        let home = PathBuf::from("/home/test-user");
        let result = get_server_log_path_from(None, Some(home.clone()));

        assert_eq!(result, home.join(".local/state/cflx/server.log"));

        match original {
            Some(val) => set_env_var("XDG_STATE_HOME", val),
            None => remove_env_var("XDG_STATE_HOME"),
        }
    }

    #[test]
    fn test_get_server_log_path_without_home_directory() {
        use std::env;

        let original = env::var("XDG_STATE_HOME").ok();
        remove_env_var("XDG_STATE_HOME");

        let result = get_server_log_path_from(None, None);

        assert_eq!(result, std::env::temp_dir().join("cflx-server.log"));

        match original {
            Some(val) => set_env_var("XDG_STATE_HOME", val),
            None => remove_env_var("XDG_STATE_HOME"),
        }
    }

    #[test]
    fn test_cleanup_old_logs_with_nonexistent_directory() {
        // Should return Ok(0) for non-existent directory
        let repo_root = PathBuf::from("/tmp/nonexistent-repo-for-test");
        let result = cleanup_old_logs(Some(&repo_root), 7);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_cleanup_old_logs_retains_exactly_n_days() {
        use chrono::Local;
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary XDG_STATE_HOME
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_state_home = temp_dir.path().join("state");
        fs::create_dir(&temp_state_home).expect("Failed to create state dir");

        // Save original XDG_STATE_HOME and set to temp
        let original_state_home = env::var("XDG_STATE_HOME").ok();
        set_env_var("XDG_STATE_HOME", &temp_state_home);

        // Create repo root and get project slug
        let repo_root = PathBuf::from("/tmp/test-retention-repo");
        let project_slug = generate_project_slug(&repo_root);

        // Create log directory
        let log_dir = temp_state_home
            .join("cflx")
            .join("logs")
            .join(&project_slug);
        fs::create_dir_all(&log_dir).expect("Failed to create log dir");

        // Create dated log files: today - 9, today - 8, ..., today - 1, today
        let today = Local::now();
        let mut expected_files = Vec::new();

        for days_ago in (0..=9).rev() {
            let date = today - chrono::Duration::days(days_ago);
            let filename = format!("{}.log", date.format("%Y-%m-%d"));
            let file_path = log_dir.join(&filename);
            fs::write(&file_path, "test log content").expect("Failed to write log file");

            // Files within retain_days (7) should be kept: today, today-1, ..., today-6
            if days_ago < 7 {
                expected_files.push(filename);
            }
        }

        // Run cleanup with retain_days = 7
        let deleted_count = cleanup_old_logs(Some(&repo_root), 7).expect("cleanup_old_logs failed");

        // Should delete exactly 3 files (today-9, today-8, today-7)
        assert_eq!(deleted_count, 3, "Expected to delete 3 files");

        // Verify exactly 7 files remain
        let remaining: Vec<_> = fs::read_dir(&log_dir)
            .expect("Failed to read log dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("log"))
            .map(|e| e.path().file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            remaining.len(),
            7,
            "Expected exactly 7 log files to remain, found: {:?}",
            remaining
        );

        // Verify today's log is preserved
        let today_filename = format!("{}.log", today.format("%Y-%m-%d"));
        assert!(
            remaining.contains(&today_filename),
            "Today's log file should be preserved: {}",
            today_filename
        );

        // Verify all expected files are present
        for expected in &expected_files {
            assert!(
                remaining.contains(expected),
                "Expected file {} should be present",
                expected
            );
        }

        // Restore original XDG_STATE_HOME
        match original_state_home {
            Some(val) => set_env_var("XDG_STATE_HOME", val),
            None => remove_env_var("XDG_STATE_HOME"),
        }
    }
}
