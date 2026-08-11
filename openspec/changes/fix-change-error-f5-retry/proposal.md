---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/changes/archive/2026-08-10-preserve-run-mode-on-change-error/proposal.md
  - src/orchestration/run_control.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/operator_coordinator.rs
  - src/tui/command_handlers.rs
  - src/tui/run_supervisor.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
verifications:
  - id: change-error-f5-retry-tests
    requirement: Marked retry-eligible change-local errors can be retried through Start/F5 without requiring process-wide Error mode
    phase: pre-integration
    owner: conflux-acceptance
    trigger: apply-completion
    automation: Makefile
    evidence: Focused lib-target tests named change_error_f5_retry_* prove routing, atomicity, scheduler analysis, frontend parity, and refusal cases
    rerun: make test-change-error-f5-retry
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry Marked Change-Local Errors with F5

**Change Type**: implementation

## Premise / Context

- A real TUI run left `restore-running-mark-reanalysis` in change-level `error` after Apply reached its 3600-second absolute runtime limit.
- `ProcessingError` correctly revoked the stale execution mark and left the persistent scheduler alive in Select presentation.
- Re-marking the error row and pressing F5 was rejected with `none with status 'not queued'` instead of entering explicit retry and dependency analysis.
- The prior `preserve-run-mode-on-change-error` change intentionally stopped promoting change-local failures into process-wide Error, but Start still selects retry routing only from process-wide mode.

## Problem / Context

Start/F5 currently chooses one of two routes from process mode alone: Error mode plans retries, while Select and Stopped admit only marked `not queued` rows; Running rejects Start. A retry-eligible change-local error therefore cannot use the configured F5 recovery control while the process correctly remains Running or settles into persistent-idle Select. The row can be marked, but final admission excludes it as `error` before `ReducerCommand::RetryError`, the target-specific explicit-retry edge, scheduler wake, or dependency analysis can occur.

The runtime-limit message forbids automatic retry of the terminated invocation in the same run. It does not make a later operator-requested retry ineligible. Existing retry classification, active Apply iteration-limit gating, workspace-evidence checks, and explicit-retry edge ownership already define the safe retry boundary; Start must reach that boundary without reintroducing process-wide Error for a change-local failure.

## Proposed Solution

Make Start/F5 retry classification evidence-aware in every non-stopping process mode where an operator can request recovery.

- In Running, Start/F5 considers only marked retry-eligible routes and wakes the live scheduler after committing an accepted retry.
- In Select and Stopped, ordinary marked `not queued` work retains priority. If no ordinary target is startable, Start/F5 falls back to marked retry-eligible routes instead of rejecting solely because their display status is `error`, `stalled`, or resumable external `blocked`.
- In process-wide Error, retain the existing marked retry behavior.
- In Stopping, retain mutation-free refusal.
- Reuse existing retry planning and commit paths so terminal errors use `ReducerCommand::RetryError`, target-specific explicit-retry edges release scheduler failed classification, acceptance holds resume through their existing route, and active-run Apply iteration limits remain authoritative.
- Keep ordinary Start and retry admission separate within one request. A request with at least one ordinary startable target does not implicitly retry additional marked error/wait rows; those rows are reported as excluded with target-specific status.
- Preserve the prepared-command transaction: all fallible admission and scheduler preparation precede reducer/mark/edge effects, the accepted outcome is dispatched before scheduler activation, and TUI plus `/api/v2` receive identical results.

## Acceptance Criteria

1. A marked retry-eligible change-level `error` in persistent-idle Select is accepted by F5, transitions through the existing explicit retry reducer route, wakes the scheduler, and reaches a distinct dependency-analysis attempt without waiting for mark settlement.
2. A marked retry-eligible change-level `error` can be retried with F5 while process mode remains Running; unrelated active or queued work and process mode are not converted to global Error.
3. A retry accepted from Stopped starts a fresh explicit-retry scheduler boundary; an accepted retry from a live scheduler notifies that scheduler instead of spawning a second boundary.
4. A runtime-limit failure is not automatically retried, but a later explicit F5 request is eligible once ordinary shared retry guards permit it.
5. Active-run Apply iteration-limit evidence, non-resumable or identity-mismatched holds, unsupported terminal states, and Stopping mode produce mutation-free refusal or target-specific exclusion with no reducer, mark, queue, retry-edge, scheduler, or projection effect.
6. In Select or Stopped, ordinary marked `not queued` work keeps existing Start semantics. If ordinary and retry-only marks are mixed, ordinary work is admitted and retry-only rows are excluded rather than implicitly retried.
7. TUI F5 and remote `start` resolve the same targets, reducer transitions, explicit-retry semantics, scheduler effect, outcome revision, and diagnostics from the same authoritative snapshot.
8. Explicit F5 retry remains separate from the 10-second ordinary mark-settlement path: error/wait marks do not gain delayed implicit retry, and an accepted F5 retry does not arm or wait for that deadline.

## Explicit Completion Conditions

- `src/orchestration/run_control.rs` performs mode-aware Start admission that can plan retry routes in Running and can fall back to retry routes in Select/Stopped only when no ordinary target is startable.
- The implementation reuses `OperatorCommandService::plan_retry_errors` and `commit_retry_routes`; it does not clear terminal error through ordinary `AddToQueue` or generic queue notification.
- Prepared command commit and activation preserve the current fail-atomic ordering across reducer mutation, explicit-retry publication, authoritative outcome dispatch, and scheduler wake/start.
- Focused cross-adapter tests prove TUI and remote Start parity for persistent-idle Select, Running, Stopped, process-wide Error, mixed marks, Stopping, active iteration limit, and unsupported evidence.
- A scheduler component test proves an accepted retry consumes the target-specific edge and emits `AnalysisStarted`; absence of the edge or a no-op implementation fails the test.
- `make test-change-error-f5-retry` fails when no `change_error_f5_retry_*` tests are discovered, then runs the focused lib-target tests without heavy tests or short wall-clock correctness thresholds.

## Scope Rationale

Start admission, retry mutation, scheduler wake/start, and frontend parity remain one proposal because partial delivery would either continue rejecting F5 or clear error evidence without creating the scheduler edge required for safe reanalysis.

## Out of Scope

- Reverting change-local `ProcessingError` to process-wide Error.
- Automatically retrying runtime-limit or other failed invocations without explicit operator intent.
- Treating Space, bulk `x`, mark settlement, or ordinary queue addition as retry commands.
- Changing retry eligibility, Apply iteration limits, acceptance-hold identity validation, dependency analyzer output, or dispatch ordering.
- Combining ordinary Start targets and retry routes into one mixed explicit-retry scheduler launch.
- Adding a new key, API command, durable workflow state, or frontend-specific retry implementation.

The tracked Rust hooks are path-scoped, so proposal-only commit creation does not run Rust validation. Requirement-specific focused tests remain explicit implementation evidence.
