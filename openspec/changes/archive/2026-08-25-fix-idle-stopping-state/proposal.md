---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/operator_coordinator.rs
  - src/orchestration/run_control.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers/idle_start_running_tests.rs
verifications:
  - id: idle-stopping-regressions
    requirement: No-work stop and cancel-stop converge on truthful lifecycle modes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/command_handlers/idle_start_running_tests.rs
    evidence: Focused Rust test output proving terminal no-work stop and cancel-stop projections across Core, TUI, and Web
    rerun: cargo test idle_start_running_tests --locked
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix idle scheduler Stopping and Running state retention

**Change Type**: implementation

## Problem / Context

A persistent scheduler can have no executable or in-flight work while the application remains `Stopping`. In that stale state, pressing F5 is translated to cancel-stop. Cancel-stop then restores `Running` when the persistent-idle episode fact was lost, although no accepted Start or typed work-start event exists. No later idle edge is guaranteed, so the TUI can remain `Running` indefinitely with nothing to execute.

This violates the existing rule that lifecycle presentation must be backed by typed scheduler or operator evidence.

## Proposed Solution

Make the shared lifecycle boundary converge a no-work graceful stop to its truthful inactive state instead of retaining `Stopping`. Preserve the persistent-idle origin through stop settlement so cancel-stop can restore Ready when no accepted Start or typed work-start event has opened a run episode.

Keep F5 as cancel-stop while a genuine graceful stop is pending. Do not fix the symptom with a TUI-only status rewrite: Core, TUI, and Web must consume the same authoritative outcome and agree.

## Acceptance Criteria

- A graceful stop requested while the persistent scheduler has no executable, queued, admitted, active, resolve, merge, or cleanup work reaches inactive `Stopped`/Ready without waiting for a nonexistent work boundary.
- A no-work stop request cannot remain indefinitely in `Stopping` after the scheduler has settled.
- Cancel-stop from an idle-origin stopping episode returns to Ready/`select`; it does not claim `Running` without accepted Start or typed work-start evidence.
- Cancel-stop after accepted Start or typed work-start evidence still returns to `Running`.
- Core, TUI, and Web projections agree for every transition.
- Execution marks remain unchanged. No queue intent or synthetic work event is introduced.

## Explicit Completion Conditions

- Regression tests reproduce both reported sequences before the fix:
  - no work followed by graceful stop remains `Stopping`;
  - F5/cancel-stop from that state produces unsupported `Running` retention.
- The focused tests pass after one shared-boundary fix and assert Core, TUI, Web, scheduler liveness, persistent-idle state, and absence of executable work.
- Existing stop/cancel-stop and persistent-idle tests remain green.
- `cargo test idle_start_running_tests --locked` succeeds.

## Retired Scenarios

Scenarios this change deliberately retires from the MODIFIED requirement, rather
than losing by accident. Declared here because the promotion-safety regression
treats every other disappearance as a coverage regression, and because a
declaration inside a delta block would be copied verbatim into a canonical spec
that should describe the system rather than the history of one change.

- operator-command-execution: Persistent-idle Ready remains a live run-control target / cancel stop returns to idle Ready
  — replaced by `Cancel idle-origin stop does not invent Running`, which asserts
  strictly more: the withdrawal returns to Ready for *every* parked Ready the
  stop could have been accepted from, including the one an `AllCompleted`
  settlement produced, and it names the absence of accepted Start or typed
  work-start evidence as the reason rather than the presence of an idle edge.
- operator-command-execution: Persistent-idle Ready remains a live run-control target / accepted Start makes later cancel stop return to Running
  — replaced by `Cancel stop after real work restores Running`, which states the
  same restoration for the same episode and keeps the accepted-Start evidence as
  its precondition.

## Out of Scope

- Changing F5 key bindings or removing cancel-stop.
- Changing execution-mark persistence.
- Adding timers, polling, frontend-only reconciliation, or durable lifecycle state.
- Altering active-work graceful-stop or force-stop semantics.

## Verification Ownership

The focused repository-local regression suite is the change-blocking proof. Repository-wide formatting and Clippy remain owned by the tracked path-scoped pre-commit hooks when Rust files change.
