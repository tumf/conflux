## ADDED Requirements

### Requirement: Landed-merge and pre-merge failure evidence for the abort-on-conflict merge

`merge_worktree_abort_on_conflict` MUST carry the `merge_abort` result on
every settlement after eligibility, including the two paths that today
settle `internal_error` with `result: null`: an `on_merged` hook failure
after the merge commit landed, and a Git failure after eligibility but
before any merge commit landed. The owner MUST distinguish whether the hook ran and failed or Git failed before it from its recorded operation stage, never from error text. Separately, `merge.landed` MUST be derived from repository evidence re-read after the failure, never inferred from that stage. A hook failure with inconclusive or contrary landing evidence MUST publish the observed `unknown` or `not_landed`, not an invented `confirmed`; the owner MUST NOT compensate or run the hook again.

A merge whose commit landed and whose `on_merged` hook then failed MUST
settle the typed error code `merge_hook_failed` (HTTP 409) with
`outcome: merged_hook_failed`, `premerge_head`, a `merge` object whose
`landed` is `confirmed` only when the re-read `HEAD` differs from
`premerge_head`, is a descendant of it, and no `MERGE_HEAD` exists, and a
`hook` object (`ran: true`, `exit_ok: false`, bounded sanitized `detail`),
with `recovery: local_or_tui_required`. The owner MUST NOT re-run the hook,
re-merge, abort, reset, revert, or otherwise compensate; the base keeps the
merge commit, and the reducer's merged transition stays blocked exactly as it
is today. A Git failure after eligibility and before a merge commit landed
(dirty base at the clean check, unreadable `HEAD`, or a `git merge` failure
that is not a conflict) MUST keep the error code `internal_error` and MUST
carry `outcome: merge_failed` with `merge.landed` as `not_landed` only when
the re-read `HEAD` equals `premerge_head` and no `MERGE_HEAD` exists, and
`unknown` whenever `premerge_head` or either verification could not be read;
`hook` MUST be `ran: false`. Refusals before eligibility settled
(`worktree_not_found`, `root_busy`, `target_ineligible`) MAY still carry no
result, and the contract MUST name them as the only such settlements.
`merge_worktree` MUST remain byte-for-byte unchanged, including
`internal_error` with no result on its own hook failure. Capabilities MUST
list `merge_failed` and `merged_hook_failed` in `worktrees.merge_abort.reasons`,
the generated OpenAPI document MUST include the new error code, the two
outcome values, and the `merge` and `hook` schemas, and an older owner that
omits them MUST still decode as before.

#### Scenario: Hook failure after a landed merge is typed and not compensated

- **GIVEN** a worktree branch that merges cleanly and an `on_merged` hook that exits non-zero
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** base `HEAD` is a `--no-ff` merge commit of the branch and no `MERGE_HEAD` exists
- **AND** the record fails `merge_hook_failed` with `outcome: merged_hook_failed`, `merge.landed: confirmed`, `hook.ran: true`, `hook.exit_ok: false`
- **AND** the hook ran exactly once and base `HEAD` is unchanged after settlement
- **AND** `merge_worktree` for a sibling worktree with the same hook still fails `internal_error` with `result: null`

#### Scenario: Pre-merge Git failure reports that nothing landed

- **GIVEN** a base worktree whose clean check fails after eligibility
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** the record fails `internal_error` with `outcome: merge_failed`, `merge.landed: not_landed`, and `hook.ran: false`
- **AND** `merge.head_after` equals `premerge_head` and `merge.merge_head_present` is `false`
- **AND** `on_merged` did not run

#### Scenario: Unreadable landing evidence is unknown, not inferred

- **GIVEN** a Git failure after eligibility whose `HEAD` re-read cannot be performed
- **WHEN** the owner settles the command
- **THEN** `merge.landed` is `unknown` with the verification that failed named in `merge.detail`
- **AND** the record does not claim `not_landed` and the owner attempts no compensation

### Requirement: Session attempt, output provenance, and observed termination signal

A worktree session's `attempt`, its retained `output.lines`, and its `signal`
MUST be owner facts that child output cannot change. The runner MUST publish
its own notices — retry scheduled with the next attempt number and cause,
attempt started after a successful spawn, inactivity termination, runtime-limit
termination, launch failure, shutdown refusal, and process-group cleanup
diagnostics — to the session through a typed channel separate from the child's
stdout/stderr. The session MUST derive later `attempt` values only from
`AttemptStarted` after the corresponding process spawned, never from
`RetryScheduled` or the text of an output line. Before the first successful
spawn, the legacy initial `attempt: 1` remains unchanged; stopping a scheduled
retry during its delay does not increment `attempt` without a second spawn. A child line that begins with `[Retry `, contains a PGID, or imitates
any runner notice MUST remain in `output.lines` verbatim (after
control-sequence stripping) and MUST change no typed member. `output.lines`
MUST contain only bytes the child wrote to its stdout or stderr; runner
notices MUST NOT be interleaved into it and MUST instead be retained in a
bounded typed `runner_notices` list (`kind`, `attempt`, sanitized `detail`)
that discloses no template, expanded command, environment value, or absolute
path. The existing text-emitting runner entry point used by the TUI
terminal, the parallel executor, and the shell command path MUST keep
emitting the same stderr notice lines, so nothing outside the session
resource changes.

