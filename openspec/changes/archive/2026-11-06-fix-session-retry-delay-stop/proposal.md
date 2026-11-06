---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/archive/2026-11-06-fix-public-api-contract-gaps/proposal.md
  - openspec/changes/archive/2026-11-06-fix-public-api-contract-gaps/design.md
  - openspec/changes/archive/2026-11-06-fix-public-api-contract-gaps/tasks.md
  - openspec/changes/archive/2026-11-05-add-remote-session-command/proposal.md
  - docs/guides/USAGE.md
  - AGENTS.md
verifications:
  - id: session-retry-stop-runner
    requirement: Under the explicit refuse-before-spawn stop policy the retry loop races both retry delays and the stagger wait against the handle's cancellation, rechecks it immediately before spawn admission, spawns nothing after a stop, publishes a typed stop_refused_attempt notice and CommandTermination::Cancelled, and never consumes, loses, or misreads the oneshot cancel signal
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/ai_command_runner.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-session
    requirement: Against real processes, stop_worktree_session during a crash retry delay, during an inactivity retry delay, during the stagger wait before a retry, and racing the spawn boundary leaves exactly one leader PID, attempt 1, state stopped, the retry_scheduled notice followed by stop_refused_attempt, no child output from a second process, and accurate terminated and cleanup evidence, while a stop of a running process still settles SIGTERM or SIGKILL as before
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/worktree_session_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-legacy
    requirement: The text-emitting entry point keeps its default spawn-then-cancel stop policy byte for byte, the TUI terminal, parallel executor, apply, archive, and acceptance call sites are untouched, and scope shutdown behaves exactly as before on both policies
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/ai_command_runner.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests session_retry_stop -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-contract
    requirement: Generated OpenAPI documents the stop_refused_attempt notice kind, the CLI export equals the live document, payloads from an owner built at f64f34d7b decode unchanged, and the full contract suite passes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-session
    requirement: The existing configured-command session group keeps passing with a nonzero count after the stop-policy change
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/worktree_session_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-contract-gap
    requirement: The archived contract-gap group, including its injected stop-during-delay fixture and the two real-process stop signal cases, keeps passing with a nonzero count
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops/session/tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-edit
    requirement: The existing bounded text edit group, which exercises the unscoped runner paths and writer leases, keeps passing with a nonzero count
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/text_edit_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-delete
    requirement: The existing confirmed-delete group keeps passing with a nonzero count, proving an untouched extension is unaffected
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/confirmed_delete_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-merge
    requirement: The existing merge-abort group keeps passing with a nonzero count, proving an untouched extension is unaffected
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/merge_abort_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-regression-timing
    requirement: The existing progress and timing group keeps passing with a nonzero count, proving an untouched extension is unaffected
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/progress_timing_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_progress_timing -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_progress_timing -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: session-retry-stop-build
    requirement: Default-feature and web-monitoring builds and clippy stay clean, with any pre-existing wide-suite failure attributed to the unmodified baseline 10da8ac45 by re-running it there rather than fixed here
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo clippy --all-targets --features web-monitoring -- -D warnings
    rerun: cargo clippy --all-targets --features web-monitoring -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix: a session stop during a retry delay must admit no further attempt

**Change Type**: implementation

## Problem / Context

`fix-public-api-contract-gaps` is implemented, integrated, and archived on
`feature/public-api-provider-extensions` at
`f64f34d7b3b03488452cf18429f7b76c873e726d`. Its independent review (agent-exec
job `abdbce821226a5303520e1cbf02c47e3`, responding model `claude-fable-5-1`,
verdict `PASS_WITH_MINOR_FINDINGS`) left one finding unresolved — finding 2,
quoted from the raw stream:

> The retry-loop head rechecks only scope shutdown, not the handle's cancel
> signal, so a `stop_worktree_session` that arrives during the retry delay is
> honoured only after attempt 2 spawns. The runner then emits
> `AttemptStarted { 2 }` and the session settles with `attempt: 2`. [...] the
> spec scenario "Stopping in the retry delay does not invent a new attempt ...
> no second process was spawned" is proven only by the injected-channel
> fixture, never by the runner.

