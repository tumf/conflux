use crate::error::{OrchestratorError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

const STATE_FILE: &str = ".opencode/orchestrator-state.json";

/// Orchestrator state for persistence and recovery
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrchestratorState {
    pub current_change: Option<String>,
    pub processed_changes: Vec<String>,
    pub archived_changes: Vec<String>,
    pub failed_changes: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub total_iterations: u32,
}

impl OrchestratorState {
    /// Create a new state
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            current_change: None,
            processed_changes: Vec::new(),
            archived_changes: Vec::new(),
            failed_changes: Vec::new(),
            started_at: now,
            last_update: now,
            total_iterations: 0,
        }
    }

    /// Get the state file path
    fn state_path() -> PathBuf {
        PathBuf::from(STATE_FILE)
    }

    /// Load state from file if it exists
    pub fn load() -> Result<Option<Self>> {
        let path = Self::state_path();

        if !path.exists() {
            debug!("State file does not exist: {:?}", path);
            return Ok(None);
        }

        info!("Loading state from: {:?}", path);
        let content = fs::read_to_string(&path)
            .map_err(|e| OrchestratorError::State(format!("Failed to read state file: {}", e)))?;

        let state: Self = serde_json::from_str(&content)
            .map_err(|e| OrchestratorError::State(format!("Failed to parse state file: {}", e)))?;

        Ok(Some(state))
    }

    /// Save state to file
    pub fn save(&self) -> Result<()> {
        let path = Self::state_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                OrchestratorError::State(format!("Failed to create state directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| OrchestratorError::State(format!("Failed to serialize state: {}", e)))?;

        fs::write(&path, content)
            .map_err(|e| OrchestratorError::State(format!("Failed to write state file: {}", e)))?;

        debug!("Saved state to: {:?}", path);
        Ok(())
    }

    /// Reset state by removing the file
    pub fn reset() -> Result<()> {
        let path = Self::state_path();

        if path.exists() {
            info!("Resetting state: {:?}", path);
            fs::remove_file(&path).map_err(|e| {
                OrchestratorError::State(format!("Failed to remove state file: {}", e))
            })?;
        } else {
            info!("No state file to reset");
        }

        Ok(())
    }

    /// Update last_update timestamp
    pub fn touch(&mut self) {
        self.last_update = Utc::now();
    }
}

impl Default for OrchestratorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = OrchestratorState::new();
        assert_eq!(state.current_change, None);
        assert_eq!(state.processed_changes.len(), 0);
        assert_eq!(state.archived_changes.len(), 0);
        assert_eq!(state.failed_changes.len(), 0);
        assert_eq!(state.total_iterations, 0);
    }

    #[test]
    fn test_state_serialization() {
        let state = OrchestratorState::new();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: OrchestratorState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.total_iterations, deserialized.total_iterations);
        assert_eq!(state.current_change, deserialized.current_change);
    }
}
