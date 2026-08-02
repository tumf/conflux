## Context

`AppMode` currently combines five orchestration lifecycle states with three modal presentation states. Callers match that enum for command admission, screen rendering, popup routing, and external lifecycle emission. QR works around the replacement semantics with `previous_mode`, while force-kill confirmation restores `Running` directly. The integrated operator-command layer already has an execution-only `OperatorMode` and shared command services, so the TUI should align its lifecycle axis with that model rather than extending it with presentation variants.

The split is process-local UI state and complies with `openspec/CONSTITUTION.md`: neither execution presentation nor modal state becomes durable or authoritative for workflow routing.

## Goals

- Make execution lifecycle and modal input ownership independent and exhaustively typed.
- Remove restoration logic that can overwrite a newer execution transition.
- Make modal validity and payload identity explicit and fail closed on stale confirmation.
- Keep command eligibility aligned with the shared operator lifecycle matrix.
- Preserve user-visible behavior except for more accurate bulk-mark diagnostics and corrected modal restoration.
- Keep external lifecycle publication semantic and typed.

## Non-Goals

- Redesigning the TUI or changing key bindings.
- Reclassifying fatal and non-fatal execution events.
- Replacing the existing warning-popup state, which is already independent and has different scrolling behavior.
- Changing shared operator/worktree service semantics.
- Introducing durable UI state or a second scheduler state machine.

## State Model

Use a TUI-specific execution enum to avoid collision with `orchestration::state::ExecutionMode`:

```rust
enum AppExecutionMode {
    Select,
    Running,
    Stopping,
    Stopped,
    Error,
}

enum ModalState {
    QrPopup,
    ConfirmWorktreeDelete {
        path: PathBuf,
        branch: Option<String>,
    },
    ConfirmForceKill {
        change_id: String,
    },
}
```

`AppState` stores `execution_mode: AppExecutionMode` and `modal: Option<ModalState>`. `previous_mode`, `pending_worktree_action`, and `pending_worktree_branch` are removed. A confirmation and its identity-bearing payload therefore cannot diverge.

`StopMode` remains separate because it records stop-request progress within the execution lifecycle and is already consumed by shared stop command behavior.

Warning popup content remains in `warning_popup`. It is a diagnostic overlay with scrolling and explicit close semantics, not an interaction represented by the old modal variants. Its key routing remains highest priority.

## Transition Rules

### Execution transitions

Execution event handlers continue updating their existing row state, timers, `current_change`, `StopMode`, and other presentation caches. They update `execution_mode` when the event carries a lifecycle transition:

- start, resume, retry, and resolve start enter `Running` under their existing guards;
- graceful stop request enters `Stopping`;
- stop completion enters `Stopped`;
- ordinary completion enters `Select` unless existing terminal retention rules preserve `Stopped` or `Error`;
- fatal `ExecutionEvent::Error` enters `Error`;
- change-local error events update the affected row and preserve execution mode.

A background event may update execution mode while a modal is visible. Validity is then evaluated for the active modal; closing a surviving modal exposes the latest execution mode rather than a captured stale value.

### Modal validity matrix

| Modal | Survives | Invalidated by | Confirmation authority |
|---|---|---|---|
| `QrPopup` | execution transitions, including `Running` to `Stopping`, `Stopped`, or `Error` | Web monitoring disabled or `web_url` removed | current `web_url` in `AppState` |
| `ConfirmWorktreeDelete { path, branch }` | execution transitions while the same eligible worktree identity remains | fresh worktree observation shows target absent, main, active, already deleting, or path/branch identity changed | fresh repository-backed worktree observation and existing delete eligibility/service path |
| `ConfirmForceKill { change_id }` | `Running` to `Stopping` while the same target remains retryable active work | target absent, dequeued, terminal, non-active, non-retryable, or global execution state makes the operation invalid | `OperatorCommandService::stop_and_dequeue`, using reducer state and cancellation registry |

Invalidation clears `modal` once; because the payload is embedded in the variant, no independent payload can remain stale. A warning popup raised during invalidation remains an independent diagnostic overlay and still owns input first.

### Confirmation-time revalidation

Opening a confirmation is advisory, not authorization. Confirmation dispatches through the existing shared service, which revalidates authoritative state immediately before mutation:

- worktree deletion resolves the same path/branch identity from a fresh repository observation and reruns delete eligibility; mismatch or absence refuses the command;
- force-kill dispatches the change ID to `OperatorCommandService::stop_and_dequeue`; reducer status and cancellation registration determine whether cancellation/dequeue can proceed;
- stale, invalid, failed-cancellation, and timeout outcomes do not mutate the invalid target and surface through existing warning/error reporting.

