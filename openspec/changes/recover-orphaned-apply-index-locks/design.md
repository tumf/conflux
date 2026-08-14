## Context

Apply currently has two safety mechanisms around managed-worktree Git operations:

- `ProcessGroupCleanupReport` proves whether the owned process group is quiescent.
- WIP and final-commit retry policies classify narrow existing-`index.lock` failures and retry idempotently without deleting the lock.

They leave a gap when forced process cleanup kills the lock creator after lockfile creation but before Git commits or rolls back the lock. Process quiescence is true, yet filesystem convergence is false. Waiting cannot repair a lock with no surviving owner.

The recovery authority must stay narrower than generic stale-lock cleanup. It is valid only for one Apply dispatch whose pre-dispatch state and cleanup result are both still available in memory.

## Goals

- Converge a same-dispatch orphaned zero-byte managed-worktree `index.lock` after confirmed quiescence.
- Preserve fail-closed behavior when provenance, identity, stability, or platform support is ambiguous.
- Keep retry policy separate from reclamation authority.
- Preserve workspace-derived restart routing and avoid durable recovery state.

## Non-Goals

- Proving that a specific PID or process group created the lock.
- Supporting external Git concurrency on managed worktrees.
- Cleaning stale locks found at startup or after a process restart.
- Providing a Windows lock-reclamation implementation.

## Decisions

### Decision: Use a pre-dispatch absence observation instead of wall-clock provenance

Before spawning an Apply command, resolve the managed worktree's lock candidates and `lstat` them. Record process-lifetime observations that distinguish absent, present with file identity, and unverifiable.

A post-cleanup lock is eligible only if the corresponding candidate was absent before this dispatch. This excludes every lock already visible before the agent started without relying on mtime/ctime ranges, NTP, filesystem timestamp granularity, or log timestamps.

If pre-dispatch resolution or observation fails, the dispatch may still run, but post-quiescence reclamation is unavailable and any remaining lock is refused rather than deleted.

### Decision: Require Unix confirmed quiescence

Reclamation requires `ProcessGroupQuiescence::Confirmed`. `NotApplicable` is insufficient because it says verification does not apply, not that this Unix process set was proven empty. `MembersRemain` and `Unverifiable` already block repository finalization and continue to do so.

The process-group result is a lifecycle authorization boundary, not proof of file ownership. Reclamation therefore also requires independent file evidence.

### Decision: Use two identity-preserving filesystem observations

The first post-quiescence observation uses `lstat` and requires:

- the exact managed-worktree lock path;
- a regular file rather than a symlink or another file type; and
- zero length.

After a 500 millisecond dwell, open the path read-only with `O_NOFOLLOW` and inspect that descriptor with `fstat`. The second observation must match the first device and inode, remain zero length, and retain the same modification time. Opening the path before the second `fstat` prevents metadata from being read through a symlink and binds the second evidence to an actual file object.

A changed or missing identity before the second observation is not reclamation authority. A missing path is natural convergence and needs no unlink.

### Decision: Accept a bounded residual TOCTOU window

Portable Unix APIs do not provide an atomic "unlink this pathname only if it still names this open inode" operation. A race remains between second `fstat` and `unlink`.

The design bounds this risk by requiring same-dispatch pre-absence, confirmed quiescence, a 500 millisecond unchanged dwell, identical device/inode, and zero length. A concurrent external Git actor that creates or reuses the path inside the final race can be made to fail, but its committed index is not accepted as Conflux evidence and Conflux finalization still revalidates repository state. This residual race and the unsupported external-concurrency assumption are explicit rather than hidden.

If this risk is later judged unacceptable, the upgrade path is to remove automatic unlink and retain only typed refusal; `lsof` or a longer age threshold is not an acceptable substitute.

### Decision: Keep reclamation outside retry policies

`wip_lock_retry` and `final_commit_lock_retry` continue to classify only structured Git failures and prove ambiguous command completion from repository state. They never unlink.

One reclamation attempt occurs after the Apply process-group barrier and before repository work. Retry then handles live transient contention that appears later, including external tools or hook-side Git activity.

### Decision: Apply one implementation at two finalization boundaries

The shared reclamation function is consumed by:

1. normal `execute_apply_loop` post-agent completion, immediately after `evaluate_process_group_barrier` succeeds; and
2. `preserve_interrupted_apply_progress`, immediately after confirmed cleanup and before dirtiness/progress reads.

Both consumers treat unsafe or unverifiable reclamation as a repository-finalization refusal. This prevents one path from preserving progress while another remains vulnerable to the same residue.

### Decision: Increase supplementary retry to five attempts at 500 milliseconds

Both retry policies use five total attempts with four fixed 500 millisecond waits. The two-second total delay is long enough for ordinary transient contention while remaining bounded and cancellation-aware. It does not attempt to solve orphaned locks; reclamation owns that case.

The injected environment sleepers remain the unit-test mechanism so default tests do not wait in real time. Real process cleanup and filesystem integration cases remain heavy when they cannot satisfy the default one-second suite target.

## Proposed Runtime Types

The exact names may follow local conventions, but the implementation must preserve these responsibilities:

- `PreDispatchLockObservation`: workspace-bound candidate observations captured before one Apply spawn.
- `IndexLockReclaimOutcome`: `NotPresent`, `Reclaimed`, `NaturallyConverged`, or `Refused(reason)`.
- `IndexLockReclaimRefusal`: typed reasons for cleanup outcome, missing pre-observation, pre-existing lock, path/file type, non-zero length, identity/stability change, observation failure, and unlink failure.
- an injectable filesystem/dwell environment for deterministic decision tests.

The observation is consumed once and is neither serialized nor retained for restart routing.

## Failure and Diagnostics Contract

A refusal is terminal for the current Apply finalization boundary. Diagnostics must include the change, iteration, workspace, lock path, process-group quiescence classification, and the specific evidence condition that failed. When available, include device, inode, size, and modification-time observations without exposing unrelated filesystem contents.

A naturally absent lock is success. `ENOENT` during final unlink is also natural convergence. Other unlink errors are refusals.

No refusal may create a WIP commit, final Apply commit, cleanup-review invocation, rejecting handoff, or Acceptance dispatch.

## Verification Strategy

- Pure decision tests use fake filesystem observations and an injected dwell to cover all branches without sleeping.
- Unix filesystem tests use temporary directories to prove `O_NOFOLLOW`, device/inode replacement, natural disappearance, and unlink behavior.
- Apply-loop tests use fake cleanup evidence and workspace managers to prove both finalization boundaries consume the same outcome and suppress side effects on refusal.
- Heavy process integration creates a real zero-byte lock from a descendant, triggers cleanup, and verifies reclamation before exactly one Git finalization.
- Existing retry tests change expected attempts/delays and continue to prove cancellation and idempotency.

## Constitution Alignment

All authorization evidence is either current workspace filesystem state or ephemeral process-lifetime observation. No state is persisted outside the workspace, and restart routing cannot consume the observation. Successful completion remains repository-verifiable after normal Git finalization. `openspec/CONSTITUTION.md` requires no amendment.
