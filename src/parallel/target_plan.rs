//! Deferred explicit-target classification inside the parallel scheduler.
//!
//! An enabled real `-u` run cannot classify its explicit targets before startup:
//! the mandatory initial upstream base-lane checkpoint may integrate the very
//! archive that proves a requested target is already completed. This module owns
//! the post-checkpoint boundary where that classification happens - after the
//! checkpoint, before any change-worktree creation or reuse registration.

use crate::error::Result;
use crate::openspec::Change;
use crate::orchestration::target_resolution::TargetClassification;
use crate::parallel_run_service::filter_committed_changes_at;

use crate::events::LogEntry;

use super::events::send_event;
use super::{ParallelEvent, ParallelExecutor};

impl ParallelExecutor {
    /// Resolve a deferred explicit-target plan, if one is installed.
    ///
    /// Returns the changes that should enter scheduling. Already-completed
    /// targets are excluded as successful skips; unknown, duplicate,
    /// contradictory, unreadable, and `--no-resume`-refused targets are reported
    /// together as one error before any workspace is touched.
    pub(super) async fn apply_explicit_target_plan(
        &self,
        changes: Vec<Change>,
    ) -> Result<Vec<Change>> {
        let Some(plan) = self.explicit_target_plan.clone() else {
            return Ok(changes);
        };

        let resolution = plan.resolve(&self.repo_root).await;

        for line in resolution.report_lines() {
            tracing::info!("{}", line);
            send_event(&self.event_tx, ParallelEvent::Log(LogEntry::info(&line))).await;
        }

        if let Some(err) = resolution.failure_error() {
            return Err(err);
        }

        // Active targets keep the ordinary start-time eligibility rule. Resumable
        // targets are proven by their own workspace, not by the base HEAD tree,
        // so they are not subject to it.
        let mut active = Vec::new();
        let mut resumable = Vec::new();
        for target in &resolution.targets {
            let Some(change) = target.change.clone() else {
                continue;
            };
            match target.classification {
                TargetClassification::Active => active.push(change),
                TargetClassification::ResumableWorkspace => resumable.push(change),
                _ => {}
            }
        }

        let (mut committed, skipped) = filter_committed_changes_at(&self.repo_root, active).await?;

        if !skipped.is_empty() {
            let message = format!(
                "Skipping uncommitted changes in parallel mode: {}",
                skipped.join(", ")
            );
            tracing::warn!("{}", message);
            send_event(
                &self.event_tx,
                ParallelEvent::Warning {
                    title: "Uncommitted changes skipped".to_string(),
                    message,
                },
            )
            .await;
            send_event(
                &self.event_tx,
                ParallelEvent::ParallelStartRejected {
                    change_ids: skipped.clone(),
                    reason: "uncommitted or not in HEAD".to_string(),
                },
            )
            .await;
        }

        let skipped_set: std::collections::HashSet<&String> = skipped.iter().collect();
        committed.extend(resumable);

        // Restore deduplicated request order, which terminal reporting and
        // dispatch both rely on.
        let mut ordered = Vec::with_capacity(committed.len());
        for requested_id in resolution.processed_ids() {
            if skipped_set.contains(&requested_id) {
                continue;
            }
            if let Some(pos) = committed.iter().position(|c| c.id == requested_id) {
                ordered.push(committed.remove(pos));
            }
        }

        if let Some(shared_state) = &self.shared_orchestrator_state {
            let mut guard = shared_state.write().await;
            for change in &ordered {
                guard.add_dynamic_change(change.id.clone());
            }
        }

        Ok(ordered)
    }
}
