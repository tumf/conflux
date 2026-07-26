---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/runner.rs
  - src/orchestration/state.rs
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
verifications:
  - id: tui-manual-deferral-regression
    requirement: Manual merge deferral clears stale TUI-local resolve reservation and leaves the row retryable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering reducer-first manual deferral, local reservation cleanup, clean second retry, and auto-resumable preservation
    rerun: make test
    prerequisites: []
---

# Fix TUI manual deferral resolve reservation

**Change Type**: implementation

## Premise / Context

- Pressing `M` on a `merge wait` row correctly records reducer-owned `ResolveWait` and wakes the scheduler.
- The observed run reached scheduler retry dispatch, then detected a dirty base workspace and emitted `MergeDeferred(auto_resumable=false)` before any `ResolveStarted` event.
- The shared reducer correctly demoted the change to `MergeWait` and cleared reducer-owned resolve-wait membership.
- `AppState::resolve_merge()` had already set the TUI-local `is_resolving` reservation to prevent consecutive `M` presses. `handle_merge_deferred()` mistakes that reservation for an active resolve, re-adds the same change to the local resolve queue, and overwrites the reducer-derived `merge wait` display with `resolve pending`.
- Restarting the TUI clears the stale local reservation and queue, explaining why the same retry works after restart.
- Canonical specs already require manual deferral to return to `merge wait`; this proposal fixes runtime conformance rather than changing scheduler ownership.

## Problem / Context

A manual merge retry can be classified as manually blocked before `ResolveStarted`. In that event ordering, the reducer truth is `MergeWait`, but TUI-local serialization state remains reserved and recreates a display-only pending state. No resolve lifecycle completion follows, so the local queue cannot drain, later refreshes preserve the misleading row, and another `M` is ignored because the row no longer appears as `merge wait`.

This violates truthful state display and forces users to restart `cflx` before retrying a cleaned workspace.

## Proposed Solution

Align TUI-local resolve serialization with reducer-owned manual deferral outcomes:

1. Treat `MergeDeferred(auto_resumable=false)` as authoritative manual-blocker evidence even when `is_resolving` is true only because `M` optimistically reserved the local resolve slot.
2. When the deferred change has not entered actual `resolving`, clear its local resolve reservation, remove it from the TUI-local resolve queue/set, and retain the reducer-derived `merge wait` display.
3. Do not emit another `TuiCommand::ResolveMerge` or recreate local `resolve pending` for that manual deferral.
4. Preserve `auto_resumable=true` queueing and preserve serialization when another change is actually resolving.
5. Add focused regression coverage using the production event order: reducer applies `MergeDeferred(false)`, TUI syncs the reducer display, then the TUI event handler processes the event.

The change remains one proposal because reservation cleanup, row status, and retry actionability are one lifecycle invariant and cannot be verified independently.

## Acceptance Criteria

- If `M` reserves the local resolve slot and retry evaluation emits `MergeDeferred(auto_resumable=false)` before `ResolveStarted`, the row ends at `merge wait`, not `resolve pending`.
- The deferred change is absent from TUI-local `resolve_queue` and `resolve_queue_set`, and the stale `is_resolving` reservation is cleared when no other change is actually resolving.
- The manual deferral does not return or enqueue another `TuiCommand::ResolveMerge`.
- After the user cleans the workspace/base, the next `M` on the same row creates a fresh scheduler-consumable retry without restarting `cflx`.
- `MergeDeferred(auto_resumable=true)` continues to leave scheduler-owned retry work at `resolve pending` when it is genuinely waiting behind an active base-mutating lane.
- A manual deferral for one change does not clear serialization for a different change that is actually displayed as `resolving`.
- TUI display after the event remains consistent with `OrchestratorState::all_display_statuses()`.

## Explicit Completion Conditions

- `src/tui/state/event_handlers/errors.rs` handles manual deferral independently from the optimistic `is_resolving` reservation and does not locally recreate cleared reducer retry intent.
- `src/tui/state.rs` provides the minimal queue cleanup needed to remove a specific deferred change while preserving FIFO order for unrelated queued resolves.
- Unit tests reproduce the reducer-first production event order and fail if the row, local reservation, or local queue remains stale.
- Tests prove a clean second `M` is actionable without TUI restart and prove existing auto-resumable and different-active-change behavior remains intact.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, focused TUI tests, and the default `cargo test` suite pass.
- New default-path tests complete within one second or are optimized; they must not be moved to the heavy tier merely to hide a local state regression.

## Out of Scope

- Changing scheduler notification, retry dispatch, or base-dirty classification, which worked in the observed run.
- Replacing reducer-owned workflow state with TUI-local or externally persisted state.
- Removing resolve serialization or redesigning the full TUI resolve queue.
- Changing `M` key bindings, merge conflict resolution, or unrelated apply/accept/archive behavior.
- Adding restart-time persistence for TUI-local state.
