use crate::config::LoggingConfig;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::info;

/// Snapshot of the latest known state for a change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeStateSnapshot {
    pub completed_tasks: u32,
    pub total_tasks: u32,
}

/// Tracks log state to suppress repetitive debug messages.
#[derive(Debug)]
pub struct LogDeduplicator {
    suppress_repetitive_debug: bool,
    summary_interval: Duration,
    last_summary_time: Instant,
    change_states: HashMap<String, ChangeStateSnapshot>,
    last_change_count: Option<usize>,
}

impl LogDeduplicator {
    pub fn new(config: LoggingConfig) -> Self {
        Self {
            suppress_repetitive_debug: config.suppress_repetitive_debug,
            summary_interval: Duration::from_secs(config.summary_interval_secs),
            last_summary_time: Instant::now(),
            change_states: HashMap::new(),
            last_change_count: None,
        }
    }

    /// Returns true if the snapshot differs from the last seen state.
    pub fn should_log(&mut self, change_id: &str, snapshot: ChangeStateSnapshot) -> bool {
        let previous = self.change_states.get(change_id);
        let changed = previous != Some(&snapshot);
        // Always store the latest snapshot so summaries reflect current state.
        self.change_states.insert(change_id.to_string(), snapshot);

        !self.suppress_repetitive_debug || changed
    }

    pub fn should_log_task_progress(
        &mut self,
        change_id: &str,
        completed_tasks: u32,
        total_tasks: u32,
    ) -> bool {
        let _updated = self
            .change_states
            .get(change_id)
            .cloned()
            .unwrap_or_default();
        let snapshot = ChangeStateSnapshot {
            completed_tasks,
            total_tasks,
        };
        self.should_log(change_id, snapshot)
    }

    pub fn should_log_change_count(&mut self, change_count: usize) -> bool {
        let changed = self.last_change_count != Some(change_count);
        self.last_change_count = Some(change_count);

        !self.suppress_repetitive_debug || changed
    }

    pub fn maybe_log_summary(&mut self) {
        if self.summary_interval == Duration::ZERO || self.change_states.is_empty() {
            return;
        }

        if self.last_summary_time.elapsed() < self.summary_interval {
            return;
        }

        let mut entries: Vec<_> = self.change_states.iter().collect();
        entries.sort_by_key(|(change_id, _)| *change_id);

        info!("Status summary: {} changes tracked", entries.len());
        for (change_id, state) in entries {
            info!(
                "  - {}: {}/{} tasks",
                change_id, state.completed_tasks, state.total_tasks
            );
        }

        self.last_summary_time = Instant::now();
    }
}

static LOG_DEDUPLICATOR: OnceLock<Mutex<LogDeduplicator>> = OnceLock::new();

fn global_deduplicator() -> &'static Mutex<LogDeduplicator> {
    LOG_DEDUPLICATOR.get_or_init(|| Mutex::new(LogDeduplicator::new(LoggingConfig::default())))
}

fn with_deduplicator<T>(handler: impl FnOnce(&mut LogDeduplicator) -> T) -> T {
    let mut deduplicator = global_deduplicator()
        .lock()
        .expect("Log deduplicator lock poisoned");
    handler(&mut deduplicator)
}

/// Configure or reset log deduplication behavior.
pub fn configure_logging(config: LoggingConfig) {
    let mut deduplicator = global_deduplicator()
        .lock()
        .expect("Log deduplicator lock poisoned");
    *deduplicator = LogDeduplicator::new(config);
}

pub fn should_log_task_progress(change_id: &str, completed_tasks: u32, total_tasks: u32) -> bool {
    with_deduplicator(|deduplicator| {
        deduplicator.should_log_task_progress(change_id, completed_tasks, total_tasks)
    })
}

pub fn should_log_change_count(change_count: usize) -> bool {
    with_deduplicator(|deduplicator| deduplicator.should_log_change_count(change_count))
}

pub fn maybe_log_summary() {
    with_deduplicator(|deduplicator| deduplicator.maybe_log_summary());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LoggingConfig {
        LoggingConfig {
            suppress_repetitive_debug: true,
            summary_interval_secs: 60,
        }
    }

    #[test]
    fn test_should_log_state_change_detection() {
        let mut deduplicator = LogDeduplicator::new(test_config());
        let snapshot = ChangeStateSnapshot {
            completed_tasks: 0,
            total_tasks: 10,
        };

        assert!(deduplicator.should_log("change-a", snapshot.clone()));
        assert!(!deduplicator.should_log("change-a", snapshot.clone()));
        let updated = ChangeStateSnapshot {
            completed_tasks: 1,
            total_tasks: 10,
        };
        assert!(deduplicator.should_log("change-a", updated));
    }

    #[test]
    fn test_multiple_changes_tracked_independently() {
        let mut deduplicator = LogDeduplicator::new(test_config());

        assert!(deduplicator.should_log_task_progress("change-a", 0, 10));
        assert!(deduplicator.should_log_task_progress("change-b", 0, 8));
        assert!(!deduplicator.should_log_task_progress("change-a", 0, 10));
        assert!(deduplicator.should_log_task_progress("change-a", 1, 10));
    }

    #[test]
    fn test_summary_interval_logic() {
        let mut deduplicator = LogDeduplicator::new(LoggingConfig {
            suppress_repetitive_debug: true,
            summary_interval_secs: 1,
        });
        deduplicator.should_log_task_progress("change-a", 1, 2);

        deduplicator.last_summary_time = Instant::now() - Duration::from_secs(2);
        deduplicator.maybe_log_summary();

        assert!(deduplicator.last_summary_time.elapsed() < Duration::from_secs(1));
    }
}
