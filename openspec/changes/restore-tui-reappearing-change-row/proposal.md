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
    rerun: cargo test tui::state::event_handlers::refresh::tests::changes_refreshed_restores_change_after_transient_absence -- --exact
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Restore TUI row after transient change absence

**Change Type**: implementation

## Problem/Context

The local TUI periodically projects active OpenSpec changes into `AppState::changes`. A change can be temporarily absent from one filesystem snapshot while a proposal worktree is refreshed or merged into the base branch.

The refresh path removes rows that are absent from the latest snapshot, but `known_change_ids` retains their IDs. If the same proposal becomes observable again, it is classified as already known and the missing row is not reconstructed. The proposal remains available to repository and orchestration state while the TUI row stays absent.

`src/web/operator_facts.rs` already reconciles its known-ID set with each current observation. The TUI needs the same "no stale suppression" property, but its convergence target is different: the TUI row retention rule deliberately keeps rows that are absent from the current snapshot (rows with a recorded start, and rows in terminal or wait statuses such as `merge wait`, `archived`, `merged`, `rejected`, `error`). Identity bookkeeping must therefore converge to the retained row projection — an ID must not outlive its row, and an ID whose row is still retained must stay known — without changing scheduler, queue, acceptance, archive, or resume behavior.

## Proposed Solution

- Reconcile TUI change identity bookkeeping with the row projection that survives each successful refresh, so an ID whose row was removed cannot remain as a stale suppression entry, while an ID whose row is deliberately retained (recorded start, or terminal/wait display status) stays known and is never re-created as a duplicate.
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
- A row retained through an absence (recorded start, or terminal/wait display status) that is observed again is updated in place: no duplicate row is added, no NEW badge is applied, and no detection log is emitted for it.
- Rejected-row behavior remains read-only and does not increment the active new-change counter.

## Explicit Completion Conditions

- `src/tui/state/processing_logic.rs` no longer allows `known_change_ids` to retain an ID after that ID's row has been removed from the projection; IDs whose rows are retained despite being absent from the current observations stay known, so `known_change_ids` converges to exactly the IDs of rows present after the refresh settles.
- The refresh handler has a focused test that applies present, absent, and present-again snapshots to one `AppState` and fails if the final row is missing.
- The test also verifies cursor stability, unselected state, new-change count/log behavior, rejected-row invariants where applicable, and that a row retained through an absence is not duplicated, re-badged NEW, or re-logged when observed again.
- `cargo test tui::state::event_handlers::refresh::tests::changes_refreshed_restores_change_after_transient_absence -- --exact` passes and runs at least one test.
- Strict OpenSpec validation passes.

## Out of Scope

- Changing OpenSpec filesystem discovery or polling frequency.
- Persisting TUI discovery state outside the process.
- Changing reducer, scheduler, queue, mark, acceptance, archive, or resume semantics.
- Automatically selecting or executing a restored proposal.
- Reworking remote TUI transport.