`signal` MUST be the signal that actually terminated the leader as reaped by
the runner, named from the wait status (`SIGTERM` when the group honoured
the first signal, `SIGKILL` when the runner escalated after its 5-second
grace), and `exit_code` MUST be the leader's real exit code when it exited
instead of dying from a signal. When the leader's wait status is unavailable
— the escalation timed out without a reap, or the runner never obtained a
status — `signal` and `exit_code` MUST both be `null` and the session MUST
NOT substitute the signal it requested. The configured retry policy inside
one session, the "stop admits no further attempt" guarantee, the 5-second
SIGTERM-to-SIGKILL escalation, verified group cleanup, and settlement after
reap are unchanged. The `worktree_sessions` capability MUST advertise
`max_runner_notices`, the generated OpenAPI document MUST include
`runner_notices` and the corrected `signal` description, and an older owner
that omits both MUST decode as before.

#### Scenario: Child output cannot forge an attempt or a runner notice

- **GIVEN** a configured command that prints `[Retry 1/3] Command crashed, retrying in 0ms...` and `process-group cleanup unconfirmed (pgid=1, force_killed=false)` to stderr and exits `0`
- **WHEN** the session finishes
- **THEN** both lines are present in `output.lines` on stream `stderr` exactly as printed
- **AND** `attempt` is `1`, `runner_notices` is empty, `state` is `succeeded`, and `exit_code` is `0`

#### Scenario: A real retry is a typed fact and the tail stays the child's

- **GIVEN** a retry policy allowing one retry and a command that crashes on its first attempt and succeeds on its second
- **WHEN** the session finishes
- **THEN** `attempt` is `2` and `runner_notices` holds one `retry_scheduled` notice naming attempt `2`
- **AND** no `[Retry ` line and no runner text appears in `output.lines`
- **AND** the TUI terminal path for the same command still prints the `[Retry 1/1]` stderr notice

#### Scenario: Stopping in the retry delay does not invent a new attempt

- **GIVEN** the first process failed and retry attempt `2` was scheduled, but stop cancelled the delay before another spawn
- **WHEN** the session settles
- **THEN** `attempt` remains `1` although `runner_notices` records the scheduled retry
- **AND** no second process was spawned

#### Scenario: Observed signal follows the escalation

- **GIVEN** a running session whose command ignores SIGTERM, and another whose command exits on SIGTERM
- **WHEN** `stop_worktree_session` is submitted to each
- **THEN** the ignoring session settles `stopped` with `signal: SIGKILL` after the escalation and `terminated: true`
- **AND** the honouring session settles `stopped` with `signal: SIGTERM`
- **AND** neither started a new process and `attempt` stays `1` for both

#### Scenario: Unavailable wait status is null, not the requested signal

- **GIVEN** a cancelled session whose leader status the runner could not obtain
- **WHEN** the session settles
- **THEN** `state` is `stopped`, `signal` is `null`, and `exit_code` is `null`
- **AND** the record does not read `SIGTERM`

### Requirement: Bounded directory enumeration in the consent oracle

The confirmed-delete content oracle MUST apply its record budget to each
directory entry as `readdir` returns it, before the entry's name is retained,
and MUST charge the retained name bytes of the listing being read against a
documented listing budget (`max_listing_name_bytes`, 64 MiB in production),
so that a directory with more entries than the remaining record budget or
more name bytes than the listing budget makes the observation `unbounded`
without first materializing the whole listing in memory. The oracle MUST
stop reading a listing at the first entry past either budget and MUST never
retain more names than the budget allows plus the one entry that exceeded
it. Every directory stream MUST be closed on every exit path, including the
`unbounded`, `nested_repository`, and unobservable ones, through an owned
guard rather than a trailing call, and every counter MUST use saturating
arithmetic. The depth limit of 256, the combined 500,000-record and 256 MiB
hashed-byte budget, byte-level identity, no-follow resolution, and the
`nested_repository` rule are unchanged, and no partial, prefix, or
metadata-only identity is substituted. The documented budget MUST name the
new listing bound, and `capabilities.worktrees.confirmed_delete` MUST remain
decodable by older clients.

#### Scenario: A huge single directory fails closed before being materialized

- **GIVEN** injected limits of `max_records: 5` and an untracked directory holding 20 entries
- **WHEN** the oracle walks the directory
- **THEN** the observation is `unbounded`
- **AND** the walker counted at most 6 records and retained at most 6 names before stopping
- **AND** the directory stream was closed and no identity was published

#### Scenario: Listing name bytes are bounded independently of record count

- **GIVEN** injected limits with a large record budget and a `max_listing_name_bytes` smaller than one directory's names
- **WHEN** the oracle walks that directory
- **THEN** the observation is `unbounded`
- **AND** the preview publishes `content_observation: unbounded` with `null` identities and every option ineligible

