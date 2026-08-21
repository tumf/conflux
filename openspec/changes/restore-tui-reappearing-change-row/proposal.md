---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state/processing_logic.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/web/operator_facts.rs
  - openspec/specs/tui-architecture/spec.md
verifications:
  - id: refresh-reappearance-test
    requirement: A change row removed during a transient refresh absence reappears when repository discovery observes it again
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state/event_handlers/refresh.rs
    evidence: Passing focused Rust regression test covering present, absent, and present-again refresh snapshots
    rerun: cargo test changes_refreshed_restores_change_after_transient_absence -- --exact
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Restore TUI row after transient change absence

**Change Type**: implementation

## Problem/Context

The local TUI periodically projects active OpenSpec changes into `AppState::changes`. A change can be temporarily absent from one filesystem snapshot while a proposal worktree is refreshed or merged into the base branch.

The refresh path removes rows that are absent from the latest snapshot, but `known_change_ids` retains their IDs. If the same proposal becomes observable again, it is classified as already known and the missing row is not reconstructed. The proposal remains available to repository and orchestration state while the TUI row stays absent.

`src/web/operator_facts.rs` already reconciles its known-ID set with each current observation. The TUI projection needs equivalent removal symmetry without changing scheduler, queue, acceptance, archive, or resume behavior.

## Proposed Solution

- Reconcile TUI change identity bookkeeping with each successful current observation so an ID removed from the visible projection cannot remain as a stale suppression entry.
- Restore a row from current repository evidence when a previously observed change reappears.
- Treat the reappearing active proposal as newly detected for TUI observability while preserving authoritative execution intent and reducer-derived state.
- Keep cursor selection stable and do not automatically mark, queue, dispatch, or otherwise control the reappearing change.
- Add a transition regression test for `present → temporarily absent → present`.

## Acceptance Criteria

- A proposal visible in one refresh, absent in the next, and visible again in a later successful refresh is present in the final TUI changes list.
- The restored row is reconstructed from the current refresh data rather than stale cached row contents.
- A reappearing active proposal receives the existing `is_new`/new-change observability treatment and one detection log for that reappearance.
- Refresh does not move the cursor solely because the row reappears.
- Refresh does not select, mark, queue, dispatch, resume, accept, archive, or otherwise alter workflow intent for the reappearing proposal.
- Rejected-row behavior remains read-only and does not increment the active new-change counter.

## Explicit Completion Conditions

- `src/tui/state/processing_logic.rs` no longer allows `known_change_ids` to retain an ID after that ID is absent from the current successful active and rejected observations.
- The refresh handler has a focused test that applies present, absent, and present-again snapshots to one `AppState` and fails if the final row is missing.
- The test also verifies cursor stability, unselected state, new-change count/log behavior, and rejected-row invariants where applicable.
- `cargo test changes_refreshed_restores_change_after_transient_absence -- --exact` passes.
- Strict OpenSpec validation passes.

## Out of Scope

- Changing OpenSpec filesystem discovery or polling frequency.
- Persisting TUI discovery state outside the process.
- Changing reducer, scheduler, queue, mark, acceptance, archive, or resume semantics.
- Automatically selecting or executing a restored proposal.
- Reworking remote TUI transport.
