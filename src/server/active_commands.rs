//! Active command registry for worktree root-level singleton execution.
//!
//! Tracks which worktree roots (including base) currently have an active command,
//! preventing concurrent operations on the same root. The registry is in-memory only
//! and does not survive server restarts.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::remote::types::ActiveCommand;

/// Identifies a worktree root for active command tracking.
///
/// The key is `(project_id, root_kind)` where `root_kind` distinguishes
/// between the base worktree and per-change worktrees.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct WorktreeRootKey {
    pub project_id: String,
    pub root_kind: RootKind,
}

/// Distinguishes base roots from individual worktree roots.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum RootKind {
    /// The project's base worktree (branch checkout root).
    Base,
    /// A named worktree (typically a change branch).
    Worktree(String),
}

impl std::fmt::Display for RootKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootKind::Base => write!(f, "base"),
            RootKind::Worktree(branch) => write!(f, "worktree:{}", branch),
        }
    }
}

/// Thread-safe active command registry.
pub type SharedActiveCommands = Arc<RwLock<ActiveCommandRegistry>>;

/// In-memory registry of active commands, keyed by worktree root.
pub struct ActiveCommandRegistry {
    commands: HashMap<WorktreeRootKey, ActiveCommand>,
}

impl ActiveCommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Try to acquire a slot for a worktree root. Returns `Ok(())` if the slot
    /// was free, or `Err(ActiveCommand)` with the existing command if the root
    /// is already busy.
    pub fn try_acquire(
        &mut self,
        key: WorktreeRootKey,
        operation: &str,
    ) -> Result<(), ActiveCommand> {
        if let Some(existing) = self.commands.get(&key) {
            debug!(
                project_id = %key.project_id,
                root = %key.root_kind,
                existing_operation = %existing.operation,
                new_operation = %operation,
                "Active command conflict: root is busy"
            );
            return Err(existing.clone());
        }

        let root_display = key.root_kind.to_string();
        let cmd = ActiveCommand {
            project_id: key.project_id.clone(),
            root: root_display,
            operation: operation.to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        };

        info!(
            project_id = %key.project_id,
            root = %key.root_kind,
            operation = %operation,
            "Active command acquired"
        );

        self.commands.insert(key, cmd);
        Ok(())
    }

    /// Release the slot for a worktree root. No-op if no command is registered.
    pub fn release(&mut self, key: &WorktreeRootKey) {
        if self.commands.remove(key).is_some() {
            info!(
                project_id = %key.project_id,
                root = %key.root_kind,
                "Active command released"
            );
        }
    }

    /// Check whether a worktree root is currently busy.
    #[allow(dead_code)]
    pub fn is_busy(&self, key: &WorktreeRootKey) -> bool {
        self.commands.contains_key(key)
    }

    /// Snapshot of all active commands (for full_state and REST).
    pub fn snapshot(&self) -> Vec<ActiveCommand> {
        self.commands.values().cloned().collect()
    }
}

/// Create a new shared active command registry.
pub fn create_shared_active_commands() -> SharedActiveCommands {
    Arc::new(RwLock::new(ActiveCommandRegistry::new()))
}

/// RAII guard that releases the active command slot on drop.
///
/// Because the release requires an async write lock, we provide an explicit
/// `release()` method and also implement `Drop` with a best-effort try_write.
pub struct ActiveCommandGuard {
    registry: SharedActiveCommands,
    key: Option<WorktreeRootKey>,
}

impl ActiveCommandGuard {
    /// Create a guard that will release `key` from `registry` when dropped.
    pub fn new(registry: SharedActiveCommands, key: WorktreeRootKey) -> Self {
        Self {
            registry,
            key: Some(key),
        }
    }

    /// Explicitly release the active command (async-friendly).
    #[allow(dead_code)]
    pub async fn release(mut self) {
        if let Some(key) = self.key.take() {
            let mut reg = self.registry.write().await;
            reg.release(&key);
        }
    }
}

impl Drop for ActiveCommandGuard {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // Best-effort synchronous release. In practice, callers should
            // use the async `release()` method to guarantee cleanup.
            if let Ok(mut reg) = self.registry.try_write() {
                reg.release(&key);
            } else {
                tracing::warn!(
                    project_id = %key.project_id,
                    root = %key.root_kind,
                    "ActiveCommandGuard dropped without async release; slot may leak until next try"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(project: &str, kind: RootKind) -> WorktreeRootKey {
        WorktreeRootKey {
            project_id: project.to_string(),
            root_kind: kind,
        }
    }

    #[test]
    fn test_acquire_and_release() {
        let mut reg = ActiveCommandRegistry::new();
        let key = make_key("p1", RootKind::Base);

        assert!(reg.try_acquire(key.clone(), "sync").is_ok());
        assert!(reg.is_busy(&key));

        reg.release(&key);
        assert!(!reg.is_busy(&key));
    }

    #[test]
    fn test_double_acquire_fails() {
        let mut reg = ActiveCommandRegistry::new();
        let key = make_key("p1", RootKind::Base);

        assert!(reg.try_acquire(key.clone(), "sync").is_ok());
        let err = reg.try_acquire(key.clone(), "merge");
        assert!(err.is_err());

        let existing = err.unwrap_err();
        assert_eq!(existing.operation, "sync");
    }

    #[test]
    fn test_different_roots_independent() {
        let mut reg = ActiveCommandRegistry::new();
        let base = make_key("p1", RootKind::Base);
        let wt = make_key("p1", RootKind::Worktree("feature-x".to_string()));

        assert!(reg.try_acquire(base.clone(), "sync").is_ok());
        assert!(reg.try_acquire(wt.clone(), "apply").is_ok());

        assert!(reg.is_busy(&base));
        assert!(reg.is_busy(&wt));

        reg.release(&base);
        assert!(!reg.is_busy(&base));
        assert!(reg.is_busy(&wt));
    }

    #[test]
    fn test_snapshot() {
        let mut reg = ActiveCommandRegistry::new();
        let key1 = make_key("p1", RootKind::Base);
        let key2 = make_key("p2", RootKind::Worktree("feat".to_string()));

        reg.try_acquire(key1, "sync").unwrap();
        reg.try_acquire(key2, "merge").unwrap();

        let snap = reg.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[tokio::test]
    async fn test_guard_release_async() {
        let shared = create_shared_active_commands();
        let key = make_key("p1", RootKind::Base);

        {
            let mut reg = shared.write().await;
            reg.try_acquire(key.clone(), "sync").unwrap();
        }

        let guard = ActiveCommandGuard::new(shared.clone(), key.clone());
        guard.release().await;

        let reg = shared.read().await;
        assert!(!reg.is_busy(&key));
    }

    #[test]
    fn test_root_kind_display() {
        assert_eq!(RootKind::Base.to_string(), "base");
        assert_eq!(
            RootKind::Worktree("feat-x".to_string()).to_string(),
            "worktree:feat-x"
        );
    }
}
