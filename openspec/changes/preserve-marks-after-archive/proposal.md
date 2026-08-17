---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/mark_reconciliation.rs
  - src/orchestration/mark_reconciliation/tests.rs
  - src/web/remote_control_api/tests/execution_mark_event_tests.rs
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/archive/2026-08-10-simplify-tui-run-marks/proposal.md
verifications:
  - id: archive-mark-preservation
    requirement: Archive events preserve target and unrelated execution marks in the authoritative store, TUI, and API revision while genuine revocation events remain target-scoped
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/mark_reconciliation/tests.rs
    evidence: Deterministic reconciliation and API revision regressions plus focused Rust test output
    rerun: cargo test --lib execution_mark -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve execution marks after archive

**Change Type**: implementation

## Problem / Context

`ExecutionMarkStore` currently revokes a target mark when `ChangeArchived` is dispatched. In the live `prior-art-graph` TUI, this revoked `add-3d-graph-visualization` at `2026-08-17T04:54:25Z`. A nearby API mark operation made the resulting TUI update visible, but the API operation did not replace the mark set.

This is not accidental implementation drift. Archived change `2026-08-10-simplify-tui-run-marks` intentionally made archive a terminal revocation edge because the mark was treated as next-run intent that became meaningless after archive. That design updated `remote-control-api`, while the existing `operator-command-execution` requirement continued to require successful archive to preserve marks. The canonical capabilities therefore conflict.

The desired behavior now reverses that archive-specific design: an execution mark is a lifecycle-independent operator annotation and remains visible after successful archive until explicitly cleared or a genuine invalidation edge revokes it.

## Proposed Solution

Remove `ChangeArchived` and the archive evidence variant from mark-revoking event classification. Keep revocation for genuine target invalidation such as change-level Error, terminal Rejected, rejected/ineligible refresh, explicit dequeue, and the first merge-hook recovery transition.

Modify the full `remote-control-api` requirement block so archive publishes the preserved target and unrelated marks in the same authoritative revision. Do not modify `operator-command-execution`; its canonical requirement already specifies archive preservation, and replacing it with a partial delta would delete existing scenarios during promotion.

Update both reducer reconciliation and API revision regressions. The tests must prove the archived target and unrelated marks remain set in the shared store and projected snapshot without synthesizing queue intent.

## Acceptance Criteria

- Dispatching `ChangeArchived` does not clear the archived change's execution mark.
- An unrelated marked change remains marked across the same event.
- The archive event revision reports both marks unchanged through `/api/v2`.
- TUI projection and `ExecutionMarkStore` agree after the event.
- Existing target-scoped revocation for Error, Rejected, ineligible refresh, dequeue, and first merge-hook recovery remains unchanged.
- API target-scoped mark updates continue to preserve unrelated marks.
- The resulting canonical `operator-command-execution` and `remote-control-api` requirements agree that successful archive preserves marks.

## Explicit Completion Conditions

- Event reconciliation no longer classifies successful archive as mark-revoking.
- A deterministic Rust regression test fails on the current implementation and passes after the fix.
- The existing API archive-revision regression is inverted from target-cleared to target-preserved and passes.
- Focused execution-mark tests and the existing API/TUI convergence regression pass.
- Strict and archive-gate OpenSpec validation pass without deleting canonical scenarios.
- The worktree is clean after the implementation commit.

## Out of Scope

- Changing queue intent, scheduler admission, archive routing, or durable workflow state.
- Persisting execution marks across process restarts.
- Changing terminal-row mark input eligibility.
- Reworking TUI rendering or introducing a new mark store.
