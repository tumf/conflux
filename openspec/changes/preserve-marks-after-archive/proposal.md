---
change_type: implementation
priority: high
dependencies: []
references:
  - src/events.rs
  - src/tui/state/execution_mark_tests.rs
  - openspec/specs/operator-command-execution/spec.md
verifications:
  - id: archive-mark-preservation
    requirement: Archive and completion events preserve execution marks for the target and unrelated changes while genuine revocation events remain target-scoped
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/events.rs
    evidence: Deterministic event reconciliation regression tests and focused Rust test output
    rerun: cargo test --lib execution_mark -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve execution marks after archive

**Change Type**: implementation

## Problem / Context

`ExecutionMarkStore` currently revokes a target mark when `ChangeArchived` is dispatched. In the live `prior-art-graph` TUI, this revoked `add-3d-graph-visualization` at `2026-08-17T04:54:25Z`. A nearby API mark operation made the resulting TUI update visible, but the API operation did not replace the mark set.

The canonical `Event-driven execution mark reconciliation` requirement says successful archive, merge, push, and completion preserve marks. Current runtime behavior and that requirement disagree.

## Proposed Solution

Remove successful archive from mark-revoking event classification. Keep revocation for genuine target invalidation such as change-level Error, terminal Rejected, rejected/ineligible refresh, explicit dequeue, and the first merge-hook recovery transition.

Add regression coverage for an archive event with multiple marked changes. The test must prove the archived target and unrelated marks remain set in the shared store and TUI projection.

## Acceptance Criteria

- Dispatching `ChangeArchived` does not clear the archived change's execution mark.
- An unrelated marked change remains marked across the same event.
- TUI projection and `ExecutionMarkStore` agree after the event.
- Existing target-scoped revocation for Error, Rejected, ineligible refresh, and dequeue remains unchanged.
- API target-scoped mark updates continue to preserve unrelated marks.

## Explicit Completion Conditions

- Event reconciliation no longer classifies successful archive as mark-revoking.
- A deterministic Rust regression test fails on the current implementation and passes after the fix.
- Focused execution-mark tests and existing API/TUI convergence tests pass.
- The worktree is clean after the implementation commit.

## Out of Scope

- Changing queue intent, scheduler admission, archive routing, or durable workflow state.
- Persisting execution marks across process restarts.
- Changing terminal-row mark input eligibility.
- Reworking TUI rendering or introducing a new mark store.
