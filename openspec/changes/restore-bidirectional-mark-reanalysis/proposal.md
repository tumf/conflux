---
change_type: implementation
priority: high
dependencies: []
verifications:
  - id: local-regression
    requirement: Mark reconciliation and capacity-gated analysis behave correctly before integration
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: focused and full Cargo test output
    rerun: cargo test --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
references:
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: Restore delta-scoped bidirectional mark reconciliation

Change Type: implementation

## Why

Changing an execution mark currently does not reliably update ordinary queue intent in both directions. Mark additions can eventually queue work, but removing a mark leaves ordinary pending work queued and the TUI remains gray/queued. The current settlement plan only produces additions.

A global bidirectional scan is unsafe because explicit queue controls do not make marks authoritative. Reconciliation must affect only targets whose marks changed in the settled batch.

Analyze also currently runs with zero worker capacity under the canonical scheduler contract. The requested behavior requires a scheduler-level capacity gate, not merely suppressing settlement notification.

## What changes

- Record accepted individual, bulk, and API mark deltas in one common process-local settlement batch.
- After the existing stability window, re-read and reconcile only targets named by that batch.
- Add queue intent for newly marked eligible ordinary `not queued` targets.
- Remove queue intent for newly unmarked idle ordinary pending targets.
- Preserve unrelated explicit queue additions/removals and all active, retry, waiting, blocked, stalled, terminal, archive-complete, unknown, or ineligible lifecycle evidence.
- Revalidate every settlement mutation under the reducer write boundary so dispatch/terminal races become reasoned no-ops rather than dequeue or retry aliases.
- Notify the scheduler exactly once after a settled batch with applied queue membership changes; do not notify for no-op batches.
- Keep classification, reducer reconciliation, and diagnostics available at zero capacity, but gate the expensive dependency analyzer and ordinary dispatch on freshly recomputed positive worker capacity.
- Preserve unconsumed edges and avoid completed/suppression signatures when capacity prevents analysis, allowing slot recovery to re-evaluate queued work.

## Acceptance criteria

- Individual, bulk, and API mark mutations use the same settlement and reconciliation rules.
- Unmarking an idle ordinary pending queued target changes reducer intent to `NotQueued` and the TUI projects `not queued`.
- Marking an eligible ordinary `not queued` target queues it after stabilization.
- Settlement never mutates queue intent for targets not named by accepted mark deltas.
- Active or raced lifecycle evidence is never cleared; settlement additions never alias `RetryError`.
- One settled batch emits at most one scheduler notification and only when membership changed.
- Zero worker capacity never starts dependency analysis, records suppression/completion, or consumes an unevaluated edge.
- Positive slot recovery re-evaluates remaining eligible queued work.
- Empty eligible queue and no-op settlement start no Analyze.

## Impact

- Specs: `operator-command-execution`, `parallel-execution`
- Code: mark settlement/coordinator, guarded queue command application, scheduler reanalysis guard
- Tests: orchestration/operator command and parallel scheduler regression suites
- No new dependencies or durable workflow-control state
