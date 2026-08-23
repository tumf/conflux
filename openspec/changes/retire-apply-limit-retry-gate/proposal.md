---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
verifications:
  - id: explicit-retry-tests
    requirement: A settled Apply-limit error accepts a later explicit operator retry without automatic redispatch
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: cargo test operator_command --lib
    rerun: cargo test operator_command --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---
# Retire the Apply-limit gate when failure settles

**Change Type**: implementation

## Problem

An Apply invocation that reaches its iteration ceiling settles the change into terminal `error`, but the command-capable TUI keeps the scheduler task alive. Retry eligibility is currently tied to that scheduler task's lifetime rather than to the failed invocation. The row therefore remains marked but `start` reports no eligible target indefinitely:

`no marked change is startable ... excluded: <change> (error)`

The retained `ApplyIterationLimit` record is valid diagnostic evidence. It must prevent another dispatch inside the exhausted invocation, but it must not make an explicit later operator retry impossible merely because the persistent scheduler remains alive.

## Proposed Solution

Scope the Apply-limit guard to automatic work in the invocation that exhausted the budget. Once the failure is settled as a terminal change-local error, preserve the diagnostic record but allow an explicit `RetryChange`, bulk retry, or Start-selected retry to consume the ordinary terminal-error route and create a fresh execution boundary with a fresh Apply budget.

Remove `apply_iteration_limit_active` as an operator-action block from the shared command service, TUI eligibility, and `/api/v2` action projection. Keep the existing rule that queue reconciliation, mark settlement, and generic scheduler notification cannot synthesize a retry; only explicit retry intent may clear the terminal error.

## Acceptance Criteria

- A terminal `error` caused by the Apply iteration limit is retryable immediately after the failed invocation settles, even while the persistent scheduler task remains alive.
- Marking that row and invoking Start admits exactly one explicit retry and creates fresh Apply budget.
- Individual and bulk retry use the same behavior; unrelated eligible targets remain unaffected.
- No automatic scheduler cycle, queue reconciliation, queue addition, or mark settlement retries the exhausted change.
- The retained iteration-limit record and error detail remain observable until explicit retry consumes the terminal error.
- `/api/v2/state`, TUI guidance, and command admission agree that the settled error is retryable.

## Explicit Completion Conditions

- Shared operator-command tests reproduce a live persistent scheduler plus retained `ApplyIterationLimit` and prove explicit retry is accepted after terminal error settlement.
- Tests prove the same state is not redispatched without explicit retry intent.
- API projection and TUI tests no longer expose `apply_iteration_limit_active` as a block for a settled terminal error.
- `cargo test operator_command --lib` passes and selects the new regression tests.

## Out of Scope

- Increasing the Apply iteration limit.
- Removing iteration-limit diagnostics.
- Automatically retrying an exhausted invocation.
- Changing retry behavior for unsupported evidence, acceptance holds, or terminal outcomes other than `error`.

## Root-cause evidence

The observed run reached Apply #13 after repeated agent launcher failures (`/Users/tumf/bin/claude-auto: line 380: account_reports[@]: unbound variable`). Conflux retained the implementation edits and correctly classified the change as `error`, but `/api/v2/state` reported `retry_change` blocked by `apply_iteration_limit_active` while the persistent scheduler remained live. The gate's retirement condition therefore depends on owner lifetime rather than the failed invocation's settlement.

## Final Validation

Archive validation itself is authoritative. Expected archive gate: `cflx openspec validate retire-apply-limit-retry-gate --archive-gate`.