The implementation only reworded `docs/guides/USAGE.md:316-318` to describe
the real behaviour ("a stop that arrives during a retry delay is honoured right
after the scheduled attempt spawns, and `attempt` then counts that spawn").
That sentence is truthful about the code but contradicts two promises the
canonical `remote-control-api` spec makes:

- `openspec/specs/remote-control-api/spec.md:1340-1343`: `stop_worktree_session`
  cancels through the runner's own handle, whose cancellation path "sends
  SIGTERM to the session's process group, escalates to SIGKILL after 5 seconds,
  verifies the group, and **admits no further attempt**".
- `spec.md:2132-2137`, scenario "Stopping in the retry delay does not invent a
  new attempt": GIVEN the first process failed and attempt 2 was scheduled but
  stop cancelled the delay before another spawn, THEN `attempt` remains `1`
  and **no second process was spawned**.

The code at `f64f34d7b` cannot satisfy either on the real path:

1. The cancel signal is a `tokio::sync::oneshot::Receiver<()>` created at
   `src/ai_command_runner.rs:1763`, wrapped in `cancel_rx_opt` with a
   `cancel_observed` flag (`:1833-1834`). It is polled in exactly two places,
   the `tokio::select!` cancel branches of the two child-monitoring loops
   (`:2159` with an inactivity timeout, `:2479` without). Both branches exist
   only while a child is running.
2. The retry delays are unconditional `tokio::time::sleep(retry_delay_ms)`
   calls: `:2704` after an inactivity-timeout retry is scheduled (`:2695`) and
   `:2772` after a crash retry is scheduled (`:2763`). Neither races the cancel
   signal, and neither is preceded by a cancel check.
3. The loop head (`:1854-1876`) rechecks only `ScopeExecution::is_shutdown`,
   the stagger wait (`:1909`) is an unconditional
   `command_queue.wait_for_stagger_slot().await`, and the final spawn
   admission (`:1910-1927`, `admit_spawn` at `:1210-1231`) is serialized only
   against scope closure. A pending handle cancellation is invisible to all
   three, so attempt N+1 is spawned, `AttemptStarted { N+1 }` is published
   (`:1948`), and only then does the monitoring loop observe the cancel and
   terminate the new process.
4. The session registry forwards a stop as `handle.terminate()`
   (`src/worktree_ops/session.rs:1116`), which is just the oneshot send
   (`src/process_manager.rs:329`). It has no other lever: closing the
   registry's `RunCommandScope` would stop every session, which the spec
   forbids for a single-session stop.

So the archived test
`remote_contract_gap_session_stop_during_retry_delay_keeps_started_attempt`
(`src/worktree_ops/session/tests.rs:398`) proves only that the *session*
derives `attempt` from `AttemptStarted`; it drives the supervisor through
injected channels and never exercises the runner. No real-process test exists
for a stop that lands in a retry delay or in the stagger wait, for either retry
cause. This proposal closes that one gap and nothing else.

## Proposed Solution

One additive, explicitly opted-in stop policy on the shared retry loop:

- **`StopAdmission` policy on the retry loop.** `execute_streaming_with_retry_sink`
  gains a `StopAdmission` parameter with two values. `SpawnThenCancel` is the
  behaviour at `f64f34d7b` and is what `execute_streaming_with_retry` (the
  text entry point used by the TUI terminal, parallel executor, apply, archive,
  acceptance, and shell paths) keeps passing, unchanged. `RefuseBeforeSpawn` is
  passed only by `execute_streaming_with_retry_observed`, the entry point only
  the session registry calls. No existing caller's default changes.
- **Four cancellation checkpoints under `RefuseBeforeSpawn`.** (a) After an
  attempt's exit and cleanup, before a retry is scheduled (both the crash and
  the inactivity branch): a stop already received schedules nothing. (b) The
  retry delay is a `tokio::select!` between the sleep and `&mut cancel_rx`,
  biased to the cancel. (c) The stagger wait is the same race. (d) Immediately
  before `admit_spawn` / `Command::spawn`, a non-blocking `try_recv` on the
  cancel receiver. Each checkpoint reads the oneshot the way the existing
  monitoring branches do: `Ok(())` is a stop; `Err(Closed)` is a stop for a
  scoped runner (a dropped handle is never permission to detach, exactly as
  `:2165-2169` decides today) and a neutralised no-op for an unscoped one; a
  pending receiver is left armed and never consumed.
