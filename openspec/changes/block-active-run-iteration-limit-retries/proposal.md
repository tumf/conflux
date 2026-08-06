---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/web-monitoring/spec.md
  - openspec/specs/cli/spec.md
  - openspec/changes/fix-precomplete-apply-repair-termination/
  - src/orchestration/state.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/run_control.rs
  - src/tui/run_supervisor.rs
  - src/tui/orchestrator.rs
  - src/web/state.rs
  - src/web/remote_control_api/dto.rs
  - src/web/remote_control_api/projection.rs
  - web/app.js
verifications:
  - id: active-run-iteration-limit-regressions
    requirement: "Typed Apply iteration-limit evidence blocks every retry mutation and scheduler effect only while its owning run boundary remains active, survives through on_finish, and cannot prevent a later boundary from starting with workspace-derived state and a fresh budget"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Non-empty Rust test selection and passing output covering individual retry, direct queue alias, mixed and all-limited bulk retry, scheduler no-dispatch, finish-hook ordering, run-closing admission serialization, same-process later-run reset, TUI projection, API projection, and generated OpenAPI serialization"
    rerun: 'for filter in active_iteration_limit_retry_guard active_iteration_limit_bulk_retry active_iteration_limit_run_boundary active_iteration_limit_projection active_iteration_limit_tui; do cargo test --features web-monitoring "$filter" -- --list | grep -q ": test$" || exit 1; done && cargo test --features web-monitoring active_iteration_limit && cargo test --features web-monitoring --test openapi_contract_tests && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: active-run-iteration-limit-browser-regressions
    requirement: "The embedded console offers Retry only when the authoritative per-change action eligibility allows it and never submits a retry for an active-run iteration-limited row"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Passing Vitest output proving a server-blocked error row has no retry control or command side effect and a later allowed snapshot restores one functional retry control"
    rerun: 'npm --prefix tests/web test -- destructive-actions.spec.js'
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Block active-run iteration-limit retries

**Change Type**: implementation

## Premise / Context

- Apply dispatches are already counted by one per-change budget in the current run's shared `OrchestratorState`.
- When that budget refuses another dispatch, the executor records typed `ApplyIterationLimit` evidence so the run boundary can report `iteration_limit` and the exact Apply count to `on_finish`.
- The retry path currently mutates `error` back to queue intent, publishes an explicit-retry edge, marks the row, and wakes or starts scheduling before it accounts for that typed evidence.
- If retry arrives while the same scheduler boundary is still active, the retry cannot create budget: the existing `ApplyBudget` remains exhausted, so it immediately reaches the same refusal again.
- A later scheduler boundary in the same process replaces active-run state and must re-evaluate the preserved workspace with a fresh budget. Process restart has the same workspace-derived property.
- The embedded console currently derives its Retry button from `display_status` instead of the server's per-change action eligibility, and the TUI has no typed iteration-limit eligibility cache for its retry marks and guidance.

## Problem / Context

After a change reaches the Apply ceiling, an operator can retry quickly enough that the current boundary still owns the exhausted budget. `RetryError` then removes the failed classification, `publish_explicit_retry` releases the retry edge, marks and queue intent change, and the scheduler is notified. The same active boundary attempts another Apply reservation, is refused at the same count, and returns to error. Repeating the action creates a no-progress loop and misleadingly reports accepted operator intent that could never dispatch an Apply child.

Rejecting retry forever is also incorrect. `ApplyIterationLimit` is ephemeral active-run evidence, not durable workflow state. Once the owning boundary has invoked `on_finish` and closed, a new boundary must be allowed to derive its route from the worktree and create a fresh active-run budget. Clearing the evidence too early would hide the typed finish outcome; clearing it without coordinating scheduler admission would allow a retry to target a boundary that is still closing.

The service, `/api/v2`, WebUI, and TUI therefore need one lifecycle-aware eligibility contract rather than frontend-specific string checks.

## Proposed Solution

1. Treat a recorded `ApplyIterationLimit` as a retry gate only while its owning scheduler boundary is active. Keep the typed record available through that boundary's sole `on_finish` attempt.
2. Serialize run closure with operator admission. After `on_finish` returns, retire the gate and publish the boundary as inactive without any interval in which a retry can mutate state yet still notify the exhausted or closing scheduler.
3. Before any retry mutation, have the shared operator/run-control service reject a limited target with a typed reason. Cover `retry_change`, the terminal-error branch of queue addition used by `set_queue_intent`, TUI retry marks, and every equivalent adapter path.
4. Make bulk retry partial: omit active-run-limited targets, dispatch other retryable targets once, and return no-op with no scheduler effect when every candidate is limited or otherwise ineligible.
5. Project active typed evidence as per-change `{ attempts, max }` data in `/api/v2` and expose `retry_change` as blocked with the stable reason `apply_iteration_limit_active`. Derive TUI retry eligibility from the same reducer/run-boundary state.
6. Make the browser render Retry only from `change.actions.retry_change.allowed`. Do not infer permission from `display_status`, diagnostics, logs, or the iteration number.
7. After the owning boundary closes, remove the active gate from authoritative projections. A later retry may create a new scheduler boundary, whose `OrchestratorState` and Apply budget are initialized from current workspace evidence.
8. Keep the rule process-local. Do not reset an active budget, persist the limit record, add configuration, or parse the human-readable max-iterations error.

