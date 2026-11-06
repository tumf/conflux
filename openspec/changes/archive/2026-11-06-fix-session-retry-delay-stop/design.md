# Design: a session stop during a retry delay admits no further attempt

## Source evidence at `feature/public-api-provider-extensions` `f64f34d7b`

Line numbers are from the integrated commit; the implementer re-reads them
before editing.

### The cancel signal and where it is read

- `StreamingChildHandle { cancel_tx: Option<oneshot::Sender<()>>, .. }`
  (`src/process_manager.rs:220-236`); `terminate()` takes the sender and sends
  `()` once, idempotent (`:326-333`). There is no second channel: a stop is
  this one oneshot.
- Runner side: `let (cancel_tx, cancel_rx) = oneshot::channel::<()>()`
  (`src/ai_command_runner.rs:1763`); inside the task `cancel_rx_opt =
  Some(cancel_rx)` and `cancel_observed = false` (`:1832-1834`).
- The receiver is awaited in exactly two `tokio::select!` branches, both
  guarded by `if !cancel_observed` and both reachable only while a child is
  running: `:2159-2169` (inactivity-timeout monitoring loop, `'watch`) and
  `:2479-2489` (`'watch_no_timeout`). Both set `cancel_observed = true;
  cancel_rx_opt = None` and treat the result as a cancellation when
  `result.is_ok() || scoped` — a dropped handle cancels a scoped runner,
  never an unscoped one (`:2166-2169`, `:2486-2489`).
- Each select is `biased;` with the order: scope shutdown
  (`wait_for_scope_shutdown`, `:3054-3059`), handle cancel, runtime limit,
  activity, inactivity sleep. The scope branch and the cancel branch do the
  same work: `mark_cleaning`, `terminate_with_timeout(5s)`,
  `RunnerNotice::from_termination`, `verify_owned_process_group`,
  `record_cleanup`, `publish_cleanup_report`, `publish_termination(Cancelled)`,
  `settle_writer_lease`, `status_tx.send(make_fail_status())`,
  `execution.finish()`, `return` (`:2121-2155`, `:2170-2227`).

### Where the loop sleeps without reading it

- Loop head: shutdown recheck only — `execution.is_shutdown()` →
  `ShutdownRefused { stage: "retry" }`, `break 'retry` (`:1854-1874`), then
  `attempt += 1` (`:1876`).
- Stagger: `command_queue.wait_for_stagger_slot().await` (`:1909`), which
  holds the shared `tokio::sync::Mutex<Option<Instant>>` across its sleep and
  updates the timestamp before releasing (`src/command_queue.rs:225-240`).
- Admission: scoped → `execution.admit_spawn(|| cmd.spawn())` under the scope
  lock, refusing only `state.closed` (`:1910-1912`, `:1210-1231`); unscoped →
  lease marks and `cmd.spawn()` (`:1913-1926`). `None` → `ShutdownRefused {
  stage: "spawn" }` (`:1928-1942`); `Some(Ok)` → `AttemptStarted { attempt }`
  (`:1944-1950`).
- Post-exit cleanup and retry scheduling: the attempt's leader is reaped
  (`:2608-2624`), `LeaderReaped` published (`:2625`), strict cleanup swept and
  recorded into `cleanup_report` (`:2636-2659`). Inactivity branch:
  `inactivity_retries_used += 1`, `terminate()`, 100 ms sleep, `force_kill()`,
  `RetryScheduled { cause: InactivityTimeout }`, **`sleep(retry_delay_ms)`**,
  `continue 'retry` (`:2660-2706`). Crash branch: `should_retry`, `terminate()`,
  100 ms sleep, `force_kill()`, `RetryScheduled { cause: Crash }`,
  **`sleep(retry_delay_ms)`**, `continue 'retry` (`:2729-2774`). Both branches
  recheck `is_shutdown` before scheduling (`:2666-2670`, `:2735-2738`) and
  nothing about the handle.
- Final settlement outside the loop: `final_exit_status.unwrap_or_else(
  make_fail_status)`, downgrade on unconfirmed cleanup, `publish_cleanup_report`,
  `publish_termination(final_termination)`, `settle_writer_lease`,
  `status_tx.send`, `execution.finish()` (`:2786-2807`).

### The observed path and the session

