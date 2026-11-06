## ADDED Requirements

### Requirement: Session stop received before the next spawn refuses that attempt

For a configured worktree session, the runner's cancellation path MUST honour
a `stop_worktree_session` that is received while no child process is running
— during the retry delay that follows a crash retry, during the retry delay
that follows an inactivity-timeout retry, during the stagger wait before an
attempt, and at the final spawn admission — by refusing the next attempt
instead of spawning it. The runner MUST race each of those waits against the
handle's cancellation and MUST re-read the cancellation immediately before
the spawn call, so the "admits no further attempt" guarantee holds on the
real process path and not only in an injected fixture. A refused attempt MUST
publish a typed `stop_refused_attempt` runner notice naming the attempt that
was refused and the stage at which it was refused (`retry`, `stagger`, or
`spawn`), MUST NOT publish `AttemptStarted`, and MUST settle the invocation
with `CommandTermination::Cancelled`, the process-group cleanup report of the
attempt that actually ran (or the not-applicable report when none did), the
writer lease settled, and no further loop iteration. An already scheduled
`retry_scheduled` notice MUST remain in `runner_notices`; a stop received
before the retry is scheduled MUST schedule none.

The session MUST then read `stopped`, keep `attempt` at the last attempt that
actually spawned (`1` when the first attempt is the only one that ran, and the
legacy initial `1` when none did), publish `exit_code` and `signal` from the
reaped leader of the attempt that ran (its own exit code with `signal: null`
after a crash, the signal that reaped it after an inactivity termination,
both `null` when no attempt ever spawned), retain only that attempt's bytes in
`output.lines`, and publish `result.terminated` as today's fact — `true` only
when the state is `stopped` and the last attempt's process group was proven
quiescent — with `result.state_after: stopped`. No wire member is added or
removed; the documented `kind` vocabulary gains `stop_refused_attempt`, and an
older owner that never publishes it MUST decode unchanged.

The cancellation signal MUST be read without being consumed while pending,
without being lost once sent, and without misreading a closed channel: a
dropped handle on the registry-scoped runner is a cancellation, exactly as the
running-child cancellation branch already decides, and a pending signal is left
armed for the next checkpoint. A stop that arrives after the final admission
check returned and before the spawn call returned MAY still be honoured
against the attempt it admitted, by the existing running-child cancellation
branch, and the session then truthfully publishes that attempt; the contract
MUST name this as the only remaining way an attempt can start after a stop.

This behaviour MUST be selected by an explicit stop policy on the shared
retry loop that only the session registry's observed entry point opts into.
The text-emitting entry point used by the TUI terminal, the parallel executor,
apply, archive, acceptance, and shell commands MUST keep its default
spawn-then-cancel behaviour and stderr text byte for byte; scope shutdown MUST
keep its own loop-head, pre-delay, and admission checks and its
`shutdown_refused` notice on both policies; session stop and owner shutdown
MUST remain two distinguishable operations; and the SIGTERM-to-SIGKILL
escalation, verified group cleanup, observed signal, retry policy, stagger
semantics for every existing caller, `CommandTermination`, and
`StreamingChildHandle` MUST be unchanged. No kill authority is added and no
other session or the scheduler is affected by one session's stop.

#### Scenario: Stop during a crash retry delay spawns nothing

- **GIVEN** a session with a retry policy admitting a retry, a 60-second retry delay, and a command that records its PID and exits non-zero
- **AND** the session resource already shows a `retry_scheduled` notice for attempt `2`
- **WHEN** `stop_worktree_session` is submitted
- **THEN** the record settles `result.state_after: stopped` well within the hang guard rather than after the delay
- **AND** exactly one PID was ever recorded by the command and `attempt` is `1`
- **AND** `runner_notices` is `retry_scheduled` for attempt `2` followed by `stop_refused_attempt` for attempt `2` with `stage=retry`
- **AND** `exit_code` is the first attempt's own exit code, `signal` is `null`, and `output.lines` holds only the first attempt's output

#### Scenario: Stop during an inactivity retry delay spawns nothing

- **GIVEN** a session whose command prints nothing, an inactivity timeout with a remaining inactivity retry budget, and a 60-second retry delay
- **AND** the session shows `inactivity_terminated` for attempt `1` and `retry_scheduled` for attempt `2` with `cause=inactivity_timeout`
- **WHEN** `stop_worktree_session` is submitted
- **THEN** no second process is spawned and `attempt` is `1`
- **AND** `stop_refused_attempt` for attempt `2` with `stage=retry` follows the scheduled notice
- **AND** `signal` is the signal that reaped the first attempt and the session reads `stopped`

#### Scenario: Stop while parked in the stagger wait or at the spawn boundary spawns nothing

- **GIVEN** a session whose first attempt crashed with no retry delay, and either a long stagger wait before attempt `2` or the shared stagger lock held by the test so attempt `2` is blocked at the spawn boundary
- **WHEN** `stop_worktree_session` is submitted and the barrier is then released
- **THEN** no second process is spawned, `attempt` is `1`, and the session reads `stopped`
- **AND** `stop_refused_attempt` for attempt `2` names `stage=stagger` or `stage=spawn`

#### Scenario: Stop before the first spawn is stopped, not failed

- **GIVEN** a session whose first attempt is blocked at the spawn boundary before any process exists
- **WHEN** `stop_worktree_session` is submitted and the barrier is released
- **THEN** no process is ever spawned and the session reads `stopped` with `attempt: 1`, `exit_code: null`, `signal: null`
- **AND** `runner_notices` holds one `stop_refused_attempt` for attempt `1`

#### Scenario: Stopping a running leader is unchanged

- **GIVEN** a running session whose command ignores SIGTERM, and another whose command honours it, each with a retry policy of at least one retry
- **WHEN** `stop_worktree_session` is submitted to each
- **THEN** the ignoring session settles `stopped` with `signal: SIGKILL` and the honouring one with `signal: SIGTERM`, both with `terminated: true`, `attempt: 1`, and the whole group gone
- **AND** neither session records a `stop_refused_attempt` notice

#### Scenario: Owner shutdown during a retry delay stays a shutdown refusal

- **GIVEN** a session parked in a 60-second retry delay after its first attempt crashed
- **WHEN** the owner closes the session registry scope
- **THEN** no second process is spawned and the session reads `stopped` with `attempt: 1`
- **AND** `runner_notices` records `shutdown_refused` and no `stop_refused_attempt`

#### Scenario: The text entry point keeps spawn-then-cancel

- **GIVEN** the same crashing command run through the text-emitting runner entry point with the shared stagger lock held so attempt `2` is blocked at the spawn boundary
- **WHEN** the handle is terminated and the lock is released
- **THEN** attempt `2` spawns and is then terminated by the existing cancellation branch, so two PIDs were recorded
- **AND** the `[Retry 1/N]` stderr line is printed as before and no refusal text appears
