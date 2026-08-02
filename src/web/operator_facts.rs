//! Process-local operator facts that no single reducer field carries.
//!
//! The reducer owns lifecycle, and the execution-mark store owns operator
//! intent. Three things belong to neither: how long the current run has been
//! going, what the last lifecycle-significant thing to happen was, and which
//! changes appeared after the process started watching. This store owns exactly
//! those, plus the two server-side observations (parallel eligibility, worktree
//! relation) that a client would otherwise have to run Git to reproduce.
//!
//! Everything here is in-memory for one process lifetime and is discarded on
//! restart, so none of it can become durable workflow-control state.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::events::ExecutionEvent;
use crate::tui::types::WorktreeInfo;
use crate::web::remote_control_api::dto::{
    AttentionState, ChangeActivity, ChangeTiming, ChangeWorktree, ParallelBlockedReason,
    ParallelEligibility,
};
use crate::web::remote_control_api::projection::describe_event;
use crate::web::remote_control_api::worktrees::repository_relative_display;

/// Wire event types that are *not* published as latest activity.
///
/// Streaming output and per-tick progress would make the snapshot differ on
/// every chunk and advance `state_revision` continuously, which would leave
/// every client's optimistic-concurrency token permanently stale. Logs are
/// excluded for the same reason plus a stronger one: a log input must never
/// advance the revision at all.
const NON_ACTIVITY_EVENT_TYPES: [&str; 6] = [
    "log",
    "apply_output",
    "archive_output",
    "acceptance_output",
    "resolve_output",
    "progress_updated",
];

/// Wire event types that begin a timed run for a change.
const RUN_START_EVENT_TYPES: [&str; 5] = [
    "apply_started",
    "acceptance_started",
    "archive_started",
    "resolve_started",
    "push_started",
];

/// Wire event types that end a timed run for a change.
const RUN_END_EVENT_TYPES: [&str; 11] = [
    "processing_completed",
    "processing_error",
    "apply_failed",
    "acceptance_failed",
    "archive_failed",
    "change_archived",
    "change_rejected",
    "resolve_completed",
    "resolve_failed",
    "merge_completed",
    "push_completed",
];

/// The per-change facts this store owns.
#[derive(Debug, Clone, Default)]
pub struct ChangeOperatorFacts {
    /// Whether the change was first observed after the process started watching.
    pub newly_detected: bool,
    /// Server-observed parallel-execution eligibility.
    pub parallel: ParallelEligibility,
    /// Managed worktree relation, when one exists.
    pub worktree: Option<ChangeWorktree>,
    /// Published run boundaries.
    pub timing: ChangeTiming,
    /// Latest lifecycle-significant activity.
    pub latest_activity: Option<ChangeActivity>,
    /// Start instant backing `timing.elapsed_ms`. Never serialized.
    started_at: Option<DateTime<Utc>>,
}

impl ChangeOperatorFacts {
    /// Attention state for this change.
    ///
    /// `New` survives only until the change is acted on or starts moving: an
    /// execution mark, a queue intent, or any observed activity all mean the
    /// operator no longer needs to be told the change appeared.
    pub fn attention(&self, execution_marked: bool, queued: bool) -> AttentionState {
        if self.newly_detected
            && !execution_marked
            && !queued
            && self.latest_activity.is_none()
            && self.timing.started_at.is_none()
        {
            AttentionState::New
        } else {
            AttentionState::None
        }
    }
}

/// Process-local operator facts for every observed change.
#[derive(Debug, Default)]
pub struct OperatorFactsStore {
    facts: HashMap<String, ChangeOperatorFacts>,
    /// Change IDs this incarnation has already seen at least once.
    known_change_ids: HashSet<String>,
    /// False until the first observation, whose contents are the baseline
    /// rather than a set of new arrivals.
    bootstrapped: bool,
    /// Sanitized fatal process error, when one was reported.
    process_error: Option<String>,
    /// Repository root used to redact worktree paths.
    repo_root: Option<PathBuf>,
    /// Latest change-to-worktree-path observation.
    worktree_paths: HashMap<String, PathBuf>,
    /// Latest worktree observation, used to resolve branches.
    worktrees: Vec<WorktreeInfo>,
}

impl OperatorFactsStore {
    /// An empty store. A restarted process always begins here.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind the repository root used to redact worktree paths.
    pub fn set_repo_root(&mut self, repo_root: PathBuf) {
        self.repo_root = Some(repo_root);
        self.reproject_worktrees();
    }

    /// Facts for one change, or the empty defaults.
    pub fn facts(&self, change_id: &str) -> ChangeOperatorFacts {
        self.facts.get(change_id).cloned().unwrap_or_default()
    }

    /// Sanitized fatal process error, when one was reported.
    pub fn process_error(&self) -> Option<String> {
        self.process_error.clone()
    }