- `execute_streaming_with_retry` → `..._sink(.., NoticeSink::Text)`
  (`:1684-1699`); `execute_streaming_with_retry_observed` →
  `..._sink(.., NoticeSink::Typed(notice_tx))` (`:1709-1731`). `NoticeSink`
  (`:1472-1505`) is the only per-entry-point policy today, and it governs
  presentation only.
- `RunnerNotice` (`:1407-1437`) and `RetryCause` (`:1395-1400`). Adding a
  variant touches one exhaustive match, `apply_notice` in
  `src/worktree_ops/session.rs:1303-1380`.
- Session launch: one `execute_streaming_with_retry_observed(&spec.command,
  Some(&spec.path), Some("worktree_session"), None)` on a runner bound to the
  registry's own scope (`session.rs:908-913`, `:1043-1050`). `supervise`
  forwards a stop as `handle.terminate()` (`:1109-1119`), drains notices
  until the runner closes the channel (`:1124-1127`), then reads `wait()`,
  `termination()`, `process_group_cleanup()` and classifies (`:1128-1150`).
- `stop()` marks `stop_requested`, sends the stop oneshot, waits for
  settlement, and publishes `terminated: state_after == Stopped && confirmed ==
  Some(true)` (`:1209-1234`). `classify`: `Cancelled` → `Stopped` with
  `exit_code`/`signal` from the last `LeaderReaped`/`LeaderNotReaped` (`:1382-1396`);
  `NotStarted if stop_requested` → `Stopped, None, None` (`:1397`).
- The injected fixture `InjectedRun` and the archived
  `remote_contract_gap_session_stop_during_retry_delay_keeps_started_attempt`
  (`src/worktree_ops/session/tests.rs:307-375`, `:397-432`) never construct a
  runner; the real-process stop cases
  (`src/web/remote_control_api/tests/worktree_session_tests.rs:1023`, `:1520`,
  `:1558`) stop a *running* leader, never a parked retry.
- Heavy fixture seams: `Options { template, retries, provision }` and
  `Wired::new` building `OrchestratorConfig { command_queue_stagger_delay_ms:
  Some(0), command_queue_max_retries: Some(retries), command_queue_retry_delay_ms:
  Some(0), .. }` and the runner from `AiCommandRunner::from_orchestrator_config(
  &config, Arc::new(tokio::sync::Mutex::new(None)))` (`:207-245`); helpers
  `fixture.file/fifo/script/add_worktree`, `sleeper` (`:487-505`),
  `wired.launched/session/settled/stop/shutdown`, `registry.launch_count()`,
  `read_fifo`, `pids`, `alive`, `group_gone`.
- Existing runner-level shutdown tests that are the shape to copy:
  `run_command_scope_refuses_spawn_after_shutdown` (pre-armed stagger,
  `:3593-3660`) and `run_command_scope_suppresses_retry_after_shutdown`
  (`:3661-3720`).

### Why the stop fires only after the next spawn today

Trace, crash cause, stop sent at time T during the delay: attempt 1 exits →
reaped → `RetryScheduled { next_attempt: 2 }` → `sleep(retry_delay_ms)` with
the oneshot holding `()` unread → `continue 'retry` → loop head sees the scope
open → `attempt = 2` → stagger → `admit_spawn` admits (scope open) → child 2
spawned → `AttemptStarted { 2 }` → monitoring loop's first `select!` finds the
cancel ready → `terminate_with_timeout(5s)` on child 2 → `LeaderReaped {
attempt: 2, signal: 15 }` → `Cancelled`. The session therefore publishes
`attempt: 2`, `signal: SIGTERM`, and a second process ran. Nothing is lost
or misread; the signal is simply not looked at for the whole gap between
`LeaderReaped { 1 }` and `AttemptStarted { 2 }`.

Why the fixed path settles as `Cancelled` → `stopped` with no retry: the new
refusal branch publishes `CommandTermination::Cancelled` and returns from the
task exactly like the existing cancel branch, without `continue 'retry`;
`classify` maps `Cancelled` to `SessionState::Stopped` unconditionally
(`session.rs:1393-1396`), and `stop()` derives `terminated` from that state
plus the published cleanup (`:1230`). No new session state is needed.

## Contract

### 1. Explicit stop policy on the shared retry loop

