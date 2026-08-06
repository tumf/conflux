---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/apply-commit-recovery/spec.md
  - openspec/specs/process-execution/spec.md
  - openspec/changes/archive/fix-apply-completion-hang/
  - openspec/changes/archive/2026-08-03-retry-apply-after-commit-hook-failure/
  - openspec/changes/archive/2026-08-06-harden-apply-finalization/
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/command_queue.rs
verifications:
  - id: apply-repair-completion-regressions
    requirement: "Apply completion grace terminates completion reached by the active dispatch without terminating task-complete repair work that was already required when that dispatch began"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering stage, task-format, and final-commit-hook repair commands that outlive a shortened completion grace; ordinary task-completion transition; transient completion; blocked and rejecting handoffs; and process-group cleanup compatibility"
    rerun: "cargo test --lib precomplete_apply_repair && cargo test --lib test_execute_apply_loop_terminates_lingering_child_after_tasks_complete && cargo test --lib test_execute_apply_loop_keeps_child_running_when_tasks_become_incomplete_during_grace && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix pre-complete Apply repair termination

**Change Type**: implementation

## Premise / Context

- The Apply completion watchdog was introduced to terminate a command that reaches a repository completion condition but leaves its output pipes or process group alive.
- Finalization recovery intentionally dispatches another Apply command after tasks are already complete when staging, task format, or the hook-enabled final commit still requires repair.
- The current watchdog does not distinguish completion reached by the active dispatch from task completion that existed before the dispatch. It therefore starts the same grace timer as soon as a repair command begins and can terminate that repair before it stages files, corrects `tasks.md`, or fixes repository-hook failures.
- The failure repeats through the existing Apply iteration budget. The same pattern has been observed across more than one change, so it is a shared orchestration defect rather than a one-off agent delay.
- Workspace-local progress remains authoritative. A dispatch-start snapshot is permitted ephemeral state under `openspec/CONSTITUTION.md` and is discarded on restart.

## Problem / Context

`execute_apply_loop` correctly bypasses its task-complete loop-entry short circuit when finalization repair is required. Once the repair child starts, however, streaming completion detection calls `detect_apply_completion`, which sees the already-complete `tasks.md` and reports `TasksComplete`. After the bounded grace period, the stable recheck sees the same pre-existing condition and terminates the owned process group.

This makes the loop's routing and child-lifetime contracts disagree: routing says a repair command must run, while the watchdog treats the reason that repair was needed as proof that the new command is finished. Stage repair and final-commit-hook repair carry process-local pending state, while task-format repair is derived directly from workspace state, so special-casing one repair flag would leave the other paths vulnerable and would not cover future task-complete repairs.

The fix must preserve the original watchdog behavior for a normal Apply dispatch that starts incomplete and reaches task completion while still running. It must also preserve blocked and rejecting handoff detection, cancellation, command-queue retry and inactivity behavior, and the process-group quiescence barrier before repository finalization.

## Proposed Solution

1. Derive a dispatch-local task-completion eligibility value from the task progress already read before the Apply command is launched.
2. Arm `TasksComplete` grace termination only when that dispatch began with incomplete task progress. A transition to complete during the active command remains eligible for the existing grace and stable-recheck behavior.
3. When a dispatch begins with task progress already complete, do not let that pre-existing task completion arm, refresh, or finalize the grace timer for that dispatch. Let stage repair, task-format repair, and final-commit-hook repair follow the normal command lifecycle.
4. Keep `BlockedHandoff` and `RejectingHandoff` eligible completion conditions for every dispatch, including a task-complete repair dispatch. Their stable appearance may still trigger bounded process-group termination and handoff.
5. Apply the same dispatch-local eligibility to both the initial streaming probes and the deadline's stable repository recheck so a disabled `TasksComplete` condition cannot reappear only at grace expiry.
6. Keep the policy ephemeral and generic. Do not add durable workflow state, a repair-kind hierarchy, a new timeout, or special handling in the command runner.

