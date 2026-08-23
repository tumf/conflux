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

Remove `apply_iteration_limit_active` as an operator-action block from the shared command service, TUI eligibility, and `/api/v2` action projection. Keep the existing rule that queue reconciliation, mark settlement, and generic scheduler notification cannot synthesize a retry; only explicit retry intent — individual retry, bulk retry, retry-class Start, or the explicit per-target terminal-error queue-intent alias — may clear the terminal error.

Retire the gate from every canonical requirement that mandates it: `operator-command-execution` (`Retry routing preserves reconciled evidence`, `Mode-aware mark and queue behavior`), `cli` (`Error Retry with F5 Key`, `Error State Display`, `Footer Dynamic Guidance Display`), and `remote-control-api` (`Authoritative operator snapshot`, `Shared lifecycle scheduling semantics`). A partial retirement would leave canonical requirements simultaneously mandating and forbidding the same blocked reason.

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

## Retired Scenarios

Each of these canonical scenarios exists only to mandate the gate this change
retires, so each is replaced by a renamed scenario asserting the opposite inside
the same requirement. No coverage is dropped: the retained iteration-limit
record, the headless projection, the bulk-retry partition, the TUI error header,
and the footer guidance are all still pinned — by the scenario that names the new
behavior rather than the retired one.

- remote-control-api: Authoritative operator snapshot / Active iteration limit is projected as typed eligibility
- remote-control-api: Authoritative operator snapshot / Scheduler-task exit removes the active action block
- remote-control-api: Authoritative operator snapshot / Headless read-only projection does not retain an actionable block
- remote-control-api: Shared lifecycle scheduling semantics / V2 individual retry reports active limit refusal
- remote-control-api: Shared lifecycle scheduling semantics / V2 bulk retry remains partial
- cli: Error State Display / Active iteration limit replaces retry guidance
- cli: Error Retry with F5 Key / F5 cannot target an active limited run
- cli: Error Retry with F5 Key / F5 becomes available after boundary closure
- cli: Error Retry with F5 Key / Active iteration limit remains mutation-free
- cli: Footer Dynamic Guidance Display / Limited error rows do not produce retry promises
- cli: Footer Dynamic Guidance Display / Bulk mark selection excludes a limited error row
- operator-command-execution: Retry routing preserves reconciled evidence / Individual active-limit retry is mutation-free
- operator-command-execution: Retry routing preserves reconciled evidence / Bulk retry skips only active limited targets
- operator-command-execution: Retry routing preserves reconciled evidence / All-limited bulk retry is a no-op
- operator-command-execution: Retry routing preserves reconciled evidence / Later boundary uses ordinary retry routing
- operator-command-execution: Mode-aware mark and queue behavior / Bulk mark excludes active limited queue aliases before mutation
- operator-command-execution: Mode-aware mark and queue behavior / Queue intent cannot alias an active limited retry

## Out of Scope

- Increasing the Apply iteration limit.
- Removing iteration-limit diagnostics.
- Automatically retrying an exhausted invocation.
- Changing retry behavior for unsupported evidence, acceptance holds, or terminal outcomes other than `error`.
- Rewriting `web-monitoring` console scenarios whose Given is a snapshot blocked by `apply_iteration_limit_active` (`Server-blocked error row offers no Retry`, `Later allowed snapshot restores Retry`). They condition on a projection this change makes unreachable rather than mandating the gate, so they become vacuous, not contradictory; a follow-up change must retire or generalize them.

## Root-cause evidence

The observed run reached Apply #13 after repeated agent launcher failures (`/Users/tumf/bin/claude-auto: line 380: account_reports[@]: unbound variable`). Conflux retained the implementation edits and correctly classified the change as `error`, but `/api/v2/state` reported `retry_change` blocked by `apply_iteration_limit_active` while the persistent scheduler remained live. The gate's retirement condition therefore depends on owner lifetime rather than the failed invocation's settlement.

## Final Validation

Archive validation itself is authoritative. Expected archive gate: `cflx openspec validate retire-apply-limit-retry-gate --archive-gate`.
