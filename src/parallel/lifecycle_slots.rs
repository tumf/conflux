//! Lifecycle-owned concurrency slots.
//!
//! `max_concurrent_workspaces` bounds *admitted changes* and their managed
//! worktrees, not individual async tasks. A change acquires one slot
//! immediately before workspace preparation and keeps that same slot — the same
//! owned semaphore permit — through apply, acceptance, archive, background
//! merge, automatic resolve, and any retained merge/resolve/reject wait, until
//! repository-visible evidence settles its admitted lifecycle.
//!
//! The permit is therefore *transferred* across task boundaries rather than
//! dropped and reacquired. Dropping it when the workspace task returned is what
//! let a queued change take the apparent free slot while the previous change was
//! still merging or resolving, so a configured limit of three could present
//! three applying changes plus one resolving change.
//!
//! # One membership, no double counting
//!
//! Occupancy is keyed by change ID, so a change that is simultaneously visible
//! to a phase counter (auto resolve, manual resolve) and to lifecycle membership
//! is counted once. Acquiring a second permit for an already-occupying change is
//! impossible by construction: [`LifecycleSlots::occupy`] and
//! [`LifecycleSlots::reserve_retained`] both refuse to create a second entry.
//!
//! # Ephemeral by constitutional law
//!
//! This is in-memory state held for one process lifetime. Nothing is persisted,
//! and restart recovery re-derives occupancy from workspace, Git, and reducer
//! evidence through [`LifecycleSlots::reserve_retained`] rather than from any
//! durable slot lease (`openspec/CONSTITUTION.md`, law 1).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Where an occupied slot currently is in its change's lifecycle.
///
/// The phase decides which owner may release the slot, which is what keeps two
/// owners from releasing the same lifecycle twice — or neither releasing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotPhase {
    /// Workspace preparation, apply, acceptance, archive.
    ///
    /// Released by the workspace-task completion handler alone.
    Workspace,
    /// Background merge, including automatic conflict resolution inside it.
    ///
    /// Released by the background result handler alone; the merge task always
    /// reports exactly one result, including on the cancellation drain.
    Merge,
    /// Held for an operator-owned or scheduler-owned wait (`merge wait`,
    /// `resolve pending`, `reject pending`).
    ///
    /// This is the one phase reducer reconciliation may release, because no task
    /// owns the change while it waits.
    Retained,
}

struct OwnedSlot {
    phase: SlotPhase,
    /// Whether the scheduler has actually observed reducer-visible wait evidence
    /// for this change.
    ///
    /// A background result can reach the scheduler before the reducer applies
    /// the `MergeDeferred` event that explains it, so "absent from the wait
    /// sets" means nothing until the wait has been seen at least once. Without
    /// this, reconciliation could release a slot for a change that is about to
    /// appear in `merge wait`.
    retention_confirmed: bool,
    /// The transferred permit. Held for the whole admitted lifecycle; dropping
    /// it is what returns capacity.
    _permit: OwnedSemaphorePermit,
}

/// Outcome of reserving a slot for reducer-visible retained work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotReservation {
    /// The change already owns exactly one lifecycle slot.
    AlreadyOwned,
    /// A slot was acquired for it now (restart recovery, or a wait this process
    /// never dispatched).
    Reserved,
    /// No permit is available. The caller must fail closed for new dispatch.
    Unavailable,
}

/// What a returning owner does to the change's lifecycle slot.
///
/// The decision is deliberately separated from the work it accompanies —
/// spawning a merge, cleaning a rejected workspace, promoting a base-lane
/// waiter — so "does this boundary end the admitted lifecycle?" is one
/// verifiable answer rather than a condition spelled out at each call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotTransition {
    /// The lifecycle continues under a new owner; the same permit moves with it.
    Transfer(SlotPhase),
    /// The admitted lifecycle ended; return the capacity.
    Release,
    /// Leave the slot exactly as it is. Used where a different exit — the run's
    /// own abort path — owns every slot this run holds.
    Keep,
}

/// What the returning apply/acceptance/archive task does to its slot.
///
/// A successful archive with a revision to integrate is the *transfer*: the
/// change is still admitted, and background merge handling becomes the owner.
/// Every other return ends the admitted lifecycle here — a terminal error, a
/// rejection, or a completion with nothing left to integrate.
pub(super) fn workspace_completion_transition(
    errored: bool,
    rejected: bool,
    has_final_revision: bool,
) -> SlotTransition {
    if !errored && !rejected && has_final_revision {
        SlotTransition::Transfer(SlotPhase::Merge)
    } else {
        SlotTransition::Release
    }
}

/// What a settled background base-lane result does to its slot.
///
/// Only a completed merge releases. Every other change-local outcome leaves the
/// change admitted — deferred for scheduler-owned retry, exhausted into
/// `merge wait`, or held for an operator — and keeps its slot as admission
/// backpressure, because the alternative is admitting a newer worktree on top of
/// unsettled work built from an older baseline.
///
/// A run-fatal outcome keeps the slot untouched: the loop is already aborting
/// and its exit clears every slot the run owns.
pub(super) fn merge_result_transition(merged: bool, run_fatal: bool) -> SlotTransition {
    match (merged, run_fatal) {
        (true, _) => SlotTransition::Release,
        (false, true) => SlotTransition::Keep,
        (false, false) => SlotTransition::Transfer(SlotPhase::Retained),
    }
}