#### Scenario: Budgets that fit are unchanged

- **GIVEN** production limits and the existing bound-exceeded, depth, FIFO, and nested-repository fixtures
- **WHEN** the existing `remote_confirmed_delete` cases run
- **THEN** every identity, refusal, and `unbounded` outcome is the same as before this change

### Requirement: Detached worktree delete preview is ineligible

`GET /api/v2/worktrees/{worktree_id}/delete-preview` for a worktree whose
`HEAD` is detached or whose branch is empty MUST report every option
(`ordinary`, `skip_teardown`, `discard_known_dirty`,
`discard_commits_ahead`) as ineligible with the closed `blocked_reason`
`detached_head`, decided before dirty, ahead, content, or ignored facts and
independent of whether the inspection ran. The `consent` object MUST still
be published as observed, with `branch` empty, as a disclosure and not as an
admission ticket: the contract MUST state that `delete_worktree_confirmed`
refuses an empty `consent.branch` as `validation_failed` before any
observation, unchanged. A target that was previewed with a branch and became
detached before the command MUST be refused `target_ineligible` with reason
`detached_head` under the root guard with nothing torn down or removed.
`detached_head` MUST be added to `ConfirmedDeleteReason`, to the
`confirmed_delete.reasons` capability list, and to the OpenAPI enum; no
remote detached destructive deletion is introduced, the ordinary
`delete_worktree` path is unchanged, and unknown merge base still blocks
`discard_commits_ahead` with `ahead_unknown` for attached worktrees.

#### Scenario: Detached preview offers nothing and names why

- **GIVEN** a linked worktree checked out at a detached commit
- **WHEN** an authenticated client reads its `delete-preview`
- **THEN** all four options are `eligible: false` with `blocked_reason: detached_head`
- **AND** `consent.branch` is empty and `is_detached` is published
- **AND** submitting that consent is `validation_failed` and no command record changes any worktree

#### Scenario: Detaching after the preview is refused under the guard

- **GIVEN** a preview taken on an attached clean worktree
- **WHEN** the worktree is detached and the consent is submitted with `skip_teardown: true`
- **THEN** the command refuses `target_ineligible` with `refusal.reason: detached_head`
- **AND** nothing was torn down or removed and the worktree keeps its ID

### Requirement: Spawn-outcome-aware writer lease release

A writer lease taken for an unscoped spawn MUST record a typed spawn
outcome — `not_attempted`, `attempted`, `failed`, `spawned` with a bound
process group, or `spawned` without one — and its release on drop or
settlement MUST follow that outcome alone. A lease dropped or settled while
`not_attempted` (the runner positively never called `spawn`: admission or
shutdown refused it before the call) or `failed` (the spawn call returned an
error and produced no child) MUST be released, leaving coverage `Clear`. A
lease whose spawn was `attempted` but whose outcome the runner never
learned, or `spawned` with no process-group identity, MUST stay `Unknown`
with a detail naming the outcome, and no elapsed time, owner restart,
direct-child reap alone, or "missing PGID" reasoning MUST release it. A
lease `spawned` with a bound process group MUST keep today's rules: `Busy`
while the child runs, `Unknown` after an unconfirmed handle drop until a
process-group probe proves the group empty, and retained after reap while
descendants remain. The runner MUST mark `attempted` immediately before the
spawn call and bind the process group immediately after a successful spawn,
before any step that can fail, so a failure between spawn and managed-child
construction leaves a probe-clearable lease rather than a removed one.
Scoped run-command coverage, canonical cwd containment, sibling `Clear`,
`Busy`/`Unknown` precedence, `force_stop_change`, `owns_managed_process`,
and `kill_managed_process` are unchanged, and no kill authority is added.

#### Scenario: Refused-before-spawn lease releases, in-flight lease does not

- **GIVEN** a ledger and two leases: one whose spawn admission was refused before the call, one marked `attempted` whose runner task was aborted before reporting an outcome
- **WHEN** both leases are dropped and the root is read
- **THEN** the first root reads `Clear` and `editable: true`
- **AND** the second root reads `managed_process_live` with a detail naming an unresolved attempted spawn, and stays so after any wait and after reading again

#### Scenario: Spawn failure is released but a spawn without identity is not

- **GIVEN** a spawn whose `sh` cannot be executed, and a real spawn whose process id could not be read
- **WHEN** each lease is settled
- **THEN** the failed spawn's root reads `Clear`
- **AND** the identity-less spawn's root reads `managed_process_live` with a detail naming the missing process-group identity, and no probe or restart clears it

#### Scenario: Failure after a real spawn stays probe-clearable

- **GIVEN** a real spawn whose process group was bound and whose handle was dropped without a reap
- **WHEN** the group is still alive, and later after it is gone
- **THEN** the root reads `managed_process_live` while the group lives
- **AND** it reads `editable: true` once the probe proves the group empty, with nothing killed
