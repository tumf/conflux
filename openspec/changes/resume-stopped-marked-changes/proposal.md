---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/orchestration/state.rs
  - src/orchestration/mark_settlement.rs
  - src/orchestration/run_control.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/operator_coordinator.rs
  - src/web/remote_control_api
verifications:
  - id: stopped-marked-resume-regression
    requirement: A stopped ordinary change whose execution mark was preserved can be explicitly resumed without restarting the owner, and the resume produces one dependency-analysis edge.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: change-acceptance
    automation: src/orchestration/run_control/tests/change_error_f5_retry.rs
    evidence: Focused `stopped_marked_resume_*` Rust tests, initially placed in the existing run-control test module and movable to a dedicated module in the same change, exercise the shared TUI/API Start transaction from a stopped process with preserved marks, prove terminal evidence is cleared only by explicit resume, and observe queue admission plus one scheduler boundary and dependency analysis.
    rerun: cargo test --locked stopped_marked_resume
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Resume stopped marked changes without owner restart

**Change Type**: implementation

## Problem / Context

A live Conflux v0.6.310 owner force-stopped two changes and preserved their execution marks. The coherent owner snapshot then reported each target as:

- `execution_marked: true`
- `queue_intent: not_queued`
- `display_status: stopped`
- no blocker and parallel eligibility true

The remote mark command succeeded because the marks already had the requested value. A subsequent remote Start failed with `target_ineligible`: both marked targets were excluded as `stopped`. Delayed mark settlement also intentionally excludes `stopped` as terminal. The owner therefore had no transition that could satisfy the canonical Interrupted Change Handling contract that preserved marks are restored to queued and reprocessed on resume. Restarting the owner erased the process-local stopped terminal state and made the same mark plus Start work, but owner restart is an operational workaround rather than the declared resume behavior.

This is distinct from `fix-running-mark-settlement-admission`. That change repaired a live Running owner whose ordinary target was temporarily `NotLoadable`. Here the target is loadable and intentionally classified `Terminal(Stopped)`; retrying settlement cannot resolve it.

## Proposed Solution

Add one explicit resume transition to the shared Start transaction for process mode `Stopped`.

- When Start is explicitly requested in `Stopped`, classify preserved marked targets whose only terminal state is operator `Stopped` as ordinary resumable work.
- Atomically clear only the stop-owned terminal/runtime residue required to return those targets to ordinary queue admission, preserve their execution marks, admit them through the existing queue/scheduler path, and start one fresh scheduler boundary.
- Keep delayed mark settlement unchanged: mark or re-mark alone must not resume stopped work or synthesize Start.
- Do not route stopped work through terminal-error retry. Preserve Error, rejected, archived, merged, pushed, blocked/stalled, acceptance-hold, and unsupported terminal evidence under their existing explicit routes.
- Use the same `OperatorApplication` transaction for TUI F5 and remote `/api/v2` Start so validation, reducer effects, revision, outcome, and scheduler activation stay equivalent and fail-atomic. The existing `remote-control-api` requirement already mandates parity and the complete-request worktree fence, so this change needs no separate remote API delta.
- Make the successful outcome identify the resumed targets and exclusions. A failed preparation leaves stopped state, marks, queue intent, process mode, and scheduler unchanged.

## Acceptance Criteria

1. Given process mode `Stopped` and an ordinary target with `execution_marked=true`, `queue_intent=not_queued`, `display_status=stopped`, and no blocker, explicit TUI or remote Start resumes it without owner restart.
2. The accepted command preserves the execution mark, clears only stop-owned terminal state, admits the target through ordinary queue semantics, starts exactly one fresh scheduler boundary, and produces a new dependency-analysis attempt.
3. Mark, bulk mark, re-mark, and the delayed mark-settlement deadline alone do not resume a stopped target and do not wake or start a scheduler.
4. Select and Running behavior is unchanged. Running ordinary marks still use delayed settlement, while retry-eligible Error and resumable holds retain their explicit retry routes.
5. A worktree-ineligible marked target rejects the complete request before class selection with no mutation. Otherwise, mixed marks in Stopped admit only ordinary stopped/not-queued targets; Error, rejected, archived, merged, pushed, blocked/stalled, or otherwise unsupported targets retain evidence and receive target-specific exclusions without mutation.
6. Scheduler preparation failure is fail-atomic: stopped terminal state, marks, queue intent, process mode, hooks, revision-visible effects, and scheduler counts equal their pre-command values.
7. TUI F5 and remote `/api/v2` Start produce equivalent targets, exclusions, reducer transitions, scheduler effect, result revision, and diagnostics.
8. `cargo test --locked stopped_marked_resume` exits 0 and fails against the current v0.6.310 behavior.

## Explicit Completion Conditions

- A production-equivalent regression first reproduces `marked + not_queued + stopped`, demonstrates current Start refusal, and turns green only after the shared resume transition exists.
- A scheduler-level assertion observes a fresh `AnalysisStarted` or equivalent dependency-analysis edge, not merely `queued` display state.
- The implementation does not reset the whole owner, rebuild the catalog, delete worktrees, or depend on owner restart.
- No frontend directly mutates runtime terminal state or queue intent.
- Existing terminal-error and running-mark regression suites remain green.

## Out of Scope

- Automatically resuming from mark changes or timer settlement.
- Treating terminal Error as ordinary queue work.
- Changing force-stop cancellation or worktree preservation policy.
- Durable persistence of process-local marks or runtime state.
- Restarting active owners as part of normal resume.
