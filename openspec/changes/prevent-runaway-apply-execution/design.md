# Design: Bounded Apply execution and interruption recovery

## Safety invariant

Conflux-owned repository mutation may begin only after the active agent process group is proven quiescent. If Apply is interrupted after changing a managed workspace, Conflux must preserve those repository-visible changes before ending the run. The preserved Git/workspace state, not logs or process-local counters, determines the next action after restart.

## Unified termination sequence

Cancellation, TUI external shutdown, and absolute runtime timeout use one ordered sequence:

1. Close run-command-scope spawn and retry admission.
2. Cancel the active runner task.
3. Send SIGTERM to the owned process group.
4. Escalate to SIGKILL after the existing grace period.
5. Prove process-group quiescence with the existing typed cleanup report.
6. Inspect managed-worktree status.
7. If dirty, create one Conflux-owned WIP iteration snapshot.
8. Return a typed terminal outcome that does not permit same-run automatic redispatch.

A cleanup failure stops before step 6. A snapshot failure keeps the workspace untouched and returns actionable diagnostics. Neither failure may be reported as successful shutdown.

## Absolute runtime limit

`command_max_runtime_secs` belongs to the common command-runner configuration because every AI command uses the same process ownership boundary. The deadline is measured from successful child spawn and is not reset by stdout or stderr. `0` disables it. The default is 3,600 seconds.

The timeout outcome is not an inactivity timeout, transient error, or generic non-zero crash. It closes retry admission for that invocation. Apply additionally preserves WIP progress and stops the active run so an operator can inspect and explicitly retry.

## WIP identity and restart

The WIP commit remains a normal workspace-local `WIP: <change-id> (...)` commit created by the existing `WorkspaceManager::create_iteration_snapshot` path. No timeout marker, retry counter, or lifecycle state is persisted outside Git/workspace files. A new Conflux process recomputes routing from the preserved worktree and base comparison.

## Verification discipline

Runtime enforcement bounds the outer agent invocation. Portable skills bound work inside the agent:

- Run a verification command once by default.
- A retry is justified only by a repository repair or concrete environment recovery since the previous execution.
- The identical command may run at most three times within one Apply invocation.
- A suspected flaky test is reported as `verification_unstable`; a command that cannot complete within the invocation budget is reported as `verification_timeout`.
- Heavy or non-local gates are proposal-owned CI, Acceptance, or operational observations, not accidental Apply loops.

The skill does not depend on a specific harness timeout command. It requires the agent to use the runtime's managed execution facility when available and to stop with structured blocker evidence when bounded execution cannot be guaranteed.

## TUI signal integration

TUI keyboard stop and external SIGINT/SIGTERM converge on `TuiRunSupervisor` and `RunCommandScope`. Signal handling must not directly exit the process while a run is active. The TUI waits for the same bounded cleanup barrier, then shuts down lifecycle adapters and exits. A second signal may request forceful escalation but cannot bypass quiescence evidence or WIP preservation.

## Testing strategy

Tests avoid short wall-clock correctness thresholds:

- Command deadlines use paused Tokio time or an injected clock plus controlled child fixtures.
- Process cleanup uses existing real process-group integration fixtures with generous safety timeouts.
- Apply WIP preservation uses fake workspace managers for ordering and Git-backed tests for restart-visible commits.
- TUI signal behavior drives the supervisor boundary directly and verifies scope/registry state transitions.
- Skill contract tests inspect embedded source text and blocker schema requirements.
