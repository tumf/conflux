---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/show-ready-header-after-stop/
  - src/events.rs
  - src/parallel/orchestration.rs
  - src/tui/state/event_handlers/processing.rs
  - src/tui/lifecycle.rs
  - src/web/state.rs
verifications:
  - id: persistent-idle-ready-regressions
    requirement: "A persistent scheduler that parks with no executable work projects Ready consistently to TUI, Web, API, and lifecycle observers, remains alive for explicit wake, and returns to Running only when typed admitted work actually starts"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering edge-triggered persistent-idle emission, fully-drained and blocked/waiting-only Ready projection, terminal-mode preservation, coherent API revisions, idle lifecycle output, and typed admitted-work resume"
    rerun: "cargo test --lib persistent_idle_event_is_edge_triggered -- --list | grep -q persistent_idle_event_is_edge_triggered && cargo test --lib persistent_idle_event_is_edge_triggered && cargo test --lib persistent_idle_projects_ready_without_completion -- --list | grep -q persistent_idle_projects_ready_without_completion && cargo test --lib persistent_idle_projects_ready_without_completion && cargo test --lib persistent_idle_projects_api_ready_once -- --list | grep -q persistent_idle_projects_api_ready_once && cargo test --lib persistent_idle_projects_api_ready_once && cargo test --lib admitted_work_restores_running_after_idle -- --list | grep -q admitted_work_restores_running_after_idle && cargo test --lib admitted_work_restores_running_after_idle && cargo test --lib persistent_idle_lifecycle_is_idle -- --list | grep -q persistent_idle_lifecycle_is_idle && cargo test --lib persistent_idle_lifecycle_is_idle && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
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
- project frontend `Running` to Ready/`select` without changing reducer status, blocker metadata, queue intent, worktree evidence, diagnostics, or execution marks;
- preserve `Error`, `Stopping`, and `Stopped` instead of overwriting them;
- project semantic external lifecycle `idle`;
- leave the persistent scheduler alive and waiting for its existing explicit wake sources.

When work is later admitted, the first existing typed event that proves actual execution has started, including workspace preparation or a base-lane operation, SHALL return TUI and Web to `Running`. A queue notification or analysis attempt alone SHALL NOT claim Running.

## Split Rationale

This proposal is independent from `synchronize-execution-marks`. Persistent-idle projection changes scheduler/frontend execution presentation but does not change mark ownership. Mark synchronization can ship without changing scheduler lifetime or idle behavior. Neither proposal consumes repository output from the other, so both may be implemented in parallel with no hard dependency.

## Acceptance Criteria

1. A persistent scheduler emits one typed idle transition before parking after coherent fully-drained detection.
2. Stable dependency-blocked, acceptance/external stalled, resolve-wait, or reject-wait-only persistent states also project Ready while retaining their row status and blocker/wait evidence.
3. Repeated idle-loop evaluation and notifications that admit no work do not emit duplicate idle transitions or advance `/api/v2` state revision repeatedly.
4. TUI changes from `AppExecutionMode::Running` to `Select`; Web and `/api/v2` change `app_mode` from `running` to `select` at the same dispatch.
5. The idle transition does not overwrite TUI/Web `error`, `stopping`, or `stopped` modes and does not emit completion success messaging.
6. The scheduler remains alive, non-polling, and responsive to the existing dynamic queue, scheduler retry, merge-result, and cancellation wake sources.
7. Queue notification or dependency analysis without admitted execution leaves the frontend Ready.
8. The first typed admitted-work start after idle changes TUI and Web back to `Running`; this includes ordinary workspace preparation and scheduler-owned resolve/rejection-review work.
9. External lifecycle projection reports `idle` for Ready even when blocked/stalled rows remain visible, and reports `working` only after admitted work starts.
10. Reducer display status, queue intent, blocker metadata, worktree state, diagnostics, and execution marks are identical before and after the idle-only transition.

## Explicit Completion Conditions

- `src/events.rs` contains an exhaustively classified typed persistent-idle event with remote event vocabulary and semantic lifecycle mapping.
- `src/parallel/orchestration.rs` emits that event only from the coherent persistent-idle admission path and has an idle-episode latch that rearms only after admitted work begins.
- TUI and Web consume the same event and apply a guarded Running-to-Ready transition without invoking `handle_all_completed`, `try_transition_to_select`, or another success-completion helper.
- Existing typed admitted-work events restore Running consistently after idle without treating notification or analysis as execution.
- Unit/integration tests cover fully drained, blocked/waiting-only, duplicate idle, no-op wake, retained terminal modes, unchanged row/mark facts, API revision behavior, lifecycle projection, and ordinary/base-lane resume.
- The commands declared by `persistent-idle-ready-regressions` pass.

## Out of Scope

- Terminating or recreating the persistent scheduler.
- Changing finite scheduler completion, `SchedulerRunReport`, or `AllCompleted` semantics.
- Adding timer-driven repository/worktree polling while idle.
- Changing mixed Start, target-scoped retry, or `RetryPlan.explicit_retry` behavior.
- Clearing or otherwise changing execution marks.
- Changing reducer blocker, wait, queue, worktree, diagnostic, or resume-routing authority.
- Replacing internal `Stopped` presentation covered by `show-ready-header-after-stop`.
