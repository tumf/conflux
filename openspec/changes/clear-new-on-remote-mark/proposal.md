---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/state/selection_logic.rs
  - src/tui/state/execution_mark_tests.rs
  - src/web/remote_control_api
  - openspec/specs/cli/spec.md
verifications:
  - id: remote-mark-new-tests
    requirement: Every settled operator execution-mark interaction acknowledges the target row's TUI NEW attention state regardless of TUI, API, client, or MCP origin
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state/execution_mark_tests.rs
    evidence: cargo test tui::state::execution_mark_tests --lib
    rerun: cargo test tui::state::execution_mark_tests --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Clear TUI NEW after remote execution-mark interaction

**Change Type**: implementation

## Problem / Context

A newly discovered proposal receives a TUI-local `NEW` badge and increments the Changes footer count. Pressing Space in the TUI clears that attention state, but the same execution-mark mutation through `/api/v2`, `cflx client`, or MCP only updates the shared `ExecutionMarkStore`. The TUI synchronizes `selected` from that store while leaving `is_new` and `new_change_count` untouched.

The operator therefore sees a proposal as both remotely handled and still new. The behavior depends on frontend origin even though all frontends mutate the same execution-mark authority.

## Proposed Solution

Make a settled operator execution-mark interaction acknowledge the target proposal's TUI-local NEW state independent of frontend origin.

- During shared execution-mark projection into the TUI, detect target rows whose authoritative mark state changed because of an external operator mutation and clear their `is_new` flag.
- Recompute or decrement `new_change_count` from the resulting rows without affecting unrelated NEW proposals.
- Preserve the existing direct TUI behavior and keep execution marks separate from queue intent, retry, lifecycle, and admission.
- Consolidate the two legacy local NEW-clearing scenarios (Select-mode selection, Running/Stopped queue addition) into one mode-independent local toggle scenario; both already run through the single execution-mark toggle path, so no behavior is removed.
- Do not clear NEW for passive refreshes, lifecycle-driven mark revocation, rejected rows, or an unchanged/no-op remote request that did not constitute a new operator interaction.

## Acceptance Criteria

- Marking a newly detected proposal through `/api/v2`, `cflx client`, or MCP removes its `NEW` badge in the live TUI and decrements the footer count.
- Unmarking through the same operator surfaces after a prior mark change has the same acknowledgement behavior when the row is still NEW.
- Unrelated newly detected proposals retain their badge and remain counted.
- Passive mark-store synchronization and system/lifecycle reconciliation do not acknowledge NEW accidentally.
- TUI Space and bulk execution-mark behavior remain unchanged.
- Queue intent and lifecycle state are not mutated by this acknowledgement.

## Explicit Completion Conditions

- Production code carries enough operator-mutation identity to distinguish a settled remote mark interaction from passive/shared-store synchronization.
- Regression tests exercise the shared API/client/MCP command boundary and final TUI projection, not only a helper that directly edits `is_new`.
- Regression tests cover mark, unmark, unchanged/no-op, unrelated rows, and system revocation.
- `cargo test tui::state::execution_mark_tests --lib` passes.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- Persisting NEW state across process restarts.
- Publishing NEW as authoritative workflow state.
- Clearing NEW merely because a row becomes visible or receives an auto-refresh.
- Changing execution-mark stability, queue admission, or Start semantics.

## Constitutional Alignment

`NEW` remains ephemeral frontend attention state. The change introduces no durable workflow state and does not make UI state authoritative for orchestration decisions.