/// What one reconciliation pass did to retained occupancy.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct RetainedReconciliation {
    /// Waits that had no slot and took one now.
    pub(super) reserved: Vec<String>,
    /// Waits that could not take one. Non-empty means fail closed.
    pub(super) unavailable: Vec<String>,
    /// Retained slots whose change settled.
    pub(super) released: Vec<String>,
}

/// The authoritative lifecycle-slot membership for one scheduler.
pub(super) struct LifecycleSlots {
    capacity: usize,
    semaphore: Arc<Semaphore>,
    slots: HashMap<String, OwnedSlot>,
    /// Read view of `slots.keys()`, kept in step by the two mutating methods so
    /// callers that already take `&HashSet<String>` need no allocation.
    members: HashSet<String>,
    /// Retained changes that could not be reserved a permit.
    ///
    /// Ambiguous or over-subscribed occupancy fails closed: while this is
    /// non-empty, no new dispatch capacity is reported.
    unreserved: HashSet<String>,
}

impl LifecycleSlots {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            semaphore: Arc::new(Semaphore::new(capacity)),
            slots: HashMap::new(),
            members: HashSet::new(),
            unreserved: HashSet::new(),
        }
    }

    /// Raise the slot ceiling to `max_parallelism` when the scheduler runs with
    /// a larger limit than the one this executor was constructed with.
    ///
    /// Growing only. Reducing capacity at runtime never cancels already admitted
    /// work, so an admitted change keeps the permit it owns and the ceiling
    /// converges as those changes settle.
    pub(super) fn ensure_capacity(&mut self, max_parallelism: usize) {
        if max_parallelism > self.capacity {
            self.semaphore.add_permits(max_parallelism - self.capacity);
            self.capacity = max_parallelism;
        }
    }

    /// The single permit source. There is deliberately no second semaphore for
    /// merge or resolve: independent permits recreate the transfer race.
    pub(super) fn semaphore(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    pub(super) fn members(&self) -> &HashSet<String> {
        &self.members
    }

    pub(super) fn len(&self) -> usize {
        self.slots.len()
    }

    pub(super) fn contains(&self, change_id: &str) -> bool {
        self.slots.contains_key(change_id)
    }

    /// Unique lifecycle occupancy, counting `also_occupied` change IDs that are
    /// not represented here.
    ///
    /// The union is what makes the count *unique*: a change tracked by both the
    /// caller's in-flight set and this membership is one admitted lifecycle, not
    /// two.
    pub(super) fn occupancy_with(&self, also_occupied: &HashSet<String>) -> usize {
        self.slots.len()
            + also_occupied
                .iter()
                .filter(|change_id| !self.slots.contains_key(*change_id))
                .count()
    }

    /// Whether some retained change is occupying no reserved permit.
    pub(super) fn has_unreserved(&self) -> bool {
        !self.unreserved.is_empty()
    }

    /// Dispatch capacity under `max_parallelism`, counting `also_occupied` once.
    ///
    /// Unprovable retained occupancy fails closed at zero: admitting against
    /// occupancy the scheduler could not reserve is the overrun this accounting
    /// exists to prevent.
    pub(super) fn available(
        &self,
        max_parallelism: usize,
        also_occupied: &HashSet<String>,
    ) -> usize {
        if self.has_unreserved() {
            return 0;
        }
        max_parallelism.saturating_sub(self.occupancy_with(also_occupied))
    }

    /// Apply one lifecycle transition to `change_id`'s slot.
    ///
    /// Returns whether the slot was released, which is the caller's "capacity
    /// came back" edge.
    pub(super) fn apply_transition(&mut self, change_id: &str, transition: SlotTransition) -> bool {
        match transition {
            SlotTransition::Transfer(phase) => {
                self.transfer(change_id, phase);
                false
            }
            SlotTransition::Release => self.release(change_id),
            SlotTransition::Keep => false,
        }
    }

    /// Record an acquired permit as `change_id`'s one lifecycle slot.
    ///
    /// Returns `false` — dropping `permit`, which returns it to the semaphore —
    /// when the change already owns a slot, so no change can ever hold two.
    pub(super) fn occupy(
        &mut self,
        change_id: &str,
        permit: OwnedSemaphorePermit,
        phase: SlotPhase,
    ) -> bool {
        if self.slots.contains_key(change_id) {
            return false;
        }
        // Never pre-confirmed, whatever the phase: confirmation means *this*
        // scheduler observed reducer wait evidence for the change, and only
        // `reserve_retained` can have done that.
        self.slots.insert(
            change_id.to_string(),
            OwnedSlot {
                phase,
                retention_confirmed: false,
                _permit: permit,
            },
        );
        self.members.insert(change_id.to_string());
        self.unreserved.remove(change_id);
        true
    }

    /// Move an owned slot to a later lifecycle phase, keeping the same permit.
    ///
    /// This is the transfer: no interval exists in which the change owns zero or
    /// two slots. Returns `false` when the change owns no slot.
    pub(super) fn transfer(&mut self, change_id: &str, phase: SlotPhase) -> bool {
        match self.slots.get_mut(change_id) {
            Some(slot) => {
                slot.phase = phase;
                true
            }
            None => false,
        }
    }

    /// Reserve a slot for a change the reducer reports as waiting.
    ///
    /// This is the restart-recovery and late-evidence path: a retained
    /// `merge wait` survives the process that dispatched it only as repository
    /// and reducer evidence, so the scheduler re-establishes its occupancy here
    /// instead of reading a durable lease.
    pub(super) fn reserve_retained(&mut self, change_id: &str) -> SlotReservation {
        if let Some(slot) = self.slots.get_mut(change_id) {
            slot.retention_confirmed = true;
            return SlotReservation::AlreadyOwned;
        }
        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.occupy(change_id, permit, SlotPhase::Retained);
                if let Some(slot) = self.slots.get_mut(change_id) {
                    // Reserved *from* wait evidence, so the wait is confirmed by
                    // construction: a later pass that no longer sees it is
                    // observing a real settlement, not a missing publication.
                    slot.retention_confirmed = true;
                }
                SlotReservation::Reserved
            }
            Err(_) => {
                self.unreserved.insert(change_id.to_string());
                SlotReservation::Unavailable
            }
        }
    }

    /// Release `change_id`'s lifecycle slot exactly once.
    ///
    /// Returns whether this call was the release. Duplicate completion delivery
    /// therefore cannot create or return a second slot.
    pub(super) fn release(&mut self, change_id: &str) -> bool {
        self.members.remove(change_id);
        self.unreserved.remove(change_id);
        self.slots.remove(change_id).is_some()
    }

    /// Reconstruct retained occupancy from evidence, and settle what ended.
    ///
    /// `retained` is the reducer's base-lane wait evidence, `active` its
    /// executing rows, and `settled` positive terminal evidence. Reservation
    /// runs first so a wait that just appeared holds capacity before the same
    /// pass computes it.
    pub(super) fn reconcile_retained(
        &mut self,
        retained: &HashSet<String>,
        active: &HashSet<String>,
        settled: &HashSet<String>,
    ) -> RetainedReconciliation {
        let mut report = RetainedReconciliation::default();
        for change_id in retained {
            match self.reserve_retained(change_id) {
                SlotReservation::Reserved => report.reserved.push(change_id.clone()),
                SlotReservation::Unavailable => report.unavailable.push(change_id.clone()),
                SlotReservation::AlreadyOwned => {}
            }
        }
        for change_id in self.releasable_retained(retained, active, settled) {
            if self.release(&change_id) {
                report.released.push(change_id);
            }
        }
        report.reserved.sort();
        report.unavailable.sort();
        report.released.sort();
        report
    }

    /// Retained slots whose admitted lifecycle reducer evidence has ended.
    ///
    /// Two independent proofs, and only for [`SlotPhase::Retained`] — a
    /// workspace task and a background merge each own their own release edge:
    ///
    /// - `settled` is positive terminal evidence (merged, pushed, rejected,
    ///   error, stopped), which needs no corroboration;
    /// - a wait this scheduler confirmed, and which has since left both the wait
    ///   evidence and active execution, is an explicit dequeue or an equivalent
    ///   not-queued settlement.
    pub(super) fn releasable_retained(
        &self,
        retained: &HashSet<String>,
        active: &HashSet<String>,
        settled: &HashSet<String>,
    ) -> Vec<String> {
        self.slots
            .iter()
            .filter(|(change_id, slot)| {
                if !matches!(slot.phase, SlotPhase::Retained) {
                    return false;
                }
                if settled.contains(*change_id) {
                    return true;
                }
                slot.retention_confirmed
                    && !retained.contains(*change_id)
                    && !active.contains(*change_id)
            })
            .map(|(change_id, _)| change_id.clone())
            .collect()
    }

    /// Drop every slot this run owned.
    ///
    /// Used by the cancellation and run-fatal exits, after their bounded drain
    /// has already handled the background results those slots were waiting for.
    pub(super) fn clear(&mut self) {
        self.slots.clear();
        self.members.clear();
        self.unreserved.clear();
    }

    #[cfg(test)]
    pub(super) fn phase(&self, change_id: &str) -> Option<SlotPhase> {
        self.slots.get(change_id).map(|slot| slot.phase)
    }

    /// Occupy a slot for `change_id`, waiting for capacity if necessary.
    ///
    /// Test-only convenience for building an occupancy state the production
    /// edges would have built; production dispatch acquires its permit through
    /// [`Self::semaphore`] so it can race the run's cancellation token.
    #[cfg(test)]
    pub(super) async fn occupy_now(&mut self, change_id: &str, phase: SlotPhase) -> bool {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("lifecycle slot semaphore is never closed");
        self.occupy(change_id, permit, phase)
    }
}
