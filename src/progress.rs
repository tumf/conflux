use crate::openspec::Change;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// Manages progress display using indicatif
pub struct ProgressDisplay {
    multi: MultiProgress,
    overall: ProgressBar,
    current: Option<ProgressBar>,
}

impl ProgressDisplay {
    /// Create a new progress display
    pub fn new(total_changes: usize) -> Self {
        let multi = MultiProgress::new();

        let overall = multi.add(ProgressBar::new(total_changes as u64));
        overall.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        overall.set_message("Overall progress");

        Self {
            multi,
            overall,
            current: None,
        }
    }

    /// Update current change progress
    pub fn update_change(&mut self, change: &Change) {
        // Remove old progress bar if exists
        if let Some(pb) = self.current.take() {
            pb.finish_and_clear();
        }

        // Create new progress bar for current change
        let pb = self.multi.add(ProgressBar::new(change.total_tasks as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:40.green/yellow}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_position(change.completed_tasks as u64);
        pb.set_message(format!("{} ({:.1}%)", change.id, change.progress_percent()));

        self.current = Some(pb);
    }

    /// Mark change as completed
    pub fn complete_change(&mut self, change_id: &str) {
        if let Some(pb) = self.current.take() {
            pb.finish_with_message(format!("✓ {} completed", change_id));
        }
        self.overall.inc(1);
    }

    /// Mark change as archived
    pub fn archive_change(&mut self, change_id: &str) {
        if let Some(pb) = self.current.take() {
            pb.finish_with_message(format!("📦 {} archived", change_id));
        }
        self.overall.inc(1);
    }

    /// Display error
    pub fn error(&mut self, message: &str) {
        if let Some(pb) = self.current.take() {
            pb.finish_with_message(format!("✗ {}", message));
        }
    }

    /// Complete all progress bars
    pub fn complete_all(&mut self) {
        if let Some(pb) = self.current.take() {
            pb.finish_and_clear();
        }
        self.overall.finish_with_message("✓ All changes processed");
    }

    /// Set overall message
    #[allow(dead_code)]
    pub fn set_message(&self, message: &str) {
        self.overall.set_message(message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_display_creation() {
        let _display = ProgressDisplay::new(5);
        // Just test that it doesn't panic
    }

    #[test]
    fn test_progress_update() {
        let mut display = ProgressDisplay::new(3);
        let change = Change {
            id: "test-change".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            last_modified: "now".to_string(),
        };
        display.update_change(&change);
        // Just test that it doesn't panic
    }
}