The TUI display cache may trigger eager invalidation but cannot authorize either destructive operation.

## Input Routing

Key handling order is:

1. warning popup input;
2. typed modal input;
3. view and execution input.

An active modal consumes every key event. QR closes on any key as currently specified. Confirmations accept only their documented confirmation/cancel keys and consume all other keys without dispatching underlying actions.

Bulk mark requires all of:

- `view_mode == ViewMode::Changes`;
- `warning_popup.is_none()`;
- `modal.is_none()`;
- the execution mode admits mark mutation through the shared `OperatorMode` lifecycle matrix.

The existing shared policy remains authoritative: Select and Stopped use mark-only behavior, Running may update queue intent for eligible rows, Stopping is immutable, and Error is retry-owned. The TUI may format operator-facing diagnostics but must not maintain a divergent eligibility table.

## Rendering

The base screen, elapsed orchestration data, and ordinary key hints are selected from `execution_mode` and `view_mode`. Modal overlays are rendered from `modal` after the base screen. The title may show `QR Code`, `Confirm Delete`, or `Confirm Kill` while an overlay is active, but that label is presentation-only.

QR and worktree-delete overlays remain renderable above every execution mode while valid. Force-kill is renderable above Running and Stopping while its target remains valid. Error and terminal/idle transitions render their base state after force-kill invalidation. There is no fallback branch that converts an unsupported combination to Select or Running.

## External Lifecycle Projection

`TuiLifecycleSnapshot` carries execution mode and modal state separately. Projection order is:

1. valid user confirmation interactions project `LifecycleState::Blocked`, with context derived from the modal payload;
2. QR does not block workflow and projects the underlying execution lifecycle;
3. without an interaction modal, execution mode and stop mode map as before.

This preserves the canonical requirement that typed confirmation state reports blocked while preventing a QR presentation overlay from erasing working, stopping, stopped, or error semantics.

## Compatibility and Integration Order

- `unify-remote-operator-commands` is already archived and integrated; this change consumes its `OperatorMode`, `OperatorCommandService`, and run-control surfaces.
- Canonical Web/API `app_mode` remains an execution token: `select`, `running`, `stopping`, `stopped`, or `error`.
- `OperatorMode` remains the frontend-neutral admission type. `AppExecutionMode` converts explicitly to it.
- No serialized API adds modal state in this change.
- Existing key bindings and popup text remain except for bulk-mark rejection text that identifies Error or Stopping accurately.
- A later `add-remote-parallel-control` implementation should consume the shared service contracts, not this TUI-local modal representation; it is not a hard dependency of this change.

## Verification Strategy

Use fast Rust tests under existing TUI and Web modules:

- table-driven valid and invalid execution/modal combinations;
- QR survival and Web URL invalidation;
- worktree identity, activity, disappearance, and refresh invalidation;
- force-kill survival from Running to Stopping while active, plus terminal, dequeued, absent, non-active, and Error invalidation;
- confirmation-time shared-service revalidation proving stale or failed destructive intent cannot mutate authoritative state;
- key routing tests proving underlying cursor, marks, queue intent, stop, and retry are untouched while a warning or modal owns input;
- bulk-mark matrix tests for Select, Running, Stopping, Stopped, Error, view mode, warning popup, and modal presence;
- event-handler tests for fatal global error versus change-local error while overlays are present;
- lifecycle projection tests for every execution mode with no modal, QR, and valid confirmations;
- render tests proving underlying status is preserved while supported overlay titles and content remain visible;
- Web operator snapshot tests proving canonical `app_mode` tokens and action semantics remain execution-only.

Run filters as separate valid `cargo test` commands. The complete local gate is `cargo test --lib tui::`, `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`, `cargo fmt --check`, and `cargo clippy -- -D warnings`.

Tests that would exceed one second must use the repository's heavy-test feature policy; these state, service-double, and render tests are expected to remain in the default fast suite.

## Risks and Mitigations

- **Broad compile-time migration:** removing modal variants touches many matches. Use exhaustive enum matching and migrate state, handlers, rendering, and lifecycle together.
- **Stale destructive intent:** embed payload identity in the modal, invalidate on fresh observations, and revalidate through existing shared services at confirmation time.
- **TUI/Web lifecycle drift:** keep `OperatorMode` as the shared command-admission authority and test canonical app-mode projection remains execution-only.
- **Hidden input leakage:** make warning and modal routing return a consumed result for every key, with tests using high-impact underlying keys such as `x`, Escape, retry, and navigation.
