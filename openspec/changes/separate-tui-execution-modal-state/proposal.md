---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/changes/archive/2026-08-03-unify-remote-operator-commands/proposal.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/tui-mode-management/spec.md
  - openspec/specs/tui-qr-popup/spec.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - src/orchestration/operator_command.rs
  - src/orchestration/run_control.rs
  - src/tui/types.rs
  - src/tui/state.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/tui/render.rs
  - src/tui/lifecycle.rs
  - src/tui/state/selection_logic.rs
  - src/tui/state/worktree_action_logic.rs
  - src/tui/state/event_handlers
verifications:
  - id: tui-state-tests
    requirement: "TUI execution state, modal validity and input routing, bulk-mark eligibility, rendering, lifecycle projection, and TUI/Web operator compatibility preserve the specified behavior"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering valid and invalid execution/modal combinations, confirmation-time revalidation, fatal versus change-local errors, and canonical app_mode compatibility"
    rerun: "cargo test --lib tui:: && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Separate TUI execution and modal state

**Change Type**: implementation

## Problem / Context

The TUI stores execution lifecycle states and transient modal overlays in one `AppMode` enum. `Select`, `Running`, `Stopping`, `Stopped`, and `Error` describe orchestration, while `ConfirmWorktreeDelete`, `QrPopup`, and `ConfirmForceKill` describe input ownership and presentation. Opening a modal therefore overwrites execution state, QR close requires `previous_mode`, and force-kill cancel restores `Running` unconditionally.

This conflation also obscures bulk-mark diagnostics. Bulk mark follows the shared operator lifecycle matrix and rejects `Stopping` and `Error`, but the warning cannot distinguish genuine execution state from an overlay represented as another `AppMode` variant. In the observed incident, a fatal `ExecutionEvent::Error` placed the TUI in `Error`; later `x` input produced the generic warning even though the TUI remained responsive.

The archived `unify-remote-operator-commands` change now provides the shared `OperatorMode`, `OperatorCommandService`, and run-control paths consumed here. No hard dependency remains because those repository outputs are integrated into the base.

State, input routing, rendering, and lifecycle projection must change atomically. Splitting them would leave intermediate builds with ambiguous command admission or lifecycle behavior.

## Proposed Solution

Replace mixed `AppMode` with execution-only `AppExecutionMode { Select, Running, Stopping, Stopped, Error }` and optional payload-bearing `ModalState`. QR carries no destructive payload; worktree-delete carries path and branch identity; force-kill carries change ID. Remove `previous_mode` and separately mutable pending worktree confirmation fields.

Apply a variant-specific validity policy. QR survives execution transitions while its Web URL remains available. Worktree deletion survives execution transitions while a fresh observation retains the same eligible worktree identity. Force-kill survives `Running` to `Stopping` only while the target remains authoritative retryable active work. Invalidation clears the typed modal and payload atomically. Confirmation revalidates through the existing repository-backed worktree path or shared operator command service instead of trusting TUI display cache.

Route warning-popup keys first, typed modal keys second, and ordinary view keys last. Derive bulk-mark eligibility through the shared `OperatorMode` matrix while separately requiring Changes view and no overlay. Preserve immutable `Stopping` and retry-owned `Error`, with diagnostics naming the actual execution condition.

Render the base from execution/view state and overlays from modal state. Confirmation projects lifecycle `blocked`; QR projects the underlying execution lifecycle. Canonical Web `app_mode` remains execution-only.

## Acceptance Criteria

1. TUI execution and modal state are independent; no modal variant exists in `AppExecutionMode`, no previous-mode field remains, and destructive payload identity is carried by `ModalState`.
2. QR and valid worktree-delete confirmations preserve the latest execution state across background transitions. Force-kill survives `Running` to `Stopping` only while its target remains retryable active work.
3. QR invalidates when its Web URL disappears. Worktree-delete invalidates when fresh observation shows target disappearance, main/active/deleting status, or identity change. Force-kill invalidates when the target becomes terminal, absent, dequeued, non-active, non-retryable, or otherwise invalid.
4. Invalidation clears modal and payload atomically. Confirmation-time shared-service revalidation safely refuses stale worktree identity, invalid force-kill state, failed cancellation, missing termination evidence, and timeout without mutating the invalid target.
5. Warning-popup input remains highest priority; modal input is next. `x`, navigation, stop, retry, and ordinary actions cannot leak through an active overlay.
6. Bulk mark is available only in Changes view without an overlay and with execution `Select`, `Running`, or `Stopped`; `Stopping` is immutable and `Error` is retry-owned through the shared lifecycle matrix.
7. Bulk-mark rejection distinguishes `Stopping` and `Error` and never describes a modal variant as execution state.
8. Change-local processing failures preserve execution mode; fatal global `ExecutionEvent::Error` enters execution `Error`. Existing event classification is unchanged.
9. Base rendering uses execution state and valid overlays render independently, including worktree confirmation over `Error` and force-kill over valid `Stopping`, without fallback mode rewrites.
10. Typed lifecycle publication reports valid confirmations as `blocked`, QR from underlying execution, and never relies on terminal scraping or feeds workflow control.
11. Shared TUI/Web command semantics and canonical `app_mode` tokens remain execution-only and backward compatible.

## Explicit Completion Conditions

- `src/tui/types.rs` defines `AppExecutionMode` and payload-bearing `ModalState`; `src/tui/state.rs` stores both without `previous_mode` or separate worktree-confirmation payload fields.
- Modal validity is explicit and tests cover valid combinations plus every listed invalidation boundary rather than every confirmation across every mode.
- Worktree and force-kill confirmations revalidate against fresh repository/shared-service state before mutation; stale or failed destructive intent leaves authoritative state unchanged.
- Execution event handlers retain ownership of row state, timers, `current_change`, and `StopMode`; modal changes occur only through explicit validity/invalidation policy.
- Warning/modal input routing and bulk-mark admission use the declared priority and shared lifecycle matrix.
- Rendering and lifecycle projection consume both state axes without rewriting execution state.
- `cargo test --lib tui::`, `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`, `cargo fmt --check`, and `cargo clippy -- -D warnings` all pass.

## Out of Scope

- Reclassifying `ExecutionEvent::Error` producers, including background merge failures.
- Allowing mark mutation in Error or changing retry ownership.
- Changing key bindings, modal visual design, Web schemas, scheduler algorithms, worktree service semantics, or stop-and-dequeue service semantics.
- Persisting TUI state or exposing modal state through remote monitoring.
