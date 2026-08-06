//! One coherent reducer work view for a single scheduler evaluation.
//!
//! Reducer-owned scheduler work used to be read through several independent
//! `try_read` calls: dynamic hint admission, lane-wait synchronization, queue
//! reconciliation, and queue/dependency classification each took their own
//! non-blocking snapshot. Every one of them failed closed on its own, but
//! together they could erase all scheduler-local evidence of still-queued
//! reducer intent, and the scheduler then read that temporary result as a
//! *stable* drained or blocked-only state.
//!
//! [`ReducerWorkSnapshot`] replaces those reads with one awaited acquisition per
//! evaluation. Waiting suspends the evaluation instead of converting contention
//! into queue state, and copying the facts out lets the guard be released before
//! any repository, VCS, analyzer, or dispatch await.
//!
//! The snapshot is process-local scheduler scratch state. It is never persisted
//! and never becomes a workflow-control input.

use std::collections::HashSet;

use crate::orchestration::state::OrchestratorState;

/// Immutable copy of the reducer-owned facts one scheduler evaluation needs.
///
/// Three distinct situations must stay distinguishable, because they authorize
/// very different things:
///
/// - **captured** — a reducer exists and its facts were read coherently;
/// - **absent** ([`Self::absent`]) — no reducer is wired at all, so there is no
///   reducer-owned intent to consult and scheduler-local candidates stand on
///   their own;
/// - **incomplete** ([`Self::incomplete`]) — a reducer exists but acquisition was
///   abandoned (cancellation). Every set is empty, which withholds all ordinary
///   work, and [`Self::is_complete`] additionally forbids drain, termination, and
///   idle decisions so incomplete evidence can never become a lifecycle claim.
#[derive(Debug, Clone)]
pub(super) struct ReducerWorkSnapshot {
    reducer_present: bool,
    acquisition_complete: bool,
    queued_intent_ids: Vec<String>,
    ordinary_eligible_ids: HashSet<String>,
    final_terminal_stop_ids: HashSet<String>,
    active_ids: HashSet<String>,
    resolving_ids: HashSet<String>,
    merge_wait_ids: HashSet<String>,
    resolve_wait_ids: HashSet<String>,
    reject_wait_ids: HashSet<String>,
    acceptance_stalled_ids: HashSet<String>,
    externally_blocked_ids: HashSet<String>,
    /// Terminal-error IDs among the run's initial changes.
    ///
    /// Dependency classification has always scoped terminal-error evidence to
    /// the initial set, so the intersection is captured here rather than
    /// recomputed by each consumer.
    terminal_error_ids: HashSet<String>,
}

impl ReducerWorkSnapshot {
    /// Copy every fact a scheduler evaluation needs out of one reducer read.
    ///
    /// Called with the read guard held; the caller drops the guard immediately
    /// afterwards, so nothing here may await.
    pub(super) fn from_state(state: &OrchestratorState) -> Self {
        let terminal_error_ids = state
            .initial_change_ids()
            .iter()
            .filter(|id| state.is_terminal_error_change(id))
            .cloned()
            .collect();

        Self {
            reducer_present: true,
            acquisition_complete: true,
            queued_intent_ids: state.queued_change_ids(),
            ordinary_eligible_ids: state.ordinary_queue_eligible_change_ids(),
            final_terminal_stop_ids: state.final_terminal_dispatch_stop_change_ids(),
            active_ids: state.active_change_ids().into_iter().collect(),
            resolving_ids: state.resolving_change_ids(),
            merge_wait_ids: state.merge_wait_change_ids().into_iter().collect(),
            resolve_wait_ids: state.resolve_wait_change_ids().into_iter().collect(),
            reject_wait_ids: state.reject_wait_change_ids().into_iter().collect(),
            acceptance_stalled_ids: state.acceptance_stalled_change_ids(),
            externally_blocked_ids: state.externally_blocked_change_ids(),
            terminal_error_ids,
        }
    }

    /// No reducer is wired to this executor.
    pub(super) fn absent() -> Self {
        Self::empty(false, true)
    }

    /// A reducer exists but its facts were not acquired.
    pub(super) fn incomplete() -> Self {
        Self::empty(true, false)
    }

    fn empty(reducer_present: bool, acquisition_complete: bool) -> Self {
        Self {
            reducer_present,
            acquisition_complete,
            queued_intent_ids: Vec::new(),
            ordinary_eligible_ids: HashSet::new(),
            final_terminal_stop_ids: HashSet::new(),
            active_ids: HashSet::new(),
            resolving_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
            resolve_wait_ids: HashSet::new(),
            reject_wait_ids: HashSet::new(),
            acceptance_stalled_ids: HashSet::new(),
            externally_blocked_ids: HashSet::new(),
            terminal_error_ids: HashSet::new(),
        }
    }

    /// Whether reducer facts are actually available in this view.
    ///
    /// Drain, finite termination, and persistent-idle decisions require this.
    /// An absent reducer is complete evidence of "no reducer-owned work"; an
    /// abandoned acquisition is no evidence at all.
    pub(super) fn is_complete(&self) -> bool {
        self.acquisition_complete
    }

    /// Whether a reducer owns queue intent for this executor at all.
    pub(super) fn reducer_present(&self) -> bool {
        self.reducer_present
    }

    pub(super) fn queued_intent_ids(&self) -> &[String] {
        &self.queued_intent_ids
    }

    pub(super) fn active_ids(&self) -> &HashSet<String> {
        &self.active_ids
    }

    pub(super) fn resolving_ids(&self) -> &HashSet<String> {
        &self.resolving_ids
    }

    pub(super) fn merge_wait_ids(&self) -> &HashSet<String> {
        &self.merge_wait_ids
    }

    pub(super) fn resolve_wait_ids(&self) -> &HashSet<String> {
        &self.resolve_wait_ids
    }

    pub(super) fn reject_wait_ids(&self) -> &HashSet<String> {
        &self.reject_wait_ids
    }

    pub(super) fn terminal_error_ids(&self) -> &HashSet<String> {
        &self.terminal_error_ids
    }

    /// Reducer-owned holds that suppress ordinary dispatch, whichever phase
    /// observed them. An apply-origin external blocker holds exactly like an
    /// acceptance-origin one; only the operator-facing explanation differs.
    pub(super) fn held_ids(&self) -> HashSet<String> {
        self.acceptance_stalled_ids
            .iter()
            .chain(&self.externally_blocked_ids)
            .cloned()
            .collect()
    }

    /// Set form of `OrchestratorState::is_ordinary_queue_eligible`.
    pub(super) fn is_ordinary_queue_eligible(&self, change_id: &str) -> bool {
        self.ordinary_eligible_ids.contains(change_id)
    }

    /// Set form of `OrchestratorState::is_final_terminal_dispatch_stop`.
    pub(super) fn is_final_terminal_dispatch_stop(&self, change_id: &str) -> bool {
        self.final_terminal_stop_ids.contains(change_id)
    }
}
