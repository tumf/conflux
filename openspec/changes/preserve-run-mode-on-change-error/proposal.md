---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - src/events.rs
  - src/orchestration/operator_coordinator.rs
  - src/tui/runner.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/selection_logic.rs
  - src/web/state.rs
  - /Users/tumf/.local/state/cflx/logs/beads-runner-80cdf981/2026-08-09.log
verifications:
  - id: change-error-mode-tests
    requirement: "A change-local ProcessingError preserves the active process mode and unrelated mark controls across Core, TUI, Web/API, and lifecycle projections while a genuine global Error remains fatal"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering the authoritative dispatch, CoreMode, TUI frame adoption and bulk mark behavior, Web/API snapshot, lifecycle mirror, and fatal global Error control case"
    rerun: "cargo test --lib processing_error_preserves_shared_mode && cargo test --lib processing_error_keeps_bulk_mark_available && cargo test --features web-monitoring --lib processing_error_preserves_process_snapshot"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve run mode when one change errors

**Change Type**: implementation

## Premise / Context

- In the observed TUI run, acceptance command exhaustion for `improve-log-wrap-preview` emitted `ProcessingError` while another change remained active.
- The TUI row handler correctly retained the failed change as change-level `error` without changing its local execution mode.
- The process-wide `CoreMode` nevertheless maps `ProcessingError` to `OperatorMode::Error`; `src/tui/runner.rs` adopts that Core mode on every frame, replacing the TUI handler's correct `Running` mode.
- Once the stale global mode is adopted, pressing `x` is rejected with `Bulk mark (x) is unavailable in Error mode: recovery is owned by retry`, even for unrelated eligible changes.
- `LifecycleModeMirror` and Web state contain the same change-local-to-global classification, so fixing only the visible TUI handler would leave authoritative frontends inconsistent.
- Canonical `tui-error-handling` already reserves global Error for fatal process failures and requires `ProcessingError` to remain change-local.

## Problem / Context

`ProcessingError` carries a change ID and represents terminal failure evidence for one change. The reducer and TUI row handler honor that scope, but the shared command-admission mode does not: `CoreMode::apply_event` converts the event into process-wide Error before frontend sinks run. The next TUI frame then adopts Core Error and disables normal execution-mark controls for the entire process.

The same semantic drift appears in two secondary projections. `WebState::apply_dispatch` writes `app_mode = "error"` for `ProcessingError`, and `LifecycleModeMirror::absorb` treats it like a global `ExecutionEvent::Error`. This produces conflicting interpretations of one authoritative event and risks process-level Error presentation even while the scheduler continues unrelated work.

## Proposed Solution

Treat `ProcessingError` as a change-scoped state transition at every process-mode projection boundary:

- keep reducer transition, change-local diagnostic, execution-mark revocation for the failed change, and explicit retry ownership unchanged;
- make `CoreMode::apply_event` preserve its current process mode for `ProcessingError`;
- ensure TUI frame adoption therefore retains `Running` while unrelated work remains active and does not invoke Error-mode bulk-mark rejection;
- make Web/API preserve `app_mode` and leave `process_error` unset for `ProcessingError` while still projecting the failed row and its sanitized detail;
- make the external lifecycle mirror preserve the underlying process mode for `ProcessingError`;
- retain `ExecutionEvent::Error { .. }` as the only event in this pair that enters process-wide Error, sets process error detail, disables normal mark mutation, and owns fatal recovery.

No string inspection or diagnostic-content classifier will be introduced. Scope remains determined by typed event identity.

## Acceptance Criteria

1. During a Running multi-change run, `ProcessingError { id: alpha, .. }` leaves Core, TUI, Web/API, and lifecycle process modes Running while `alpha` becomes change-level `error` with retained diagnostic evidence.
2. After that transition, pressing `x` does not produce the Error-mode recovery warning solely because `alpha` failed; unrelated eligible rows remain bulk-markable under the existing Running-mode planning rules.
3. The failed row remains excluded or routed through the existing explicit retry rules, and its stale execution mark remains revoked without changing unrelated marks.
4. A `ProcessingError` received while the process is Select, Stopping, Stopped, or already Error does not manufacture a different process-mode transition; the existing mode is preserved.
5. Web `/api/v2/state` reports `alpha.display_status = "error"`, retains sanitized `alpha.error_detail`, preserves the pre-event `app_mode`, and leaves `process_error` unset.
6. External lifecycle projection does not publish a process-fatal transition solely for `ProcessingError`.
7. A genuine `ExecutionEvent::Error { .. }` continues to enter process-wide Error across Core, TUI, Web/API, and lifecycle projection and continues to reject ordinary bulk mark mutation.

## Explicit Completion Conditions

- `src/orchestration/operator_coordinator.rs` no longer maps `ProcessingError` to `OperatorMode::Error`; focused tests cover mode preservation from Running and the other existing process modes.
- `src/tui/runner.rs` frame adoption after an authoritative `ProcessingError` retains the Core mode, and a regression test exercises the event-dispatch-to-frame-to-`x` path rather than calling only the row handler.
- The TUI regression proves an unrelated eligible row can still change mark state and proves the failed row's old mark is revoked.
- `src/web/state.rs` no longer assigns global `app_mode = "error"` for `ProcessingError`; the authoritative snapshot test proves row error detail and process mode are distinct facts.
- `src/events.rs` lifecycle mode mirroring leaves process mode unchanged for `ProcessingError` and retains fatal behavior for `ExecutionEvent::Error`.
- Tests include a global fatal control case so the change cannot pass by suppressing every Error transition.
- The declared `change-error-mode-tests` verification passes.

## Scope Rationale

Core admission mode, TUI frame adoption, Web projection, and lifecycle mirroring are tightly coupled projections of the same authoritative event. Shipping only the TUI-local change would leave command admission and other frontends contradictory, so these corrections belong in one proposal.

## Out of Scope

- Changing acceptance command retry count, provider quota handling, or agent selection.
- Automatically retrying a failed change.
- Allowing bulk mark mutation in a genuine global Error mode.
- Changing which change-local transitions revoke execution marks.
- Reclassifying repository-wide startup, workspace preparation, dependency-analysis, upstream-finalization, or run-fatal failures.
- Redesigning the execution event taxonomy or adding durable workflow state.

Repository-wide Rust format and clippy hooks are path-scoped in `.pre-commit-config.yaml`, so requirement-specific tests remain explicit in this proposal rather than being delegated to proposal-commit hooks.