## Atomic Scope Rationale

The service guard, run-closing lifetime, API projection, and frontend suppression are one admission contract. Shipping only the guard would leave WebUI and TUI advertising an impossible action; shipping only frontend changes would leave remote and direct queue aliases able to mutate the exhausted run. The scopes cannot be verified independently without a period of contradictory operator behavior, so they remain one proposal.

`fix-precomplete-apply-repair-termination` addresses why a repair command can consume the budget; this proposal addresses what happens after any legitimate budget exhaustion. The retry guard consumes typed limit evidence already present in the base and does not require the watchdog change's repository output, so no hard dependency is declared.

## Acceptance Criteria

1. While a scheduler boundary owns typed iteration-limit evidence for a change, individual retry is rejected before reducer mutation, failed-classification release, execution-mark mutation, dynamic-queue mutation, explicit-retry publication, queue hooks, scheduler notification, or scheduler spawn.
2. `set_queue_intent=true` or any direct queue-add alias cannot bypass the same guard for an iteration-limited terminal-error row.
3. Bulk retry excludes active-run-limited changes while dispatching all other retryable candidates exactly once; an all-limited request is a no-op with no scheduler effect.
4. The sole finish-hook owner observes `status=iteration_limit` and the exact cumulative Apply count before the gate is retired, including when the hook itself reports an error.
5. Run closure and retry admission are serialized so no accepted retry can be delivered to the exhausted or closing scheduler boundary.
6. After the owning boundary closes, a later retry in the same process may start a new boundary. That boundary re-derives routing from workspace and Git evidence and owns a fresh per-change Apply budget.
7. Process restart discards the gate and re-derives routing from workspace evidence; no logs, API snapshots, local-state files, or durable retry artifacts restore it.
8. `/api/v2` exposes active typed iteration-limit evidence with `attempts` and `max`, and reports `retry_change.allowed=false` with `blocked_reason=apply_iteration_limit_active` at the same authoritative revision.
9. The WebUI renders no Retry control and submits no command for that blocked action, even when `display_status` is `error`; it restores Retry when a later authoritative snapshot allows it.
10. The TUI does not create or clear a retry mark for an active-run-limited row, excludes it from bulk retry selection, and does not display Space/F5 guidance that promises retry. It shows a stable explanation instead.
11. Existing retry behavior for ordinary terminal errors, resumable acceptance stalls, and external holds remains unchanged when no active typed limit applies.
12. Added default-suite Rust tests remain under one second each or follow the repository heavy-test policy when an existing platform boundary makes that impractical.

## Explicit Completion Conditions

- `src/orchestration/state.rs` and the run-boundary owners expose an explicit active lifetime for typed iteration-limit evidence and a close operation ordered after the sole finish-hook attempt.
- The close operation and operator admission share a synchronization boundary that prevents a finishing-run race; tests deterministically pause closure and prove retry is either rejected by the old boundary or admitted to a new one, never notified into the old one.
- `src/orchestration/operator_command.rs` and `src/orchestration/run_control.rs` apply one typed guard before every individual or bulk retry mutation and before the terminal-error queue alias.
- Rejection tests compare reducer state, error detail, marks, queue contents, explicit-retry publications, hook counts, scheduler notifications, and scheduler starts before and after the request.
- `src/web/state.rs`, v2 DTO/projection code, and generated OpenAPI serialization carry the typed evidence and stable blocked reason without parsing prose.
- `web/app.js` uses `change.actions.retry_change`; `tests/web/destructive-actions.spec.js` proves both suppressed and restored behavior.
- TUI reducer synchronization, selection logic, command handling, and rendering consume typed eligibility and suppress row, bulk, Space, and F5 retry paths consistently.
- The declared Rust and browser verification commands pass with non-empty named Rust test families.

## Out of Scope

- Resetting or increasing `max_iterations` inside an active run.
- Persisting Apply counters or iteration-limit evidence across process restart.
- Parsing or standardizing the human-readable max-iterations error message as workflow input.
- Automatically retrying after a run closes, adding cooldowns, backoff, or retry timers.
- Changing Apply watchdog, repair-command lifetime, Acceptance budgets, stall policy, or cleanup-review behavior.
- Changing which workspace evidence selects Apply, Acceptance, Archive, Resolve, or Merged on a later run.
- Adding a new configuration key or a standalone frontend-specific retry policy.
