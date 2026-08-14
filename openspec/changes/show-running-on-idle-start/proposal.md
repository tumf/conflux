---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/changes/archive/2026-08-07-restore-ready-on-persistent-idle/
  - src/orchestration/operator_coordinator.rs
  - src/orchestration/run_control.rs
  - src/events.rs
  - src/parallel/orchestration.rs
  - src/tui/command_handlers.rs
  - src/tui/state/event_handlers/operator_commands.rs
  - src/web/state.rs
verifications:
  - id: idle-start-running-regressions
    requirement: "An accepted Start from persistent-idle Ready projects Running immediately and coherently, preserves truthful active-work observation, and returns to Ready when the scheduler parks without admitted work"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering accepted and refused Start, TUI/Core/Web projection parity, queue-status projection, scheduler reuse, idle-edge rearming, no-work re-park, external lifecycle output, and execution-status activity separation"
    rerun: "make test-idle-start-running"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show Running immediately after persistent-idle Start

**Change Type**: implementation

## Problem / Context

After a persistent scheduler finishes its current work, Conflux intentionally keeps that scheduler alive and projects the TUI as Ready. When the operator marks another eligible change and presses F5, shared run control accepts Start, adds reducer queue intent, and wakes the existing scheduler.

The accepted command does not currently project Running because `RunDispatched { scheduler_started: false }` is treated as notification-only. TUI, Core, and Web wait for a later typed admitted-work event such as `WorkspacePreparationStarted`. Dependency analysis runs between those two events, so the TUI remains Ready for several seconds after F5 even though the requested Start has already been accepted and the target visibly belongs to the run.

The delay is not input latency or the five-second catalog refresh. It is required by the current persistent-idle contract, which intentionally keeps Ready visible until workspace or base-lane work begins. That contract now conflicts with the operator expectation that a successfully accepted F5 command receives immediate lifecycle feedback.

## Proposed Solution

Treat an accepted Start that wakes a persistent-idle scheduler as the beginning of an operator-visible run episode.

The authoritative `OperatorCommandApplied::RunDispatched` outcome SHALL project Running immediately when all of the following are true:

- the frontend/Core currently represents a persistent-idle Ready episode;
- shared run control accepted at least one target;
- reducer queue or explicit-retry intent was committed; and
- the existing scheduler was notified rather than replaced.

The same dispatch SHALL project admitted targets as queued and clear the persistent-idle presentation fact. Raw F5 input SHALL remain intent-only: a refused Start, an empty target set, or a command no-op MUST remain Ready and MUST NOT create optimistic Running state.

The persistent scheduler SHALL rearm its idle-edge latch when it reconciles the newly committed queue or explicit-retry intent. If dependency analysis or eligibility classification produces no admitted execution and the scheduler parks again, it SHALL emit a fresh `PersistentSchedulerIdle` transition and return all frontends to Ready. A generic wake with no accepted Start or queue addition SHALL not rearm the edge and SHALL not cause Ready/Running churn.

TUI, Core, Web, `/api/v2`, and external lifecycle projection SHALL consume the same accepted outcome and converge on the same operator-visible mode. This presentation change SHALL NOT redefine actual work evidence: `GET /api/v2/execution-status` MUST continue deriving `has_active_work` and typed phases from dependency-analysis or lifecycle events, not from `app_mode`, Start acceptance, queue intent, or execution marks alone.

## Split Rationale

This remains one proposal because command outcome projection, scheduler idle-edge rearming, TUI/Web mode convergence, API observation, and external lifecycle output form one state transition. Splitting them could temporarily leave one frontend Running while another remains Ready, or leave a no-work scheduler unable to emit the Ready transition that closes the episode.

## Acceptance Criteria

1. From persistent-idle Ready, pressing F5 with at least one eligible marked target changes the TUI header to Running on the next render frame after Start acceptance, without waiting for dependency analysis or workspace preparation.
2. The accepted outcome's authoritative revision projects Core, TUI, Web, and `/api/v2` as Running and projects each admitted target as queued.
3. The existing persistent scheduler is notified and no second scheduler task is spawned.
4. The raw key handler does not set Running. A targetless, refused, stale, or no-op Start remains Ready and surfaces the existing warning or refusal feedback.
5. Accepted explicit retry from persistent-idle Ready receives the same immediate Running projection without changing retry routing or fresh Apply-budget rules.
6. Reconciliation of the accepted queue or explicit-retry intent rearms the persistent-idle edge. If no work is admitted and the scheduler parks, one new typed idle event returns TUI and Web to Ready.
7. Generic notifications, duplicate wakeups, catalog refresh, and analysis without accepted queue additions do not independently project Running or emit duplicate idle transitions.
8. `persistent_scheduler_idle` is false after the accepted Start projection and becomes true again only when a subsequent typed persistent-idle transition projects Ready.
9. External lifecycle output changes from `idle` to `working` for the accepted Start outcome and remains deduplicated across unchanged frames.
10. `/api/v2/execution-status` keeps `scheduler_running` distinct from `has_active_work`: Start acceptance or `app_mode: running` alone does not certify an active phase, while typed dependency-analysis or lifecycle evidence does.
11. Initial Start, Running-mode retry, graceful stop, cancel-stop, force stop, terminal Error/Stopped retention, and finite scheduler completion preserve their existing behavior.
12. Focused tests use event ordering and state transitions rather than short wall-clock thresholds and complete within the repository's default fast-test policy.

## Explicit Completion Conditions

- `CoreMode`, TUI `AppState`, and Web state apply the accepted persistent-idle Start outcome through the authoritative command dispatch and produce one convergent Running projection.
- Existing contradictory tests that require Ready to remain visible after an accepted persistent-idle Start are replaced with assertions for immediate Running, while no-op wake coverage continues to require Ready.
- The persistent scheduler rearms its idle latch only after observing committed queue or explicit-retry additions, allowing a no-work evaluation to publish a new Ready edge without letting generic wakes flicker the mode.
- Row queue status at the accepted outcome comes from reducer state, not a frontend-only optimistic write.
- Execution-status tests prove that mode feedback and actual active-work evidence remain separate.
- `Makefile` provides a discovery-guarded `test-idle-start-running` target, and `make test-idle-start-running` passes.
- Strict OpenSpec validation and archive-gate validation pass for this change.

## Out of Scope

- Starting a second scheduler for persistent-idle resume.
- Making dependency analysis faster or changing its model, prompt, debounce, or ordering policy.
- Treating raw key input as successful command acceptance.
- Treating `app_mode: running`, queue intent, or execution marks as proof of active lifecycle work.
- Changing persistent scheduler lifetime, finite completion, `AllCompleted`, stop settlement, or workspace-derived resume routing.
- Adding durable workflow state or changing the Constitution.
