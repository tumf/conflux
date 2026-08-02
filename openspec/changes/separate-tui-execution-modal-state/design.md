## Context

`AppMode` currently combines five orchestration lifecycle states with three modal presentation states. Callers match that enum for command admission, screen rendering, popup routing, and external lifecycle emission. QR works around the replacement semantics with `previous_mode`, while force-kill confirmation restores `Running` directly. The shared operator command layer already has an execution-only `OperatorMode`, so the TUI should align its lifecycle axis with that model rather than extending it with presentation variants.

The split is process-local UI state and complies with `openspec/CONSTITUTION.md`: neither execution presentation nor modal state becomes durable or authoritative for workflow routing.

## Goals

- Make execution lifecycle and modal input ownership independent and exhaustively typed.
- Remove restoration logic that can overwrite a newer execution transition.
- Keep command eligibility aligned with the shared operator lifecycle matrix.
- Preserve user-visible behavior except for more accurate bulk-mark diagnostics and corrected modal restoration.
- Keep external lifecycle publication semantic and typed.

## Non-Goals

- Redesigning the TUI or changing key bindings.
- Reclassifying fatal and non-fatal execution events.
- Replacing the existing warning-popup state, which is already independent and has different scrolling behavior.
- Introducing durable UI state or a second scheduler state machine.

## State Model

Use an execution-only enum and a modal enum:

```rust
enum ExecutionMode {
    Select,
    Running,
    Stopping,
    Stopped,
    Error,
}

enum ModalState {
    QrPopup,
    ConfirmWorktreeDelete,
    ConfirmForceKill { change_id: String },
}
```

`AppState` stores `execution_mode: ExecutionMode` and `modal: Option<ModalState>`. `previous_mode` is removed. `StopMode` remains separate because it records stop-request progress within the execution lifecycle and is already consumed by shared stop command behavior.

Warning popup content remains in `warning_popup`. It is a diagnostic overlay with scrolling and explicit close semantics, not an interaction represented by the old modal variants. Its key routing remains highest priority.

## Transition Rules

### Execution transitions

Execution event handlers update only `execution_mode`:

- start, resume, retry, and resolve start enter `Running` under their existing guards;
- graceful stop request enters `Stopping`;
- stop completion enters `Stopped`;
- ordinary completion enters `Select` unless existing terminal retention rules preserve `Stopped` or `Error`;
- fatal `ExecutionEvent::Error` enters `Error`;
- change-local error events update the affected row and preserve execution mode.

A background event may update execution mode while a modal is visible. Closing that modal exposes the latest execution mode rather than a captured stale value.

### Modal transitions

Opening QR or a confirmation sets `modal`. Closing, canceling, or completing the interaction clears `modal`. No modal helper writes `execution_mode`.

If an execution transition makes an interaction nonsensical, the event handler may clear that modal explicitly. This must be variant-specific and tested; it must not restore a captured mode. In particular, force-kill confirmation is valid only while the target remains force-stoppable, whereas QR remains presentation-only across execution transitions.

Worktree confirmation continues to use the existing pending worktree action data for its command payload. Clearing or invalidating that confirmation must also clear its pending payload so stale actions cannot be submitted later.

## Input Routing

Key handling order is:

1. warning popup input;
2. typed modal input;
3. view and execution input.

An active modal consumes every key event. QR closes on any key as currently specified. Confirmations accept only their documented confirmation/cancel keys and consume all other keys without dispatching underlying actions.

Bulk mark requires all of:

- `view_mode == ViewMode::Changes`;
- `modal.is_none()`;
- the execution mode admits mark mutation through the shared `OperatorMode` lifecycle matrix.

The existing shared policy remains authoritative: Select and Stopped use mark-only behavior, Running may update queue intent for eligible rows, Stopping is immutable, and Error is retry-owned. The TUI may format operator-facing diagnostics but must not maintain a divergent eligibility table.

## Rendering

The base screen, elapsed orchestration data, and ordinary key hints are selected from `execution_mode` and `view_mode`. Modal overlays are rendered from `modal` after the base screen. The title may show `QR Code`, `Confirm Delete`, or `Confirm Kill` while an overlay is active, but that label is presentation-only.

All execution modes must remain renderable beneath each modal. There is no default branch that converts unknown combinations to Select or Running.

## External Lifecycle Projection

`TuiLifecycleSnapshot` carries execution mode and modal state separately. Projection order is:

1. user confirmation interactions project `LifecycleState::Blocked`, with force-kill context derived from its modal payload;
2. QR does not block workflow and projects the underlying execution lifecycle;
3. without an interaction modal, execution mode and stop mode map as before.

This preserves the canonical requirement that typed confirmation state reports blocked while preventing a QR presentation overlay from erasing working, stopping, stopped, or error semantics.

## Compatibility

- Canonical Web/API `app_mode` remains an execution token: `select`, `running`, `stopping`, `stopped`, or `error`.
- `OperatorMode` remains the frontend-neutral admission type. The TUI execution enum converts explicitly to it or is replaced by it only if doing so preserves TUI-local naming and exhaustive tests.
- No serialized API adds modal state in this change.
- Existing key bindings and popup text remain except for bulk-mark rejection text that identifies Error or Stopping accurately.

## Verification Strategy

Use fast Rust tests under existing TUI modules:

- table-driven execution/modal round trips for QR and confirmations;
- key routing tests proving underlying cursor, marks, queue intent, stop, and retry are untouched while a modal is active;
- bulk-mark matrix tests for Select, Running, Stopping, Stopped, Error, view mode, and modal presence;
- event-handler tests for fatal global error versus change-local error while a modal is present;
- lifecycle projection tests for every execution mode with no modal, QR, and confirmations;
- render tests proving the underlying status is preserved while overlay titles and content remain visible.

Tests that would exceed one second must use the repository's heavy-test feature policy; these state and render tests are expected to remain in the default fast suite.

## Risks and Mitigations

- **Broad compile-time migration:** removing modal variants touches many matches. Use exhaustive enum matching and migrate state, handlers, rendering, and lifecycle together.
- **Stale confirmation payload:** clear pending worktree payload whenever its modal is cleared or invalidated, and test cancel/error paths.
- **TUI/Web lifecycle drift:** keep `OperatorMode` as the shared command-admission authority and test canonical app-mode projection remains execution-only.
- **Hidden input leakage:** make modal routing return a consumed result for every key, with tests using high-impact underlying keys such as `x`, Escape, retry, and navigation.