    /// Reconcile the store with the complete set of currently known changes.
    ///
    /// The first observation is the baseline: nothing in it is "new", because a
    /// process that just started has not *detected* anything, it has merely
    /// looked. Later arrivals are new. Changes that disappear lose their facts,
    /// so a re-created change is genuinely new again.
    pub fn observe_changes<'a>(&mut self, change_ids: impl IntoIterator<Item = &'a str>) {
        let observed: HashSet<String> = change_ids.into_iter().map(str::to_string).collect();

        for id in &observed {
            if self.known_change_ids.insert(id.clone()) && self.bootstrapped {
                self.facts.entry(id.clone()).or_default().newly_detected = true;
            }
        }

        self.known_change_ids.retain(|id| observed.contains(id));
        self.facts.retain(|id, _| observed.contains(id));
        self.bootstrapped = true;
        // A change observed for the first time still needs its facts entry, so
        // its worktree relation and eligibility land on it rather than on
        // nothing.
        for id in observed {
            self.facts.entry(id).or_default();
        }
        self.reproject_worktrees();
    }

    /// Record the parallel-execution observation from a workspace refresh.
    pub fn apply_parallel_eligibility(
        &mut self,
        committed_change_ids: &HashSet<String>,
        uncommitted_file_change_ids: &HashSet<String>,
    ) {
        for (id, facts) in &mut self.facts {
            facts.parallel = if !committed_change_ids.contains(id) {
                ParallelEligibility {
                    eligible: false,
                    blocked_reason: Some(ParallelBlockedReason::NotCommitted),
                }
            } else if uncommitted_file_change_ids.contains(id) {
                ParallelEligibility {
                    eligible: false,
                    blocked_reason: Some(ParallelBlockedReason::UncommittedChanges),
                }
            } else {
                ParallelEligibility::default()
            };
        }
    }

    /// Record the change-to-worktree-path relation from a workspace refresh.
    pub fn apply_worktree_paths(&mut self, worktree_paths: HashMap<String, PathBuf>) {
        self.worktree_paths = worktree_paths;
        self.reproject_worktrees();
    }

    /// Record the latest worktree observation, which supplies branches.
    pub fn apply_worktrees(&mut self, worktrees: Vec<WorktreeInfo>) {
        self.worktrees = worktrees;
        self.reproject_worktrees();
    }

    /// Rebuild every change's worktree relation from the latest observations.
    ///
    /// The branch comes from the worktree observation rather than from the
    /// directory name, so a detached or renamed worktree reports what it really
    /// is instead of what its path suggests.
    fn reproject_worktrees(&mut self) {
        let Self {
            facts,
            repo_root,
            worktree_paths,
            worktrees,
            ..
        } = self;
        for (id, entry) in facts.iter_mut() {
            entry.worktree = worktree_paths
                .get(id)
                .map(|path| Self::project_worktree(repo_root.as_deref(), path, worktrees));
        }
    }

    fn project_worktree(
        repo_root: Option<&Path>,
        path: &Path,
        worktrees: &[WorktreeInfo],
    ) -> ChangeWorktree {
        let observed = worktrees.iter().find(|worktree| worktree.path == path);
        ChangeWorktree {
            path: match repo_root {
                Some(root) => repository_relative_display(root, path),
                // Without a bound root the only safe projection is the leaf
                // name: an absolute path is exactly what must not be published.
                None => path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            },
            branch: observed
                .map(|worktree| worktree.branch.clone())
                .filter(|branch| !branch.is_empty()),
        }
    }

    /// Absorb one execution event into timing, activity, and process error.
    ///
    /// Uses the same [`describe_event`] projection the event stream publishes,
    /// so `latest_activity.event_type` and the stream's `event_type` are the
    /// same vocabulary rather than two parallel taxonomies.
    pub fn record_event(&mut self, event: &ExecutionEvent) {
        let (event_type, change_id, _) = describe_event(event);
        let detail = match event {
            ExecutionEvent::ProcessingError { error, .. }
            | ExecutionEvent::ApplyFailed { error, .. }
            | ExecutionEvent::AcceptanceFailed { error, .. }
            | ExecutionEvent::ArchiveFailed { error, .. }
            | ExecutionEvent::ResolveFailed { error, .. }
            | ExecutionEvent::PushFailed { error, .. }
            | ExecutionEvent::RejectionReviewFailed { error, .. } => Some(error.clone()),
            ExecutionEvent::ChangeRejected { reason, .. } => Some(reason.clone()),
            ExecutionEvent::MergeDeferred { reason, .. } => Some(reason.clone()),
            _ => None,
        };

        match event {
            // A fatal process error is process-scoped; a start clears it so a
            // recovered run does not keep reporting a dead one.
            ExecutionEvent::Error { message } => {
                self.process_error = Some(crate::events::sanitize_detail(message));
                return;
            }
            ExecutionEvent::ProcessingStarted(_) => self.process_error = None,
            _ => {}
        }

        let Some(change_id) = change_id else {
            return;
        };
        let now = Utc::now();
        let facts = self.facts.entry(change_id).or_default();

        // `processing_started` begins a fresh run and discards the previous
        // one's boundaries; every other start only fills a gap, so a change that
        // moves apply -> acceptance -> archive keeps one continuous elapsed time.
        if event_type == "processing_started" {
            facts.started_at = Some(now);
            facts.timing = ChangeTiming {
                started_at: Some(now.to_rfc3339()),
                completed_at: None,
                elapsed_ms: None,
            };
        } else if RUN_START_EVENT_TYPES.contains(&event_type) && facts.started_at.is_none() {
            facts.started_at = Some(now);
            facts.timing.started_at = Some(now.to_rfc3339());
            facts.timing.completed_at = None;
            facts.timing.elapsed_ms = None;
        }

        if RUN_END_EVENT_TYPES.contains(&event_type) {
            if let Some(started) = facts.started_at.take() {
                facts.timing.completed_at = Some(now.to_rfc3339());
                facts.timing.elapsed_ms =
                    Some((now - started).num_milliseconds().max(0).unsigned_abs());
            }
        }

        if !NON_ACTIVITY_EVENT_TYPES.contains(&event_type) {
            facts.latest_activity = Some(ChangeActivity {
                event_type: event_type.to_string(),
                timestamp: now.to_rfc3339(),
                detail: detail.as_deref().map(crate::events::sanitize_detail),
            });
        }
    }
}