```rust
/// How the retry loop treats the handle's cancellation while no child runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopAdmission {
    /// Behaviour at f64f34d7b: the cancel is observed at the next child
    /// monitoring loop, so a stop during a delay cancels the *next* attempt
    /// after it spawned. Every existing entry point keeps this.
    SpawnThenCancel,
    /// A stop received before the next spawn refuses that spawn.
    RefuseBeforeSpawn,
}
```

- `execute_streaming_with_retry_sink(.., sink: NoticeSink, stop: StopAdmission)`.
  `execute_streaming_with_retry` passes `SpawnThenCancel`;
  `execute_streaming_with_retry_observed` passes `RefuseBeforeSpawn`. The
  policy is a parameter, not inferred from `NoticeSink`, so the two stay
  separable and a future text caller could opt in explicitly; this change adds
  no such caller.
- Under `SpawnThenCancel` the loop is byte-for-byte today's: no new await, no
  new `try_recv`, no new notice.

### 2. Checkpoints under `RefuseBeforeSpawn`

One helper, unit-testable without a process:

```rust
enum CancelCheck { Proceed, Refuse }

/// Non-blocking read of the handle's cancellation at a spawn boundary.
fn cancel_check(
    cancel_rx_opt: &mut Option<oneshot::Receiver<()>>,
    cancel_observed: &mut bool,
    scoped: bool,
) -> CancelCheck
```

- `None` receiver or `*cancel_observed` → `Proceed` (already neutralised or
  consumed; the running-child branches own that state).
- `try_recv()` → `Err(Empty)`: `Proceed`, receiver left in place, nothing
  consumed. `Ok(())`: `*cancel_observed = true; *cancel_rx_opt = None;
  Refuse`. `Err(Closed)`: `*cancel_observed = true; *cancel_rx_opt = None;`
  then `Refuse` if `scoped`, else `Proceed` — the identical rule the
  monitoring branches apply to `result.is_ok() || scoped`.

And one awaitable race used by the two delay sites:

```rust
/// Resolve when `wait` completes, or earlier with `Refuse` when the handle
/// was cancelled (or dropped, for a scoped runner). Biased to the cancel.
async fn race_with_cancel<F: Future<Output = ()>>(
    wait: F, cancel_rx_opt: &mut Option<oneshot::Receiver<()>>,
    cancel_observed: &mut bool, scoped: bool,
) -> CancelCheck
```

implemented as `tokio::select! { biased; r = &mut rx, if armed => ..., _ =
wait => Proceed }`, awaiting `&mut Receiver` (cancel-safe; a oneshot receiver
polled by reference is not consumed until it yields). `Err(Closed)` on an
unscoped runner neutralises and then keeps awaiting `wait`, so a dropped handle
never shortens a delay for an unscoped caller.

Checkpoints, all `RefuseBeforeSpawn`-only:

| site | today | after |
|---|---|---|
| (a) immediately after the existing `terminate()` / 100 ms grace / `force_kill()` sequence and before `RetryScheduled` in both retry branches | scope shutdown is checked at the existing branch predicate | preserve the existing scope predicate and cleanup sequence, then run `cancel_check(..)` after `force_kill()`; a `Refuse` publishes `StopRefusedAttempt { next_attempt: attempt + 1, stage: "retry" }` with no `RetryScheduled` and settles. Never skip the previous group's cleanup because cancellation is ready |
| (b) `sleep(retry_delay_ms)` at `:2704` and `:2772` | unconditional | `race_with_cancel(sleep, ..)`; `Refuse` → `StopRefusedAttempt { next_attempt: attempt + 1, stage: "retry" }` and settle |
| (c) `wait_for_stagger_slot()` at `:1909` | unconditional | `race_with_cancel(stagger, ..)`; `Refuse` → `StopRefusedAttempt { next_attempt: attempt, stage: "stagger" }` and settle. Dropping the stagger future mid-wait releases the shared lock without claiming the slot, which is correct: no spawn happens |
| (d) immediately before `admit_spawn` / `cmd.spawn()` (`:1910`) | none | `cancel_check(..)`; `Refuse` → `StopRefusedAttempt { next_attempt: attempt, stage: "spawn" }` and settle |

The 100 ms post-crash grace sleeps (`:2682`, `:2755`) are cleanup of the dead
attempt's group, not a spawn boundary; they stay unconditional, and checkpoint
(a) runs after them.

