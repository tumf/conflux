---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/tui-mode-management/spec.md
  - openspec/specs/tui-qr-popup/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - src/orchestration/operator_command.rs
  - src/tui/types.rs
  - src/tui/state.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/tui/render.rs
  - src/tui/lifecycle.rs
  - src/tui/state/selection_logic.rs
  - src/tui/state/event_handlers
verifications:
  - id: tui-state-tests
    requirement: "TUI execution state, modal input routing, bulk-mark eligibility, rendering, and lifecycle projection preserve the specified behavior"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and integration test output covering execution/modal state combinations and fatal versus change-local errors"
    rerun: "cargo test tui::"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Separate TUI execution and modal state

**Change Type**: implementation

## Problem / Context

The TUI stores execution lifecycle states and transient modal overlays in one `AppMode` enum. `Select`, `Running`, `Stopping`, `Stopped`, and `Error` describe orchestration, while `ConfirmWorktreeDelete`, `QrPopup`, and `ConfirmForceKill` describe input ownership and presentation. Opening a modal therefore overwrites the execution state, QR close requires a `previous_mode` restoration slot, and force-kill confirmation returns to `Running` unconditionally.

This conflation also obscures bulk-mark diagnostics. Bulk mark correctly follows the shared operator lifecycle matrix and rejects `Stopping` and `Error`, but the warning cannot distinguish a genuine execution mode from an overlay represented as another `AppMode` variant. In the observed incident, a fatal `ExecutionEvent::Error` placed the TUI in `Error`; later `x` input produced the generic warning even though the TUI remained responsive.

The state axes, key routing, rendering, and external lifecycle projection must change together. Splitting them into separate proposals would leave intermediate builds with ambiguous input or lifecycle behavior, so this proposal keeps the refactor and its behavioral regression coverage atomic.

## Proposed Solution

Replace the mixed `AppMode` representation with an execution-only mode containing `Select`, `Running`, `Stopping`, `Stopped`, and `Error`, plus an optional modal state containing QR, worktree-delete confirmation, and force-kill confirmation. Remove `previous_mode`; opening or closing an overlay must not mutate the underlying execution state.

Route modal keys before ordinary view keys and keep warning popups as their existing independent diagnostic presentation state. Derive bulk-mark eligibility from the execution mode through the same shared `OperatorMode` lifecycle matrix used by operator commands, while separately requiring the Changes view and no active modal. Preserve the current policy that `Stopping` is immutable and `Error` recovery belongs to retry commands, but make rejection diagnostics identify the actual execution condition.

Update rendering and typed external lifecycle projection to consume both axes. Confirmation overlays continue to project `blocked`, QR projects the underlying execution lifecycle, and the visible status title may continue showing the active overlay label without becoming a workflow-control input.

## Acceptance Criteria

1. TUI execution state and modal overlay state are represented independently; no modal variant exists in the execution enum and no previous-mode restoration field is required.
2. Opening and closing QR, worktree-delete confirmation, or force-kill confirmation preserves the underlying execution state, including execution-state changes received while a modal is open.
3. Modal input is handled before underlying Changes or Worktrees input, so `x`, navigation, stop, retry, and other ordinary actions cannot leak through an active modal.
4. Bulk mark remains available only in the Changes view with no active modal and with execution mode `Select`, `Running`, or `Stopped`; `Stopping` remains immutable and `Error` remains retry-owned according to the shared operator lifecycle matrix.
5. A rejected bulk-mark action reports the actual execution condition, including distinct actionable messages for `Stopping` and `Error`, instead of describing overlay states as application modes.
6. Change-local processing failures preserve the current execution mode, while a fatal global `ExecutionEvent::Error` enters execution `Error`; this proposal does not reclassify existing event types.
7. Rendering derives its base screen and orchestration status from execution state and renders overlays from modal state without changing existing key bindings or visual layout requirements.
8. Typed external lifecycle publication reports confirmation interactions as `blocked`, reports QR from the underlying execution state, and continues to avoid terminal-screen scraping or workflow-control feedback.
9. Shared TUI/Web operator-command semantics and canonical `app_mode` tokens remain execution-only and backward compatible.

## Explicit Completion Conditions

- `src/tui/types.rs` defines separate execution and modal enums, and `src/tui/state.rs` stores both axes without `previous_mode`.
- State transition helpers for QR, worktree deletion, and force-kill confirmation only set or clear modal state; execution event handlers only mutate execution state unless explicitly clearing an invalidated interaction.
- `src/tui/key_handlers.rs`, `src/tui/command_handlers.rs`, and `src/tui/state/selection_logic.rs` route modal input first and derive bulk-mark behavior from the shared execution lifecycle classification.
- `src/tui/render.rs` and `src/tui/lifecycle.rs` consume the separated state and cover all execution/modal combinations without a fallback that silently rewrites execution state.
- Regression tests prove QR and confirmation round trips from Select, Running, Stopping, Stopped, and Error; force-kill cancel does not force Running; modal input does not reach the underlying view; and fatal versus change-local errors retain their existing classifications.
- `cargo test tui::` passes, including lifecycle, key-handler, selection, event-handler, and render tests added or updated by this change.

## Out of Scope

- Reclassifying `ExecutionEvent::Error` producers, including background merge failures.
- Allowing execution-mark mutation in Error mode or changing retry ownership.
- Changing TUI key bindings, modal visual design, warning-popup behavior, Web API schemas, or scheduler algorithms.
- Persisting TUI execution or modal state outside the process.
