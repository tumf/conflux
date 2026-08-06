---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/show-ready-header-after-stop/
  - src/events.rs
  - src/parallel/orchestration.rs
  - src/orchestration/run_control.rs
  - src/tui/command_handlers.rs
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/state/event_handlers/processing.rs
  - src/tui/lifecycle.rs
  - src/web/state.rs
  - src/web/remote_control_api/dto.rs
  - src/web/remote_control_api/projection.rs
  - web/app.js
  - tests/web/operator-console.spec.js
verifications:
  - id: persistent-idle-ready-regressions
    requirement: "A persistent scheduler that parks with no executable work projects Ready plus an explicit process-local idle-episode fact consistently to TUI, Web, API, and lifecycle observers, remains command-addressable, and returns to Running only when typed admitted work actually starts"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering edge-triggered persistent-idle emission, fully-drained and blocked/waiting-only Ready projection, process-local idle-episode state, TUI/Web live-scheduler controls, Start/stop/cancel-stop admission, terminal-mode preservation, coherent API revisions, guarded idle lifecycle output, and typed admitted-work resume"
    rerun: "cargo test persistent_idle_event_is_edge_triggered -- --list | grep -q persistent_idle_event_is_edge_triggered && cargo test persistent_idle_event_is_edge_triggered && cargo test persistent_idle_projects_ready_without_completion -- --list | grep -q persistent_idle_projects_ready_without_completion && cargo test persistent_idle_projects_ready_without_completion && cargo test persistent_idle_commands_use_live_scheduler -- --list | grep -q persistent_idle_commands_use_live_scheduler && cargo test persistent_idle_commands_use_live_scheduler && cargo test --features web-monitoring persistent_idle_projects_api_ready_once -- --list | grep -q persistent_idle_projects_api_ready_once && cargo test --features web-monitoring persistent_idle_projects_api_ready_once && cargo test admitted_work_restores_running_after_idle -- --list | grep -q admitted_work_restores_running_after_idle && cargo test admitted_work_restores_running_after_idle && cargo test persistent_idle_lifecycle_is_idle -- --list | grep -q persistent_idle_lifecycle_is_idle && cargo test persistent_idle_lifecycle_is_idle && make web-test && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Restore Ready on persistent idle

**Change Type**: implementation

## Problem / Context

A persistent parallel scheduler intentionally remains alive after automatic work is fully drained or reduced to a stable blocked/waiting-only state. It parks in an event-driven wait for dynamic queue or scheduler retry notifications instead of terminating.

That park currently emits no typed execution event. TUI and Web therefore retain `Running` even though no agent, workspace preparation, resolve, rejection review, merge, or push is executing. The scheduler is correctly alive, but the frontend presentation is not truthful.

`AllCompleted` is not an appropriate substitute: it carries successful terminal semantics, resets presentation state, and is coupled to finite completion. A persistent idle park is resumable and must not claim completion.

## Proposed Solution

Add one typed persistent-scheduler idle transition emitted through the existing authoritative event dispatcher immediately before the scheduler enters its event-driven idle wait.

The transition SHALL:

- be emitted for coherent fully-drained and stable blocked/waiting-only persistent idle states;
- be edge-triggered once per continuous idle episode and remain suppressed across duplicate evaluations or wake notifications that start no admitted work;
- project frontend `Running` to Ready/`select` and expose a process-local `persistent_scheduler_idle` idle-episode fact without changing reducer status, blocker metadata, queue intent, worktree evidence, diagnostics, or execution marks;
- preserve `Error`, `Stopping`, and `Stopped` instead of overwriting them;
- project semantic external lifecycle `idle`;
- leave the persistent scheduler alive, command-addressable, and waiting for its existing explicit wake sources.

Ready is a presentation of a live persistent scheduler, not a new stopped run. TUI and `/api/v2` SHALL expose the process-local `persistent_scheduler_idle` idle-episode fact so operator controls can distinguish this state from pre-run Select without treating the fact as workflow authority; shared run control SHALL still revalidate scheduler liveness through `is_running()`. The fact becomes true only when the typed idle event performs its guarded Running-to-Ready transition, remains true through Start notification and an idle-origin graceful-stop request, and becomes false when typed admitted work begins or the scheduler reaches Error or Stopped. A late idle event MUST NOT turn pre-run Select or a terminal mode into persistent idle. Operator marks in this state remain mark-only. Start SHALL resolve the authoritative marked target set, add its existing queue intent, and wake the same scheduler without spawning a second task. Web SHALL expose Start, graceful stop, and force stop directly; TUI SHALL expose Start plus its existing first-Esc graceful-stop and second-Esc force-stop progression. Cancel-stop remains valid only when a graceful stop is pending and SHALL restore Ready rather than Running when this fact remains true.

A persistent-idle Start returns `SchedulerEffect::Notified` and SHALL leave TUI and Web Ready until an existing typed event proves actual execution has started, including workspace preparation or a base-lane operation. That event clears `persistent_scheduler_idle`; it changes Select to Running but preserves Stopping when a graceful-stop request won the race. Cancel-stop then restores Ready only if the idle fact is still true, otherwise Running. The existing synchronous `begin_run` projection remains unchanged only when dispatch returns `SchedulerEffect::Started` for a newly spawned scheduler outside this idle-resume path. A Start notification, bare queue notification, analysis attempt, or no-op wake SHALL NOT claim Running.