## Atomic Scope Rationale

The dispatch-local completion rule, its canonical requirement update, and regression coverage define one child-lifetime contract. Splitting them would either change termination behavior without a reviewable specification or publish a requirement that the runtime still violates. The unrelated long-running test failures and global command-duration policy can be addressed independently and are excluded.

## Acceptance Criteria

1. A normal Apply dispatch that begins with incomplete tasks and reaches stable task completion while its child remains alive still starts the completion grace, terminates the owned process group after grace, confirms quiescence, and continues finalization.
2. If task completion becomes transiently absent or changes before the grace deadline, the existing stable-recheck behavior continues the child rather than terminating from stale evidence.
3. A stage-repair dispatch that begins with tasks complete can run longer than the configured completion grace, stage the intended files, exit naturally, and reach the verified final commit without an extra repair iteration.
4. A task-format-repair dispatch that begins with complete checkboxes can run longer than the configured completion grace, correct the malformed task content, exit naturally, and proceed without consuming an Acceptance attempt.
5. A final-commit-hook-repair dispatch that begins with tasks complete can run longer than the configured completion grace, repair the repository rejection, exit naturally, and allow the hook-enabled final commit retry to succeed.
6. A task-complete repair dispatch that creates `APPLY_BLOCKED` still reaches blocked handoff through the existing grace and process-group cleanup path.
7. A task-complete repair dispatch that creates `REJECTED.md` still reaches rejecting handoff through the existing grace and process-group cleanup path.
8. Task completion existing before dispatch does not become success-equivalent evidence for a non-zero repair command exit. Natural failure, permission, retry, stall, and iteration-budget classification remain authoritative when no newly eligible handoff is present.
9. The process-group cleanup barrier still gates every repository-mutating finalization or handoff after grace-driven termination.
10. Restart routing remains derived from workspace files and Git state; no out-of-worktree durable state or new generated checkpoint is introduced.
11. Added default-suite regression tests use sub-second intentional waits and complete in under one second each unless an existing platform boundary makes that impractical, in which case they follow the repository heavy-test policy.

## Explicit Completion Conditions

- `src/execution/apply.rs` computes task-completion grace eligibility once per dispatched Apply command from the pre-dispatch task-progress observation.
- Both completion probes and deadline rechecks use the same eligibility while continuing to detect blocked and rejecting handoffs.
- No changes are required in `src/ai_command_runner.rs`, `src/command_queue.rs`, process-group cleanup semantics, persisted workflow state, configuration schema, or frontend-specific code.
- Integration regressions fail against the pre-fix behavior by delaying each repair action beyond a shortened test grace, then pass only when the repair command remains alive long enough to complete.
- Regression coverage proves all three pre-complete repair paths, ordinary incomplete-to-complete termination, transient completion recheck, blocked handoff, rejecting handoff, and cleanup compatibility.
- `cargo test --lib precomplete_apply_repair`, the two named existing completion-grace tests, `cargo fmt --check`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` pass.

## Out of Scope

- Adding a hard wall-clock timeout for Apply commands or changing inactivity timeout reset semantics.
- Fixing unrelated long-running, deadlocked, flaky, or current-directory-sensitive repository tests.
- Optimizing agent polling, tool-call volume, compilation duration, or verification strategy.
- Changing cancellation, command-queue transport retries, Apply iteration budgets, stall policy, or permission-denial handling.
- Weakening or bypassing process-group cleanup and quiescence confirmation.
- Changing WIP snapshots, explicit staging ownership, final-commit hook verification, or cleanup-review behavior.
- Consolidating the duplicated canonical `Managed worktree apply MUST run post-apply cleanup review before acceptance handoff` heading; promotion currently applies a same-name modification to every occurrence, so that cleanup-spec hygiene requires separate treatment.
- Changing path-scoped pre-commit hook behavior for clean-tree amend commits.