- **Typed refusal evidence.** A refused attempt publishes
  `RunnerNotice::StopRefusedAttempt { next_attempt, stage }` with `stage` one
  of `retry`, `stagger`, or `spawn`, retained by the session as the notice
  kind `stop_refused_attempt`. The earlier `retry_scheduled` notice stays. The
  invocation then settles exactly as the existing cancel branch does: the last
  attempt's cleanup report, `CommandTermination::Cancelled`, the writer lease
  settled, the synthetic failure status, and `ScopeExecution::finish`.
  `AttemptStarted` is still published only after a successful spawn, so a
  refused attempt never moves `attempt`.
- **Session evidence.** `classify` already maps `Cancelled` to `stopped`
  (`session.rs:1393-1396`) and reads `exit_code`/`signal` from the last reaped
  leader — for a refusal that is the attempt that actually ran and exited, so
  a crashed attempt 1 publishes its real exit code with `signal: null`.
  `StopOutcome.terminated` keeps its existing formula (`session.rs:1230`):
  `true` only when the state is `stopped` and the last attempt's process-group
  cleanup was confirmed. No wire member is added or removed; `kind` is already
  a free string whose documented vocabulary gains one token.
- **Everything else stays.** A stop while a child runs is still the existing
  SIGTERM → 5-second → SIGKILL cancel branch with verified group cleanup and
  observed signal. Scope shutdown keeps its loop-head check, its pre-delay
  checks, its `admit_spawn` critical section, and its `ShutdownRefused`
  notice; it is not merged with session stop. `CommandTermination`,
  `StreamingChildHandle`, the retry policy, stagger semantics for every
  existing caller, `killpg` authority, and every `cflx client`/MCP verb are
  untouched.

## Acceptance Criteria

1. Against a real process through `run_worktree_command`, with a 60-second
   retry delay and a command that exits non-zero once, a stop submitted after
   the `retry_scheduled` notice is visible settles within the hang guard with
   exactly one PID ever recorded by the command, `attempt: 1`, `state:
   stopped`, `runner_notices` equal to `[retry_scheduled(2),
   stop_refused_attempt(2, stage=retry)]`, no second process's output in
   `output.lines`, `result.state_after: stopped`, and `result.terminated` equal
   to the published `process_group_confirmed` of attempt 1.
2. The same holds when attempt 1 was terminated by the inactivity timeout
   with an inactivity retry budget remaining (`cause=inactivity_timeout` on the
   scheduled notice), with attempt 1's `inactivity_terminated` notice retained
   as well.
3. With the shared stagger state pre-armed and a long stagger delay, a stop
   that lands while attempt 2 is parked in the stagger wait refuses with
   `stage=stagger`; a stop that lands while attempt 2 is blocked on the shared
   stagger lock the test holds — the controlled race at the spawn boundary —
   refuses with `stage=stagger` or `stage=spawn` and, in both cases, spawns
   nothing: one PID, `attempt: 1`.
4. A stop while the leader runs is unchanged: `signal: SIGTERM` for a
   SIGTERM-honouring command and `signal: SIGKILL` after escalation for an
   ignoring one, `attempt: 1`, `terminated: true`, the whole group gone, and
   no `stop_refused_attempt` notice — the archived cases keep passing as the
   negative control for "the cancel branch is not what changed".
5. The text entry point (`execute_streaming_with_retry`) with the same
   command, the same stagger-lock barrier, and `terminate()` during the
   barrier still spawns attempt 2 and then cancels it — two PIDs — and prints
   the same `[Retry 1/N]` stderr line as before: the default policy is
   unchanged and the compatibility control fails if anyone flips it.
6. Scope shutdown during a retry delay and during the stagger wait still
   refuses with `ShutdownRefused { stage: "retry" | "spawn" }` on both
   policies; the existing `run_command_scope_refuses_spawn_after_shutdown` and
   `run_command_scope_suppresses_retry_after_shutdown` pass unchanged.
