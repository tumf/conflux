//! Configuration loading: file I/O methods on `OrchestratorConfig`.
//!
//! Path-resolution helpers live in `mod.rs` (the facade) so that tests can
//! reach them via `super::*`.  This module only contains the `impl` blocks
//! that perform actual file I/O.

use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};

use super::defaults::PROJECT_CONFIG_FILE;
use super::jsonc;
use super::types::OrchestratorConfig;
// Path helpers are defined in the parent (mod.rs) and accessed via super::
use super::get_global_config_paths;

/// Configuration keys that selected the removed serial execution mode.
///
/// Detection is limited to these known retired keys; the general unknown-key
/// policy is unchanged. A silently ignored `parallel_mode` would let a stale
/// config claim it still selects an execution mode that no longer exists.
const RETIRED_KEYS: [(&str, &str); 1] = [(
    "parallel_mode",
    "cumulative Git-worktree orchestration is the only execution model; \
     remove \"parallel_mode\" from your Conflux configuration",
)];

/// Reject a configuration document that still carries a retired key.
fn reject_retired_keys(document: &str) -> Result<()> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(document)
    else {
        // Not an object, or not parseable: the ordinary parse below owns the
        // diagnostic for that.
        return Ok(());
    };

    for (key, guidance) in RETIRED_KEYS {
        if map.contains_key(key) {
            return Err(OrchestratorError::ConfigParse(format!(
                "retired configuration key \"{key}\": {guidance}"
            )));
        }
    }

    Ok(())
}

// ── OrchestratorConfig: file loading ──────────────────────────────────────

impl OrchestratorConfig {
    /// Load configuration from a JSONC file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        Self::parse_jsonc(&content).map_err(|err| match err {
            OrchestratorError::ConfigParse(msg) => OrchestratorError::ConfigParse(format!(
                "Failed to parse config file {:?}: {}",
                path, msg
            )),
            other => other,
        })
    }

    /// Parse JSONC content (JSON with Comments)
    ///
    /// Retired keys are rejected here, before any caller can start orchestration
    /// side effects with a configuration that names an execution mode.
    pub fn parse_jsonc(content: &str) -> Result<Self> {
        let document = jsonc::strip_jsonc_features(content);
        reject_retired_keys(&document)?;
        jsonc::parse(content)
    }

    /// Load configuration with merge-based priority:
    /// 1. Start with platform default config (lowest priority)
    /// 2. Merge XDG config (default path) if exists
    /// 3. Merge XDG config (environment variable path) if exists
    /// 4. Merge project config if exists
    /// 5. Merge custom config if provided (highest priority)
    ///
    /// For each field, the last config that has `Some` value wins.
    /// This allows partial configs to inherit from global configs.
    ///
    /// After merging, validates that all required commands are present.
    pub fn load(custom_path: Option<&Path>) -> Result<Self> {
        let config = Self::merge_all_sources(custom_path)?;

        // Validate required commands after merging
        config.validate_required_commands()?;

        info!("Configuration loaded and merged successfully");
        Ok(config)
    }

    /// Load only what a read-only surface needs: the merged storage settings,
    /// without validating that AI commands are configured.
    ///
    /// `cflx logs` has to resolve the same state root the writers use, but it
    /// starts nothing, so an installation that has not configured its commands
    /// yet must still be able to read its logs.
    pub fn load_storage_settings(custom_path: Option<&Path>) -> Result<Self> {
        Self::merge_all_sources(custom_path)
    }

    /// Merge every configuration source in priority order without validating.
    fn merge_all_sources(custom_path: Option<&Path>) -> Result<Self> {
        let mut config = Self::default();

        // 1-3. Global config candidates (low → high priority)
        for path in get_global_config_paths() {
            if path.exists() {
                debug!("Loading global config from: {:?}", path);
                let loaded_config = Self::load_from_file(&path)?;
                config.merge(loaded_config);
            }
        }

        // 4. Project config (higher priority than global)
        let project_config_path = PathBuf::from(PROJECT_CONFIG_FILE);
        if project_config_path.exists() {
            debug!("Loading project config from: {:?}", project_config_path);
            let project_config = Self::load_from_file(&project_config_path)?;
            config.merge(project_config);
        }

        // 5. Custom config path (highest priority)
        if let Some(path) = custom_path {
            debug!("Loading custom config from: {:?}", path);
            let custom_config = Self::load_from_file(path)?;
            config.merge(custom_config);
        }

        Ok(config)
    }
}
