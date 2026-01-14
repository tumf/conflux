use crate::config::StallDetectionConfig;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StallPhase {
    Apply,
    #[allow(dead_code)]
    Archive,
}

#[derive(Debug, Default, Clone)]
struct StallCounters {
    apply: u32,
    archive: u32,
}

#[derive(Debug, Clone)]
pub struct StallDetector {
    config: StallDetectionConfig,
    counts: HashMap<String, StallCounters>,
}

impl StallDetector {
    pub fn new(config: StallDetectionConfig) -> Self {
        let threshold = config.threshold.max(1);
        Self {
            config: StallDetectionConfig {
                threshold,
                ..config
            },
            counts: HashMap::new(),
        }
    }

    pub fn config(&self) -> &StallDetectionConfig {
        &self.config
    }

    pub fn register_commit(&mut self, change_id: &str, phase: StallPhase, is_empty: bool) -> bool {
        if !self.config.enabled {
            return false;
        }

        let counters = self.counts.entry(change_id.to_string()).or_default();
        let counter = match phase {
            StallPhase::Apply => &mut counters.apply,
            StallPhase::Archive => &mut counters.archive,
        };

        if is_empty {
            *counter = counter.saturating_add(1);
        } else {
            *counter = 0;
        }

        *counter >= self.config.threshold
    }

    pub fn current_count(&self, change_id: &str, phase: StallPhase) -> u32 {
        self.counts
            .get(change_id)
            .map(|counters| match phase {
                StallPhase::Apply => counters.apply,
                StallPhase::Archive => counters.archive,
            })
            .unwrap_or(0)
    }

    pub fn clear_change(&mut self, change_id: &str) {
        self.counts.remove(change_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_config(threshold: u32) -> StallDetectionConfig {
        StallDetectionConfig {
            enabled: true,
            threshold,
        }
    }

    #[test]
    fn test_stall_detector_triggers_after_threshold() {
        let mut detector = StallDetector::new(enabled_config(3));

        assert!(!detector.register_commit("change-a", StallPhase::Apply, true));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 1);
        assert!(!detector.register_commit("change-a", StallPhase::Apply, true));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 2);
        assert!(detector.register_commit("change-a", StallPhase::Apply, true));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 3);
    }

    #[test]
    fn test_stall_detector_resets_on_non_empty_commit() {
        let mut detector = StallDetector::new(enabled_config(2));

        assert!(!detector.register_commit("change-a", StallPhase::Apply, true));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 1);

        assert!(!detector.register_commit("change-a", StallPhase::Apply, false));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 0);
    }

    #[test]
    fn test_stall_detector_disabled_never_triggers() {
        let config = StallDetectionConfig {
            enabled: false,
            threshold: 2,
        };
        let mut detector = StallDetector::new(config);

        assert!(!detector.register_commit("change-a", StallPhase::Apply, true));
        assert_eq!(detector.current_count("change-a", StallPhase::Apply), 0);
    }
}
