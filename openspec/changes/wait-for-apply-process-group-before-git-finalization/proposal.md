---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/process-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/process_manager.rs
  - tests/process_cleanup_test.rs
verifications:
  - id: apply-process-group-barrier-tests
    requirement: Unix Apply completion cannot enter repository finalization until leader reaping and process-group absence are both confirmed
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Deterministic Rust unit tests for the probe/deadline/outcome matrix plus heavy Unix process tests covering graceful exit, forced termination, descendant-held Git lock release, cleanup timeout, and acceptance suppression
    rerun: cargo test --lib process_group_cleanup && cargo test --lib apply_process_group_barrier && cargo test --features heavy-tests --test process_cleanup_test apply_completion
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Wait for Apply process-group cleanup before Git finalization

**Change Type**: hybrid

## Problem / Context

A production run confirmed an unsafe ordering within one Conflux-owned Apply lifecycle: after the 30-second completion grace period, Conflux sent SIGTERM to the Apply process group and began WIP/final commit processing about 370 milliseconds later; the final `git commit --amend` then failed because the managed worktree `index.lock` existed. The repository-level singleton lock was working correctly.

The current Unix cleanup path waits for the process-group leader. Leader exit is not proof that the process group is empty. A remaining descendant is therefore a plausible source of the observed lock, but the production evidence did not identify the lock owner. This proposal fixes the confirmed unsafe handoff without claiming that descendant ownership of that specific lock has been proven.

## Proposed Solution

Make Unix Apply cleanup a repository-finalization barrier. A successful cleanup outcome requires both reaping the spawned leader and observing the owned process group as absent. Probe process-group presence with signal 0: success means present, `ESRCH` means absent, and `EPERM` or any other error means unknown. Present or unknown never permits finalization.

Use the existing bounded SIGTERM then SIGKILL sequence. Poll through an injected process-group probe, clock, and sleeper. After SIGTERM, wait until the graceful deadline for both conditions. If either is missing, send SIGKILL to the group and wait through a separate forceful deadline. Return a typed outcome distinguishing confirmed quiescence from timeout, permission/unknown probe result, signal failure, and leader-reap failure.

The shared Apply loop may create a WIP snapshot, run cleanup review, create the final Apply commit, enter rejecting handoff, or dispatch Acceptance only after confirmed quiescence. Any unconfirmed outcome is an Apply failure with diagnostics. This contract is Unix-specific; the existing Windows job-object lifecycle remains unchanged and no equivalent Windows claim is introduced.

This remains one proposal because process cleanup evidence and the Apply gate must ship together: either half alone preserves the unsafe handoff.

## Acceptance Criteria

1. Stable task completion or rejecting handoff still uses the bounded Apply completion grace period.
2. On Unix, grace expiry starts bounded SIGTERM cleanup; SIGKILL follows only when leader reaping plus process-group absence are not both confirmed by the graceful deadline.
3. Finalization is allowed only when the leader has been reaped and a signal-0 probe returns `ESRCH`; success, `EPERM`, and unexpected probe errors are present/unknown and fail closed.
4. Leader exit alone, lock disappearance alone, elapsed time, PID disappearance, or file age cannot establish quiescence.
5. A synthetic descendant that holds the managed-worktree `index.lock` cannot race a subsequent WIP snapshot or final Apply commit in the regression fixture.
6. If quiescence remains unconfirmed after the forceful deadline, Apply fails with phase, PGID, leader-reap state, last probe result, and signal diagnostics; no finalization or Acceptance handoff begins.
7. Natural completion and explicit cancellation preserve existing success/failure semantics while strict Unix cleanup uses the same outcome matrix.
8. Windows behavior and restart routing are unchanged; no durable workflow state is introduced.

## Explicit Completion Conditions

- Unix process management provides injectable probe/clock/sleeper seams and maps signal-0 results exactly: success to present, `ESRCH` to absent, and all other errno values to unknown.
- The typed cleanup result records termination phase, leader status/reap state, PGID, and final probe/signal evidence. Only `leader_reaped && group_absent` produces confirmed quiescence.
- The implementation documents that zombies or PGID reuse can conservatively keep the probe present; this is fail-closed and must not be converted into success.
- `AiCommandRunner` awaits the typed result and cannot publish success-equivalent completion-grace status for an unconfirmed outcome.
- The shared Apply loop gates WIP snapshot, cleanup review, final Apply commit, rejecting handoff, and Acceptance on confirmed cleanup.
- Deterministic unit tests cover the full outcome matrix without real sleeping. Unix real-process coverage creates a synthetic leader/descendant and a descendant-held real `index.lock`; it does not claim to reproduce the unobserved production lock owner.
- Real-process tests exceeding one second use `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`.
- `cargo test --lib process_group_cleanup`, `cargo test --lib apply_process_group_barrier`, `cargo test --features heavy-tests --test process_cleanup_test apply_completion`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Proving which process owned the lock in the historical production incident.
- Retrying final Apply Git operations after confirmed cleanup.
- Deleting or bypassing Git lock files.
- Detecting descendants that deliberately leave the owned process group/session.
- Changing Windows job-object behavior or the repository-level singleton lock.
- Adding out-of-worktree durable workflow state.