7. A unit test over the extracted checkpoint helper proves the oneshot
   semantics: pending → proceed with the receiver still armed; `Ok(())` →
   refuse; `Err(Closed)` → refuse when scoped, proceed-and-neutralise when
   unscoped; a second call after a refusal never panics or re-reads.
8. OpenAPI documents `stop_refused_attempt` in the `SessionRunnerNotice.kind`
   description; the CLI export equals the live document; a session resource
   captured from an owner built at `f64f34d7b` decodes unchanged; the full
   `openapi_contract_tests` suite passes.
9. `remote_worktree_session`, `remote_contract_gap`, `remote_text_edit`,
   `remote_confirmed_delete`, and `remote_progress_timing` re-run green with
   nonzero counts, `remote_merge_abort` likewise; clippy is clean; default and
   `web-monitoring` suites report the same or fewer failures than the
   unmodified baseline `10da8ac45`, each pre-existing failure named.

## Explicit Completion Conditions

Every new case carries the test-name prefix `session_retry_stop`; the three
lib gates select it with nonzero passed counts, and a zero-test selection
fails. The contract gate runs the full `openapi_contract_tests` suite. The six
regression gates re-run the existing filters one at a time after integration,
and the build gate runs clippy with the default and `web-monitoring` suites
reported alongside. Every child verification command is run with an explicit
`TMPDIR=/home/tumf/work/conflux-api-provider-20261106-evidence/tmp/<uuid>`
created for that run, because `/tmp` on host asagi refuses writes with
`EDQUOT` under the user's quota while `df` still shows free space; test
results are recorded from the selected count and the exit status of each
command, never inferred from a plan or a wrapper exit. `cflx openspec validate
--strict` and `--archive-gate` are structural checks and are not
implementation PASS. Tasks stay unchecked until the implementation is verified
in a separately authorized execution, and this proposal is not an independent
review of itself; the parent reviews it.

## Scope / Preserved Contracts

Implementation touches `src/ai_command_runner.rs` (`StopAdmission`, the four
checkpoints, `RunnerNotice::StopRefusedAttempt`, the checkpoint helper),
`src/worktree_ops/session.rs` (`apply_notice` arm for the new kind, doc
comments), `src/web/remote_control_api/dto.rs` and `src/web/openapi.rs` (the
`kind` description), focused tests in `src/ai_command_runner.rs`,
`src/worktree_ops/session/tests.rs`,
`src/web/remote_control_api/tests/worktree_session_tests.rs`, and
`tests/openapi_contract_tests.rs`, plus `docs/guides/USAGE.md` and `AGENTS.md`.
Unchanged: `execute_streaming_with_retry` and every consumer of it, the
`SpawnThenCancel` behaviour those consumers get, `execute_streaming`, the
`StreamingChildHandle` API and its oneshot cancel channel, `CommandTermination`,
`RunCommandScope` and `ScopeExecution` shutdown semantics, the 5-second
escalation and `verify_owned_process_group`, the writer ledger, the retry
policy and `should_retry`, `wait_for_stagger_slot` for legacy callers,
`stop_worktree_session`'s wire shape, `force_stop_change`, every `cflx
client`/MCP verb, and every existing wire member. No new dependency, no
Conflux-owned write into any worktree, no kill authority added, no other
session or scheduler stopped by a session stop.

## Out of Scope / Rollout Handoff

Not included: the empty `RunDispatched` clock difference, NAME_MAX handling
in text edit, Server-side UI or relay, improvements to other runners or to the
legacy text path's stop timing, the known baseline failures
(`tests/client_cli_tests.rs::wait_proves_base_publication_only_when_the_remote_matches`,
`tests/client_completion_sink.rs::artifact_is_owner_read_only_by_default_and_is_never_read_back`),
any new feature, push, release, install, deployment, owner stop or restart,
and global configuration or authentication changes. The implementation is
executed later through the existing Herdr-managed cflx owner; this proposal
marks, starts, merges, or stops nothing. The independent review of the
implementation is launched by the implementation worker per the contract in
`design.md`, and the parent owns acceptance.