Residual window, stated rather than hidden: a stop that is sent after
checkpoint (d) returns `Proceed` and before `spawn` returns is observed by the
monitoring loop's existing cancel branch and terminates the just-spawned
attempt, exactly as today. That is the only way an attempt N+1 can still run
after a stop, and the session then truthfully publishes `attempt: N+1` with
the terminating signal. The spec delta names this window; the test design
below makes it unreachable by holding the spawn boundary closed from the
test, so the race is driven, not waited for.

### 3. Settlement of a refused attempt

The refusal path is the existing cancel branch minus the process work, in the
same order, so every consumer reads the same evidence shape:

1. `sink.emit(&out_tx, RunnerNotice::StopRefusedAttempt { .. }, None)` — no
   text form; the text sink never reaches this path.
2. `pid_arc` is already `0` (the previous attempt stored `0` after reaping).
3. `publish_cleanup_report(&mut cleanup_tx, cleanup_report)` — the report
   already accumulated for the attempt that ran (the strict sweep at
   `:2631-2656`, or `not_applicable` when no attempt ever spawned, e.g. a
   stop during attempt 1's stagger).
4. `publish_termination(&mut termination_tx, CommandTermination::Cancelled)`.
   `Cancelled` and not `NotStarted`: the invocation ended because the handle
   was cancelled, and `NotStarted` stays reserved for refusals the runner made
   on its own (admission, launch failure, unwaitable child). The session maps
   both to `stopped` when `stop_requested`, so the choice changes no session
   state; it keeps `termination()` honest for any other future reader.
5. `settle_writer_lease(&mut writer_lease)`; `status_tx.send(make_fail_status())`;
   `execution.finish()`; `return`.

No `continue 'retry`, no `attempt` advance, no `AttemptStarted`.

### 4. Session-side facts

- `apply_notice` gains one arm: `StopRefusedAttempt { next_attempt, stage }` →
  `RunnerNoticeRecord { kind: "stop_refused_attempt", attempt: Some(next_attempt),
  detail: Some(format!("stage={stage}")) }`. `attempt` is untouched by it.
- `classify(Cancelled, leader)` is unchanged. For a refusal the last leader
  fact is the attempt that ran: a crashed attempt 1 reports its real
  `exit_code` (for example `3`) and `signal: null`; an inactivity-terminated
  attempt 1 reports whatever its forced kill reaped (`SIGKILL`, or `SIGTERM`
  if it honoured the first signal); a stop during attempt 1's stagger has no
  leader and reports `null`/`null`. Nothing substitutes a requested signal.
- `StopOutcome.terminated` keeps `state_after == Stopped && confirmed ==
  Some(true)`, matching its wire documentation "the runner cancelled it and
  proved the process group quiescent" (`dto.rs:2577-2585`): the group proven
  quiescent is the last attempt's. The test asserts it against the published
  `process_group_confirmed` rather than hardcoding `true`, because strict
  cleanup is a configuration fact.
- `SessionRunnerNotice.kind` doc (`dto.rs:2673-2675`) and the OpenAPI
  description add `stop_refused_attempt`; `detail` example gains
  `stage=retry`. `kind` stays a free string; no enum, no new member.

### 5. What is deliberately not changed

- `SpawnThenCancel` for `execute_streaming_with_retry`: the TUI terminal
  (`src/tui/terminal.rs`), parallel executor (`src/parallel/executor.rs:44,
  392, 1160, 2694-2758`), apply (`src/execution/apply.rs:554, 2644`), archive
  (`src/orchestration/archive.rs:319, 342`), acceptance
  (`src/orchestration/acceptance.rs:2265-2336`), and judge
  (`src/judge_command.rs:312`) call `terminate()` with today's semantics and
  must keep them; this change does not decide whether they should ever opt in.
- Scope shutdown: loop-head check, pre-delay checks, `admit_spawn` critical
  section, `ShutdownRefused` notices, the `wait_for_scope_shutdown` select
  branch. Session stop and owner shutdown remain two operations with two
  notices.
- The running-child cancel branch: SIGTERM, 5-second escalation, SIGKILL,
  `verify_owned_process_group`, `from_termination`, observed signal.
- `StreamingChildHandle`: still a oneshot; no `watch`, no `AtomicBool`, no new
  method. A `watch` channel would change the handle type every caller holds.
- `CommandTermination`: no new variant (exhaustive matches in
  `process_manager`, apply, and the parallel executor).
- `wait_for_stagger_slot`: its body is unchanged; the observed path merely
  races the future.

### Constitution check

The stop policy and every notice are process-local and die with the owner;
nothing is persisted, nothing is read for resume, acceptance, or archive
routing; no Conflux-owned path is written into any worktree, and no cleanliness
check excludes one. Truthful completion is strengthened: `attempt`,
`runner_notices`, `exit_code`, `signal`, `terminated`, and
`process_group_confirmed` each describe a process that really ran or a refusal
the runner really made, and `docs/guides/USAGE.md` will state the implemented
behaviour rather than the fixture's.

## Rejected alternatives

- **Keep the code and narrow the spec** ("admits no further attempt once the
  cancel is observed"): makes the finding disappear on paper while a stop
  still launches a process the operator asked not to run. Rejected.
- **Change the default for every caller**: the TUI, apply, archive, and
  acceptance paths were never promised refuse-before-spawn and have their own
  completion-grace and escalation tests built on today's timing; widening the
  change would make this a runner redesign, which the parent excluded.
- **Infer the policy from `NoticeSink::Typed`**: couples presentation to
  lifecycle; an explicit parameter keeps them separable and testable.
- **Replace the oneshot with `tokio::sync::watch` or an `AtomicBool`**: changes
  `StreamingChildHandle`, every `for_test` constructor, and the injected
  session fixture; the oneshot is sufficient when it is read at the right
  places, and `&mut Receiver` plus `try_recv` give exactly the needed
  non-consuming and non-blocking reads.
- **Close the registry scope on a single-session stop**: stops every session;
  forbidden by the canonical spec.
- **Reuse `ShutdownRefused` for the stop refusal**: conflates owner shutdown
  with one session's stop, which the spec keeps apart; a distinct kind costs
  one string.
- **Make the pre-spawn check atomic with `spawn` through a shared lock on the
  handle**: would require the handle to take a lock the runner holds across
  `spawn`, coupling caller and task; the residual window is already resolved
  by the existing cancel branch and is stated in the contract instead.
- **Prove the behaviour with the injected fixture only**: that is the gap
  being closed; every acceptance case here spawns a real process.

## Verification design (future implementation, not executed at proposal time)

Every new test name starts with `session_retry_stop`. Real-process cases use
`#[cfg_attr(not(feature = "heavy-tests"), ignore)]` and the existing isolated
fixtures. Fast cases stay under one second. Hang guards are generous timeouts,
never elapsed-time assertions; the fix is proven by *settling at all* within
the guard with the right evidence, not by measuring how fast. Liveness and
ordering are proven with notices, PID files, FIFOs, and the stagger lock as a
barrier. Every test process runs with `TMPDIR` set to a per-run absolute
directory under `/home/tumf/work/conflux-api-provider-20261106-evidence/tmp/`;
fixtures that create temporary directories inherit it.

### Controlled race: the shared stagger lock as a spawn-boundary barrier

`Wired::new` and the runner tests construct the runner with a
`SharedStaggerState = Arc<tokio::sync::Mutex<Option<Instant>>>`.
`wait_for_stagger_slot` takes that mutex before it can claim a slot
(`src/command_queue.rs:226`). A test that holds the guard therefore parks the
runner at the spawn boundary of the next attempt — after `RetryScheduled` and
the delay, before admission — with no timing assumption. Releasing the guard
after the stop is sent drives the race in the direction under test. `Options`
gains `retry_delay_ms`, `stagger_delay_ms`, `inactivity: Option<(timeout_secs,
grace_secs, max_retries)>`, and `shared_stagger: Option<SharedStaggerState>`;
`Wired::new` passes them into `OrchestratorConfig` and
`from_orchestrator_config`. Defaults keep every existing test identical.

### Gate `session-retry-stop-runner` (`src/ai_command_runner.rs`)

Fast unit tests on the helpers:

- `session_retry_stop_cancel_check_leaves_pending_receiver_armed`: `Empty` →
  `Proceed`, then sending on the sender and calling again → `Refuse`.
- `session_retry_stop_cancel_check_refuses_sent_cancel_once`: `Ok(())` →
  `Refuse`, `cancel_observed` true, receiver `None`; a second call → `Proceed`
  without panic.
- `session_retry_stop_cancel_check_treats_dropped_handle_by_scope`: `Closed`
  with `scoped: true` → `Refuse`; with `scoped: false` → `Proceed` and the
  receiver neutralised.
- `session_retry_stop_race_prefers_cancel_over_elapsed_wait`: a `Notify`-driven
  wait and a pre-sent cancel → `Refuse`; an unsent cancel and a completed wait
  → `Proceed`; a dropped sender on an unscoped runner → `Proceed` only after
  the wait completes.

Real-process runner tests (heavy, `#[cfg(unix)]`), on a scoped runner through
`execute_streaming_with_retry_observed`, each with a PID-appending command:

- `session_retry_stop_observed_refuses_retry_after_crash_delay_stop`:
  `retry_delay_ms: 60_000`, command `echo $$ >> pids; exit 3`; wait for
  `RetryScheduled { next_attempt: 2 }` on the notice channel; `terminate()`;
  assert the notice channel then yields `StopRefusedAttempt { 2, "retry" }`
  and closes, `termination() == Cancelled`, one PID line, no `AttemptStarted {
  2 }`, `scope.active_executions() == 0`, cleanup report equals the attempt-1
  sweep.
- `session_retry_stop_observed_refuses_retry_after_inactivity_delay_stop`:
  `inactivity_timeout_secs: 1`, `inactivity_kill_grace_secs: 0`,
  `inactivity_timeout_max_retries: 2`, `retry_delay_ms: 60_000`, command
  `echo $$ >> pids; exec sleep 300` (no output); wait for `RetryScheduled {
  cause: InactivityTimeout }`; `terminate()`; same assertions plus
  `InactivityTerminated { 1 }` seen first and `LeaderReaped { 1 }` with a
  signal.
- `session_retry_stop_observed_refuses_spawn_parked_in_stagger`: pre-armed
  stagger `Some(Instant::now())`, `stagger_delay_ms: 60_000`, `retry_delay_ms:
  0`, crashing command; wait for `RetryScheduled { 2 }`; `terminate()`;
  assert `StopRefusedAttempt { 2, "stagger" }`, one PID.
- `session_retry_stop_observed_refuses_spawn_at_held_boundary`: the barrier —
  the test holds the shared stagger lock before launch; attempt 1 is admitted
  only after the test releases it once. Observe `AttemptStarted { 1 }` or
  the first PID file before re-taking the lock (never race the first spawn
  for that lock); arrange the first crash behind a fixture barrier so the
  re-take precedes its exit. Then release the crash barrier; attempt 1 crashes;
  `retry_delay_ms: 0`; the runner blocks on the lock; `terminate()`; release;
  assert `StopRefusedAttempt { 2, "stagger" | "spawn" }` and one PID.
- `session_retry_stop_observed_running_child_still_escalates`: `trap '' TERM;
  sleep 60` running; `terminate()`; assert `LeaderReaped { signal: 9 }`,
  `Cancelled`, no `StopRefusedAttempt` — the cancel branch is unchanged.
- `session_retry_stop_observed_scope_shutdown_in_delay_is_still_shutdown`:
  `scope.close()` during the 60 s delay → `ShutdownRefused { "retry" }`, no
  `StopRefusedAttempt`, `Cancelled` not published (today's final termination
  for that path), one PID.

### Gate `session-retry-stop-legacy` (`src/ai_command_runner.rs`)

- `session_retry_stop_text_entry_point_keeps_spawn_then_cancel`: the same
  held-boundary arrangement through `execute_streaming_with_retry` (scoped, as
  the TUI uses it): attempt 1 crashes, runner blocks on the stagger lock,
  `terminate()`, release; assert **two** PID lines, a `[Retry 1/N]` stderr
  line, no `stage=` refusal text, `termination() == Cancelled`, and the group
  of PID 2 gone. This is the compatibility control; it fails if the default
  policy flips.
- `session_retry_stop_text_entry_point_unscoped_dropped_handle_does_not_shorten_delay`:
  unscoped runner, drop the handle during a 300 ms delay; assert attempt 2
  still runs (two PIDs) — the `Err(Closed)` neutralisation rule is unchanged.
- The existing `run_command_scope_refuses_spawn_after_shutdown`,
  `run_command_scope_suppresses_retry_after_shutdown`,
  `apply_completion_grace_termination_*`, and
  `test_post_completion_cleanup_on_cancellation` run unchanged under the
  `remote_text_edit`/default filters and must not be edited.

### Gate `session-retry-stop-session` (`src/web/remote_control_api/tests/worktree_session_tests.rs`)

All heavy, real `sh` children through the API, `Options` extended as above:

- `session_retry_stop_crash_delay_stop_spawns_nothing`: `retries(3)`,
  `retry_delay_ms(60_000)`, script `echo $$ >> pids; echo first; exit 3`;
  poll `wired.session(id)` until `runner_notices` contains `retry_scheduled`
  (bounded by the hang guard); `wired.stop(id)`; assert record `succeeded`,
  `result.state_after: stopped`, `result.terminated ==
  session.process_group_confirmed`; session `state: stopped`, `attempt: 1`,
  `exit_code: 3`, `signal: null`, notices `[retry_scheduled(2),
  stop_refused_attempt(2, stage=retry)]`, `tail_texts == ["first"]`,
  `pids(file).len() == 1`, `launch_count() == 1`.
- `session_retry_stop_inactivity_delay_stop_spawns_nothing`: `inactivity((1,
  0, 2))`, `retry_delay_ms(60_000)`, script `echo $$ >> pids; exec sleep 300`;
  wait for `retry_scheduled` with `detail` containing
  `cause=inactivity_timeout`; stop; assert `attempt: 1`, notices
  `[inactivity_terminated(1), retry_scheduled(2), stop_refused_attempt(2,
  stage=retry)]`, one PID, `signal` is the reaped kill signal of attempt 1
  (`SIGKILL` or `SIGTERM`), `process_group_confirmed` published.
- `session_retry_stop_stagger_before_retry_spawns_nothing`: pre-armed shared
  stagger, `stagger_delay_ms(60_000)`, `retry_delay_ms(0)`, crashing script;
  wait for `retry_scheduled`; stop; assert `stop_refused_attempt(2,
  stage=stagger)`, one PID, `attempt: 1`.
- `session_retry_stop_racing_the_spawn_boundary_spawns_nothing`: the held
  stagger lock barrier through the API; stop while blocked; release; assert
  one PID, `attempt: 1`, `stop_refused_attempt` with `stage` in
  `{stagger, spawn}`.
- `session_retry_stop_stop_before_first_spawn_is_stopped_not_failed`: held
  lock before attempt 1; stop; release; assert no PID file, `attempt: 1`,
  `state: stopped`, `exit_code: null`, `signal: null`, notices
  `[stop_refused_attempt(1, stage=stagger|spawn)]`, `process_group_confirmed`
  is `true` and `terminated` is `true`: the initial cleanup is
  `ProcessGroupCleanupReport::not_applicable`, whose `is_confirmed()` accepts
  `NotApplicable` (`src/process_manager.rs`). This means cancellation settled
  with no group needing cleanup, not that a process was spawned or killed.
  Keep `pgid` absent and assert no PID file so that distinction is explicit.
- `session_retry_stop_running_leader_stop_is_unchanged`: `sleeper` fixture
  with `retries(3)`; stop while running; assert the same facts as
  `remote_worktree_session_stop_kills_the_process_group_and_reaps` plus the
  absence of `stop_refused_attempt`.
- `session_retry_stop_shutdown_during_delay_is_shutdown_refused`:
  `registry.shutdown(10s)` during a 60 s delay; assert `shutdown_refused`
  notice, no `stop_refused_attempt`, `state: stopped`, one PID — the two
  operations stay distinguishable on the wire.
- `session_retry_stop_notices_disclose_no_path_or_template`: template with a
  credential and absolute path; assert no `stop_refused_attempt.detail`
  contains either (it is `stage=<token>` only).

### Gate `session-retry-stop-contract` (`tests/openapi_contract_tests.rs`)

- `session_retry_stop_contract_documents_the_refusal_kind`: the
  `SessionRunnerNotice.kind` description names `stop_refused_attempt`; the
  existing CLI-export-equals-live-document case still passes.
- `session_retry_stop_contract_old_owner_session_decodes`: the
  `WorktreeSessionResource` fixture captured from `f64f34d7b` decodes
  unchanged (no new member exists to be absent; the assertion is that nothing
  else moved).
- The gate command runs the whole suite.

### Gates `session-retry-stop-regression-*` and `session-retry-stop-build`

After integration, re-run `remote_worktree_session`, `remote_contract_gap`,
`remote_text_edit`, `remote_confirmed_delete`, `remote_merge_abort`, and
`remote_progress_timing` one filter at a time with `cargo test --lib
--features web-monitoring,heavy-tests <filter> -- --nocapture`; each must
select and pass a nonzero count. Then `cargo test` (default features), `cargo
test --features web-monitoring` (`web-monitoring` is in the default set, so
`cargo test --no-fail-fast` covers both and is recorded once as such), and
`cargo clippy --all-targets --features web-monitoring -- -D warnings`. Wide
failures are attributed by re-running the failing test on the unmodified
baseline `10da8ac45` in a temporary detached worktree (known there:
`tests/client_cli_tests.rs::wait_proves_base_publication_only_when_the_remote_matches`,
`tests/client_completion_sink.rs::artifact_is_owner_read_only_by_default_and_is_never_read_back`;
occasional parallel-run flakes of
`remote_merge_abort_failed_abort_is_reported_not_assumed`,
`test_list_changes_native_excludes_hidden`, and
`agent::tests::test_with_runner_paths_preserve_prompt_and_output`); a failure
that reproduces there is recorded as baseline and not fixed here; one that
does not is a regression of this change.

### Expected edits to existing tests

- `remote_contract_gap_session_stop_during_retry_delay_keeps_started_attempt`
  (`session/tests.rs:398`): unchanged; it remains the session-level unit test
  and is no longer the only evidence for the scenario.
- `Options`/`Wired::new` in `worktree_session_tests.rs:207-245`: new optional
  fields with defaults that keep every existing call identical.
- `remote_worktree_session_classification_and_retry_notifications`: unchanged
  unless the exhaustive `apply_notice` test enumerates kinds; then one row for
  `stop_refused_attempt`.
- No other existing test changes; any further edit is reported as a
  deviation.

## Documentation edits

- `docs/guides/USAGE.md:316-318`: replace "The runner observes a stop at its
  next check, so a stop that arrives during a retry delay is honoured right
  after the scheduled attempt spawns, and `attempt` then counts that spawn"
  with the implemented behaviour: a stop received during a retry delay, the
  stagger wait, or at spawn admission refuses the next attempt, records
  `stop_refused_attempt`, and leaves `attempt` at the last started one; a
  stop that lands after admission terminates the attempt it admitted.
- `docs/guides/USAGE.md:335-343` ("Stop"): name the refusal and that
  `terminated` then reflects the last attempt's group cleanup.
- `docs/guides/USAGE.md:363-372` and `AGENTS.md:550-553, 571-573`: add
  `stop_refused_attempt` to the kind list; state that the TUI terminal and
  other text consumers are unchanged.

## Independent review launch contract (for the implementation worker)

Independent implementation reviews are launched exactly as follows and never
through the built-in Agent/Task tools, a `model: fable` subagent, or a direct
`claude` session:

```text
HOST asagi
HOME=/home/tumf
PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin
UUID=$(uuidgen); PROMPT=/tmp/second-opinion-fable-$UUID.md   # write the prompt here
/home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file "$PROMPT" -- \
  /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose \
  --include-partial-messages --dangerously-skip-permissions --print
```

- Recovery path for the prompt file only: if writing `$PROMPT` fails with
  `EDQUOT` (the user quota on `/tmp` of host asagi is exhausted while `df`
  shows free space), write it instead to
  `/home/tumf/work/conflux-api-provider-20261106-evidence/tmp/second-opinion-fable-$UUID.md`
  and pass that absolute path to `--stdin-file`. Nothing else in the argv,
  environment, or wrapper changes, and no unrelated `/tmp` file or another
  session's artifact is deleted to make room.
- No `--model` flag is passed to the wrapper. The review session is separate
  from the implementation session and reviews this one fix as a focused diff
  plus compatibility (the text entry point, scope shutdown, the two archived
  real-process stop cases); it is not a repository audit, and adjacent
  improvements without a serious finding are not added.
- The worker records the job ID, the raw log path
  (`/home/tumf/.local/share/agent-exec/jobs/<id>/full.log` and `stdout.log`
  when present), the exact argv, the actually responding model from the
  stream's `model` field, and the final verdict in the handoff. The prompt
  file is deleted after the run has consumed it.
- A running job is neither killed nor duplicated; its ID is saved, and long
  test runs inside it are waited to synchronous completion
  (`agent-exec wait`), never polled with sleeps.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof, and the worker does not declare a self-PASS. No push, release,
  install, owner restart, or global configuration edit is performed by the
  worker or the reviewer, and the currently running owner is never stopped.
