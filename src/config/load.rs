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
    pub fn parse_jsonc(content: &str) -> Result<Self> {
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

        // Validate required commands after merging
        config.validate_required_commands()?;

        info!("Configuration loaded and merged successfully");
        Ok(config)
    }
}