## Split Rationale

This proposal is independent from `synchronize-execution-marks`. Persistent-idle projection changes scheduler/frontend execution presentation but does not change mark ownership. Mark synchronization can ship without changing scheduler lifetime or idle behavior. Neither proposal consumes repository output from the other, so both may be implemented in parallel with no hard dependency.

## Acceptance Criteria

1. A persistent scheduler emits one typed idle transition before parking after coherent fully-drained detection.
2. Stable dependency-blocked, acceptance/external stalled, resolve-wait, or reject-wait-only persistent states also project Ready while retaining their row status and blocker/wait evidence.
3. Repeated idle-loop evaluation and notifications that admit no work do not emit duplicate idle transitions or advance `/api/v2` state revision repeatedly.
4. TUI changes from `AppExecutionMode::Running` to `Select`; Web and `/api/v2` change `app_mode` from `running` to `select` and expose `persistent_scheduler_idle: true` at the same dispatch.
5. The idle transition does not overwrite TUI/Web `error`, `stopping`, or `stopped` modes and does not emit completion success messaging.
6. The scheduler remains alive, non-polling, and responsive to the existing dynamic queue, scheduler retry, merge-result, and cancellation wake sources.
7. TUI and Web render Ready/Idle while retaining live-scheduler controls for `persistent_scheduler_idle`: TUI shows Start plus its existing first-Esc graceful-stop hint and second-Esc force-stop progression, while Web shows Start, graceful stop, and force stop directly. Pre-run Select continues to expose only Start. Shared run control revalidates actual scheduler liveness before every command.
8. Marks made while persistent-idle Ready remain mark-only. Accepted Start resolves those marks, adds existing reducer queue intent, and notifies the same live scheduler without spawning a second scheduler task.
9. Accepted graceful stop and force stop address the live scheduler while it is presented as Ready; graceful stop wakes the idle wait so the scheduler can reach its existing stop boundary.
10. `persistent_scheduler_idle` defaults false, remains true through a Start notification and an idle-origin graceful-stop request, and resets on process restart. Cancel-stop restores Ready while it remains true; admitted work, Error, or Stopped clears it.
11. Start notification, queue notification, or dependency analysis without typed admitted execution leaves TUI and Web Ready; the first typed admitted-work start changes both to `Running`, including ordinary workspace preparation and scheduler-owned resolve/rejection-review work.
12. External lifecycle projection reports `idle` for Ready even when blocked/stalled rows remain visible, and reports `working` only after admitted work starts.
13. Reducer display status, queue intent, blocker metadata, worktree state, diagnostics, and execution marks are identical before and after the idle-only transition.
14. The persistent-idle event, `persistent_scheduler_idle: true`, and its `app_mode: select` snapshot share the one authoritative `/api/v2` revision defined by `remote-control-api`; duplicate/no-op idle observation creates no new revision.

## Explicit Completion Conditions

- `src/events.rs` contains an exhaustively classified typed persistent-idle event with remote event vocabulary and semantic lifecycle mapping.
- `src/parallel/orchestration.rs` emits that event only from the coherent persistent-idle admission path and has an idle-episode latch that rearms only after admitted work begins.
- TUI and Web consume the same event and apply a guarded Running-to-Ready transition with `persistent_scheduler_idle: true` without invoking `handle_all_completed`, `try_transition_to_select`, or another success-completion helper.
- Existing typed admitted-work events restore Running and clear the idle fact consistently after idle without treating notification or analysis as execution; only a newly spawned scheduler's existing `SchedulerEffect::Started` path retains synchronous Running projection.
- TUI and Web use the idle fact to expose Start and stop controls while shared run control independently validates scheduler liveness; idle-origin cancel-stop restores Ready.
- Generated OpenAPI includes the process-local idle field, and replay-gap state replacement restores control visibility without event replay.
- Unit/integration/browser tests cover fully drained, blocked/waiting-only, duplicate idle, no-op wake, live-scheduler Start/stop/cancel-stop commands, TUI Esc and Web control visibility, retained terminal modes, unchanged row/mark facts, API revision behavior, lifecycle projection, and ordinary/base-lane resume.
- The commands declared by `persistent-idle-ready-regressions` pass.

## Out of Scope

- Terminating or recreating the persistent scheduler.
- Changing finite scheduler completion, `SchedulerRunReport`, or `AllCompleted` semantics.
- Adding timer-driven repository/worktree polling while idle.
- Changing mixed Start, target-scoped retry, or `RetryPlan.explicit_retry` behavior beyond preserving existing Start admission against the live persistent scheduler.
- Clearing or otherwise changing execution marks.
- Changing reducer blocker, wait, queue, worktree, diagnostic, or resume-routing authority.
- Redesigning general operator modes; the only command change is recognizing persistent-idle Ready as a live scheduler for existing Start and stop-family behavior.
- Replacing internal `Stopped` presentation covered by `show-ready-header-after-stop`.
