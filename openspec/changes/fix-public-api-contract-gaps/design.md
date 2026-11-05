# Design: fix public API contract gaps

## Source evidence at `feature/public-api-provider-extensions` `10da8ac45`

Line numbers are from the integrated commit the verification document
re-ran; the implementer re-reads them before editing.

### Gap 1 — abort-on-conflict merge, hook failure after a landed merge

- `MergeAbortOutcome { Merged, PredictedConflict, ConflictAborted, AbortFailed }`
  and `MergeAbortResult { worktree_id, branch, outcome, premerge_head,
  conflict_files, abort, recovery }` (`src/web/remote_control_api/dto.rs:2770-2867`).
  `CommandResult::MergeAbort` doc says the result is present "on every
  refusal after eligibility" (`dto.rs:2573-2579`).
- `merge_abort_failure_result` maps only `PredictedConflict` and
  `MergeConflictAborted`; `_ => None` (`worktrees.rs:762-800`), and the
  record keeps `failure.result = None` (`commands.rs:268`, `registry.rs:166`).
- `WorktreeService::merge_worktree` (`service.rs:1366-1474`): eligibility
  (`classify_merge_eligibility`, `711-772`), backend merge
  (`git_backend.rs:194-223`; non-conflict `Err` → `Internal` at `208`),
  conflict handling (`1406-1420`), then `run_on_merged`; a hook failure emits
  `MergeFailed` and returns `WorktreeOpError::Internal("on_merged hook failed
  for '{change_id}'; branch merged transition blocked: {error}")`
  (`1439-1460`). `map_worktree_error` sends `Internal` to `internal_error`
  (`worktrees.rs:758`).
- `aborting_merge_with` reads `premerge_head` before `git merge --no-ff
  --no-edit` (`src/vcs/git/commands/merge.rs:572-668`); the verified
  restoration re-reads `HEAD`, `MERGE_HEAD`, and cleanliness (`645-655`).
  Those three reads are the same primitives landing evidence needs.
- Capability `MERGE_ABORT_REASONS = ["predicted_conflict", "conflict_aborted",
  "abort_failed", "restore_unknown"]` (`worktrees.rs:68-73`); `ErrorCode` has
  29 values with `ALL_ERROR_CODES` and `http_status` (`dto.rs:43-253`).
- The only hook-failure test is
  `remote_worktree_on_merged_failure_blocks_the_merged_transition`
  (`src/worktree_ops/service/tests.rs:1920`), on `PreserveConflict`.

### Gaps 2 and 3 — session attempt, output provenance, signal

- `is_retry_notification(line) = line.starts_with("[Retry ")` bumps
  `attempt` on any stderr line (`src/worktree_ops/session.rs:1088-1103,
  1213-1216`). `OutputStream { Stdout, Stderr }` is the only tag (`542-559`).
- `classify` returns `Some("SIGTERM")` for `Cancelled` and `RuntimeLimit`
  (`session.rs:1222-1244`); `SessionSnapshot.signal` doc admits "the signal
  the runner's cancellation path sends first" (`637-641`).
- Runner notices sent as `OutputLine::Stderr` on the one output channel:
  `shutdown_refusal_line` (`ai_command_runner.rs:1582, 1637, 2620`),
  `launch_failure_line` (`1662, 1684`), `runtime_limit_line` (`1908, 2188`),
  inactivity messages (`1994-2004, 2325-2332`), `[Retry N/M]` (`2318, 2376`),
  and `ProcessGroupCleanupReport::diagnostics()` with `pgid=` when cleanup is
  unconfirmed (`2697-2724`; `process_manager.rs:697-712`).
- Cancel branch: `let _ = managed_child.terminate_with_timeout(5s)`,
  `publish_termination(Cancelled)`, synthetic `make_fail_status()`
  (`ai_command_runner.rs:1842-1888, 2746-2758`). `terminate_with_timeout`
  returns `TerminationOutcome::{Exited(status), ForceKilled(status),
  TimedOut}` with the real reaped status (`process_manager.rs:63-67,
  169-185, 388-433`).
- Consumers of the text entry point that must not change: TUI terminal
  (`src/tui/terminal.rs:32-95`, `println!`/`eprintln!` per line), parallel
  executor and output bridge (`src/parallel/executor.rs:874, 1183, 1380`,
  `src/parallel/output_bridge.rs:118`), TUI key handlers
  (`src/tui/key_handlers.rs:128, 428, 465`).
- Session launch: one `execute_streaming_with_retry(&spec.command,
  Some(&spec.path), Some("worktree_session"), None)` per session
  (`session.rs:978-986`); `supervise` forwards every received line to
  `record_output` (`1023-1086`).

### Gap 4 — consent walk materializes a listing

- `walk` checks depth, then `read_dir_names(dir)` into a `Vec`, sorts, and
  only then `count_record()` per entry (`src/worktree_ops/consent.rs:677-705`).
- `read_dir_names` dups the fd, `fdopendir`, `std::mem::forget(duplicate)`,
  pushes every name, trailing `closedir` (`876-910`). `ConsentLimits {
  max_records: 500_000, max_hashed_bytes: 256 MiB }` (`68-88`),
  `MAX_WALK_DEPTH = 256` (`433`), `count_record`/`charge_bytes` (`519-533`),
  `Stop { Unobservable, Unbounded, Nested }` (`300-304`).
- Injected-limit tests already exist:
  `remote_confirmed_delete_bound_exceeded_fails_closed`
  (`consent_tests.rs:429`) with `observe_with(root, &limits)` (`62`).

### Gap 5 — detached preview

- `delete_preview` (`src/worktree_ops/service/confirmed.rs:355-413`) and
  `classify_confirmed_delete` (`41-75`: main → root busy → content → ahead
  unknown → dirty unknown → ahead → dirty → ignored) never read
  `facts.is_detached` or an empty `facts.branch`.
- `ConfirmedDeleteReason` is a closed 19-value enum with `ALL` and `as_str`
  (`consent.rs:972-1062`); none names a detached head.
- `ConfirmedDeleteParams::validate()` refuses blank `consent.branch` as
  `validation_failed` (`dto.rs:609`), before observation.
- On the real backend, inspection skips detached worktrees
  (`src/worktree_ops/inspection.rs:144, 170-171`), so every option reads
  `ahead_unknown` by accident of inspection, not by classification.

### Gap 6 — writer lease release

- `WriterLease { ledger, id, settled }`, `bind_pgid` (no-op for `0`),
  `release_unspawned`, `retain_unresolved`, `settle_after_reap` (removes a
  lease with empty `pgids` as "never spawned"), `Drop` → `Unresolved`
  (`src/ai_command_runner.rs:979-1058`). `coverage_under` clears an
  `Unresolved` lease only when every bound PGID probes empty (`900-914`).
- `execute_streaming_with_retry`: lease taken before `tokio::spawn`
  (`1519-1535`); admission refusal / spawn `Err` / `ManagedChild::new` failure
  all `break 'retry` (`1633-1692`); `bind_pgid` only after `ManagedChild::new`
  (`1697-1701`); `settle_writer_lease` at the end (`2403, 2730`).
- `execute_streaming` already distinguishes `release_unspawned` on spawn
  `Err` and `retain_unresolved` when `child.id()` is `None` (`2453-2502`).
- `process_group_is_empty` probes with `killpg(pgid, 0)` (`process_manager.rs:1068-1098`).

## Contract

### 1. Merge landing and hook evidence (additive)

```text
MergeAbortOutcome  += merged_hook_failed | merge_failed
MergeAbortResult   += merge: MergeLandingEvidence | null, hook: MergeHookEvidence | null
MergeLandingEvidence { landed: confirmed | not_landed | unknown,
                       head_after: string|null, merge_head_present: bool|null,
                       clean_after: bool|null, detail: string|null }
MergeHookEvidence    { ran: bool, exit_ok: bool|null, detail: string|null }
ErrorCode          += merge_hook_failed (HTTP 409)
MERGE_ABORT_REASONS += "merge_failed", "merged_hook_failed"
```

| settlement | error code | `outcome` | `merge.landed` | `hook` |
|---|---|---|---|---|
| merged, hook ok | — (`succeeded`) | `merged` | `confirmed` | `ran: true, exit_ok: true` |
| merged, hook failed | `merge_hook_failed` | `merged_hook_failed` | `confirmed` (re-read) | `ran: true, exit_ok: false, detail` |
| Git failure after eligibility, before a merge commit | `internal_error` | `merge_failed` | `not_landed` or `unknown` (re-read) | `ran: false` |
| predicted conflict | `merge_conflict_predicted` | `predicted_conflict` | `not_landed` | `ran: false` |
| real conflict, aborted / abort failed | unchanged | unchanged | `not_landed` / per `abort` | `ran: false` |
| before eligibility (`worktree_not_found`, `root_busy`, `target_ineligible`) | unchanged | result `null` | — | — |

`landed: confirmed` requires all of: re-read `HEAD != premerge_head`,
`git merge-base --is-ancestor premerge_head HEAD` true, and no `MERGE_HEAD`.
`not_landed` requires `HEAD == premerge_head` and no `MERGE_HEAD`. Any
unreadable fact is `unknown` with the failed verification named in
`detail`. `clean_after` is informational and never changes `landed`.

Service: two new `WorktreeOpError` variants, `MergedHookFailed { change_id,
premerge_head, landing: LandingReport, hook_detail }` and `MergeFailed {
premerge_head: Option<String>, landing: LandingReport, detail }`, with
`Display` producing exactly today's `Internal` messages so TUI text and the
`MergeFailed` event are unchanged. `map_worktree_error` maps
`MergedHookFailed` to `merge_hook_failed` and `MergeFailed` to
`internal_error`; `merge_abort_failure_result` builds the result for both.
For `merge_worktree` (PreserveConflict) `RemoteWorktreeOperations::merge`
maps both variants to `internal_error` with no result, byte-for-byte as
today, and a `remote_contract_gap` test proves it. The TUI's abort path
prints the same operator text as before. No code path re-runs the hook,
re-merges, aborts, resets, or reverts.

### 2 and 3. Typed runner notices, child-only tail, observed signal

```rust
pub enum RunnerNotice {
    RetryScheduled { next_attempt: u32, max_attempts: u32, cause: RetryCause, delay_ms: u64 },
    InactivityTerminated { attempt: u32, timeout_secs: u64 },
    RuntimeLimitReached { attempt: u32 },
    LaunchFailed { attempt: u32, stage: &'static str },
    ShutdownRefused { stage: &'static str },
    ProcessGroupCleanupUnconfirmed { pgid: u32, force_killed: bool },
    LeaderReaped { attempt: u32, exit_code: Option<i32>, signal: Option<i32> },
    LeaderNotReaped { attempt: u32 },
}
pub enum RetryCause { Crash, InactivityTimeout }
```

- `AiCommandRunner::execute_streaming_with_retry_observed(command, cwd,
  operation, change_id) -> (StreamingChildHandle, Receiver<OutputLine>,
  Receiver<RunnerNotice>)`. The shared retry loop takes a `NoticeSink`:
  `Text(out_tx)` (existing behavior, every notice also formatted as the
  existing stderr line, byte-for-byte) or `Typed(notice_tx)` (notices only on
  the typed channel, nothing runner-generated on `out_tx`). The existing
  `execute_streaming_with_retry` is `Text`; only the session registry uses
  `Typed`. `test_inactivity_timeout_retry` and the TUI terminal keep passing
  unchanged.
- In every termination branch that calls `terminate_with_timeout`, the
  runner publishes `LeaderReaped` from `TerminationOutcome::Exited(status) |
  ForceKilled(status)` using `ExitStatusExt::signal()`/`code()`, and
  `LeaderNotReaped` for `TimedOut`. The `Exited | InactivityTimeout`
  branches publish `LeaderReaped` from the real wait status. The synthetic
  `make_fail_status()` sent on `status_tx` is untouched (other consumers read
  it); the session ignores it for cancelled runs.
- Session registry: `attempt = 1`, set to `next_attempt` on each
  `RetryScheduled`; `runner_notices: Vec<RunnerNoticeRecord { kind,
  attempt, detail }>` bounded at 32 (oldest dropped, a `dropped` count kept),
  `detail` passed through `sanitize_detail` and never containing the
  template, expanded command, environment, or an absolute path (the PGID
  and attempt numbers are not secrets). `record_output` stores every
  `OutputLine` as child data; `is_retry_notification` is deleted. `classify`
  takes the last `LeaderReaped`/`LeaderNotReaped` for the final attempt:
  `Cancelled`/`RuntimeLimit` → `Stopped`/`Failed` with `exit_code` and
  `signal` from the reaped status, both `null` when not reaped. `supervise`
  drains the notice channel until the runner closes it before finalizing,
  so classification never races a late notice.
- Wire: `WorktreeSessionResource += runner_notices: SessionRunnerNotice[]`
  (`kind`, `attempt`, `detail`), `signal` description rewritten to "the
  signal that terminated the leader as reaped; `null` when unknown",
  `SessionOutputLine` description "bytes the child wrote". Capability
  `worktree_sessions` gains `max_runner_notices: 32`. Older owners omit both;
  the client decodes absence as "unpublished".

### 4. Budgeted `readdir`

- `ConsentLimits += max_listing_name_bytes: u64` (production 64 MiB;
  `PRODUCTION` and `Default` updated; the documented budget sentence gains
  "and 64 MiB of directory-entry names per listing").
- `read_dir_names(dir, walker: &mut Walker) -> Result<Vec<Vec<u8>>, Stop>`:
  a `DirStream` guard owns the `DIR*` (`fdopendir` on the dup, `closedir` in
  `Drop`, the dup fd forgotten only after `fdopendir` succeeded). Per
  `readdir` entry, skipping `.`/`..`: `walker.count_record()?` first, then
  `listing_bytes = listing_bytes.saturating_add(name.len())` checked against
  `max_listing_name_bytes` → `Stop::Unbounded`, then `names.push`. `walk` no
  longer calls `count_record` per name (the listing already counted them);
  nested `.git` detection, sort, `leaf_at`, and recursion are unchanged.
- The test-visible invariant: on `Err(Stop::Unbounded)` from a listing,
  `walker.records <= limits.max_records + 1` and the retained `names.len()
  <= limits.max_records - records_before + 1`. `Walker` exposes `records`
  and the returned partial `names` length to the `#[path]` test module only.
- Overflow: every counter uses `saturating_add`; `records` is `usize`,
  byte counters are `u64`.

### 5. `detached_head`

- `ConfirmedDeleteReason::DetachedHead` (`detached_head`), `ALL` becomes 20,
  `as_str` extended, `is_stale` unchanged.
- `classify_confirmed_delete` and `classify_consent` return `DetachedHead`
  when `facts.is_detached || facts.branch.is_empty()`, checked first (before
  `is_main`), so the preview reports it for all four options and the command
  refuses `target_ineligible` with `refusal.reason: detached_head` under the
  guard regardless of permissions. `delete_preview` adds nothing else;
  `consent.branch` stays the observed empty string and the response's
  existing `is_detached` fact is published. `validate()` is unchanged.
  `CONFIRMED_DELETE_REASONS` gains `detached_head`.
- `classify_delete_eligibility` (ordinary path) is untouched.

### 6. Spawn-outcome lease

```rust
enum SpawnOutcome { NotAttempted, Attempted, Failed, Spawned { pgid: Option<u32> } }
```

- `LeaseEntry += spawn: SpawnOutcome` (initially `NotAttempted`).
  `WriterLease` gains `mark_attempted()`, `mark_failed()`, and `bind_pgid`
  sets `Spawned { pgid: Some(pgid) }` for non-zero or `Spawned { pgid: None }`
  for `0`/`None`. `release_unspawned` is allowed only from `NotAttempted` or
  `Failed` and is otherwise a no-op that leaves the entry.
- `Drop`: `NotAttempted | Failed` → remove entry; `Attempted` →
  `Unresolved { detail: "spawn was attempted and its outcome was never
  reported" }`; `Spawned { pgid: None }` → `Unresolved { "spawned without a
  process-group identity" }`; `Spawned { pgid: Some }` → today's
  `Unresolved { "released without a reap" }` (probe-clearable).
- `settle_after_reap`: `NotAttempted | Failed` → remove; `Spawned { pgid:
  Some }` → today's quiescence rule; `Attempted | Spawned { pgid: None }` →
  retain `Unresolved` (this replaces the "empty pgids means never spawned"
  removal).
- `coverage_under` is unchanged: an `Unresolved` lease without a bound PGID
  is `Unknown`; one with PGIDs clears only when every group probes empty.
- Runner call sites: `execute_streaming_with_retry` marks `Attempted`
  immediately before `Command::spawn`, `Failed` on `Err`, and binds the PGID
  from `child.id()` immediately after `Ok` and before `ManagedChild::new`;
  admission and shutdown refusals before the spawn call leave the lease
  `NotAttempted` so `settle_writer_lease` removes it. `execute_streaming`
  gains the same marks. The scoped (`RunCommandScope`) path is unchanged.

### Constitution check

Every new fact is process-local, re-read from the repository or the
process table at the moment it is published, and never persisted: landing
evidence is three Git reads after a failure, runner notices and leases die
with the owner, and the consent budget is a limit, not state. Nothing here
decides resume, acceptance, or archive routing; nothing is written into any
target worktree; no Conflux-owned path is excluded from any check. Truthful
completion is the point of the change: `landed`, `signal`, `attempt`, and
lease release are each derived from evidence the owner observed, and
`unknown`/`null`/`Unknown` are published whenever that evidence is missing.

## Rejected alternatives

- **Narrowing "every refusal after eligibility" in the spec** to exclude
  hook and Git failures: it would make the gap disappear on paper while the
  client still cannot tell a landed merge from nothing. Rejected.
- **Reusing `internal_error` for the hook failure with only a result
  attached**: the Server relay switches on `error.code` first and a 500 is
  read as an owner fault; the base did move, which is a 409-class state. A
  new additive code is the smallest honest signal.
- **Changing `merge_worktree` to the same typed result**: its contract says
  unchanged; it stays `internal_error` with `result: null`.
- **A third `OutputLine` variant for runner notices**: every exhaustive
  `match` in the TUI terminal, parallel executor, and bridge would change,
  which is the general refactor this change excludes. A second entry point
  with a `NoticeSink` leaves them untouched.
- **Tagging runner lines with a marker in the text**: a child can print the
  marker. Forgeability is the bug being fixed.
- **Changing `CommandTermination::Cancelled` to carry the reaped status**:
  exhaustive matches in `process_manager` and the parallel executor would
  change; the notice channel carries the same fact additively.
- **Charging listing name bytes to `max_hashed_bytes`**: it would change
  when existing fixtures hit the byte budget and muddle "hashed bytes". A
  separate documented bound is additive and testable.
- **Reusing `ahead_unknown` or `branch_changed` for a detached worktree**:
  neither is the fact. Adding `detached_head` keeps the vocabulary closed and
  honest. This is the one deviation from "existing reason only" the proposal
  takes, and it is called out for the parent's decision.
- **Relaxing `validate()` to accept an empty `consent.branch`**: that is
  detached destructive support, explicitly excluded.
- **Releasing a lease after a quiescence timer, on owner restart, or when
  no PGID was bound**: the spec forbids all three; the only new releases are
  for outcomes the runner positively knows (`NotAttempted`, `Failed`).

## Verification design (future implementation, not executed at proposal time)

Every new test name starts with `remote_contract_gap`. Real-Git and
real-process cases use `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`
and the existing isolated fixtures (`GIT_CONFIG_GLOBAL=/dev/null`,
`TempDir`, repository-local config). Fast cases stay under one second. Hang
guards are generous timeouts, never elapsed-time assertions; liveness and
ordering are proven with channels and state transitions. Mocks never decide
process safety: signal, escalation, and lease cases spawn real children.

### Gate `remote-contract-gap-merge` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/contract_gap_merge_tests.rs`,
registered in `tests/mod.rs`, using the `merge_abort_tests.rs` fixture whose
`on_merged` hook counts runs into a file:

- `remote_contract_gap_merge_hook_failure_after_landed_merge_is_typed`:
  hook exits 1; assert `merge_hook_failed`, `outcome: merged_hook_failed`,
  `merge.landed: confirmed`, `merge.head_after` is a merge commit whose
  first parent is `premerge_head`, `merge_head_present: false`, `hook.ran:
  true`, `hook.exit_ok: false`, hook counter `1`, base `HEAD` unchanged after
  settlement, `recovery: local_or_tui_required`.
- `remote_contract_gap_merge_worktree_hook_failure_is_unchanged`: same hook
  on `merge_worktree`; assert `internal_error`, `result: null`, hook counter
  `1`. Negative control for "do not touch `merge_worktree`".
- `remote_contract_gap_merge_pre_merge_failure_reports_not_landed`: dirty
  base after eligibility; assert `internal_error`, `outcome: merge_failed`,
  `merge.landed: not_landed`, `head_after == premerge_head`, `hook.ran:
  false`, hook counter `0`.
- `remote_contract_gap_merge_unknown_landing_is_not_inferred`: fake backend
  whose landing re-read fails; assert `merge.landed: unknown`, `detail`
  names the failed read, no `not_landed`.
- `remote_contract_gap_merge_precheck_refusals_keep_null_result`:
  `worktree_not_found`, `root_busy`, `target_ineligible` still `result:
  null` — proves the "before eligibility" wording is implemented, not
  widened.
- `remote_contract_gap_merge_capability_lists_new_reasons`.
- Service level (`src/worktree_ops/service/tests.rs`):
  `remote_contract_gap_merge_hook_failure_is_a_typed_variant_with_same_display`
  asserts the new variants' `Display` equals the previous `Internal` text.

### Gate `remote-contract-gap-session` (`src/worktree_ops/session/tests.rs`)

Unit (fast): `remote_contract_gap_session_attempt_comes_only_from_notices`
feeds `OutputLine::Stderr("[Retry 1/3] …")` and a fake PGID line into
`record_output` and asserts `attempt == 1` and both lines retained;
`remote_contract_gap_session_classify_uses_reaped_status` covers
`Cancelled + LeaderReaped(signal 15)` → `SIGTERM`, `(signal 9)` → `SIGKILL`,
`(code 0)` → `exit_code 0`, `Cancelled + LeaderNotReaped` → both `null`,
and `RuntimeLimit` likewise; `remote_contract_gap_session_notice_ring_is_bounded`.

Integration (heavy, `src/web/remote_control_api/tests/worktree_session_tests.rs`,
real `sh` children):

- `remote_contract_gap_session_child_cannot_forge_attempt_or_notice`: command
  prints the retry prefix and `process-group cleanup unconfirmed (pgid=1,
  force_killed=false)`; assert `attempt 1`, `runner_notices: []`, both
  lines in `output.lines` on `stderr`, `exit_code 0`.
- `remote_contract_gap_session_real_retry_is_typed_and_tail_is_child_only`:
  retry policy 1, command crashes once (marker file), then succeeds; assert
  `attempt 2`, one `retry_scheduled` with `attempt 2`, no `[Retry` in the
  tail; then the same command through `execute_streaming_with_retry` (text
  entry point) yields a `[Retry 1/1]` stderr line — the compatibility
  control.
- `remote_contract_gap_session_stop_reports_escalated_signal`: command
  `trap '' TERM; sleep 60` in the group; stop; assert `stopped`, `signal:
  SIGKILL`, `terminated: true`, `attempt 1`, `killpg(pgid, 0)` is `ESRCH`.
- `remote_contract_gap_session_stop_reports_honoured_signal`: `sleep 60`;
  assert `signal: SIGTERM`.
- `remote_contract_gap_session_unreaped_leader_is_null`: a runner double
  that publishes `LeaderNotReaped`; assert `signal: null`, `exit_code: null`
  (this one may use a fake runner because it tests the session's honesty,
  not process safety).
- `remote_contract_gap_session_notices_disclose_no_path_or_template`:
  template with a credential and absolute path; assert no notice `detail`
  contains either.

### Gate `remote-contract-gap-delete` (`src/worktree_ops/consent_tests.rs`)

- `remote_contract_gap_delete_single_directory_is_bounded_before_materialization`:
  20-entry untracked directory, `ConsentLimits { max_records: 5, .. }`;
  assert `Unbounded`, `walker.records <= 6`, retained names `<= 6`, and
  (via `/proc/self/fd` count before/after on Linux) no leaked directory
  descriptor.
- `remote_contract_gap_delete_listing_name_bytes_are_bounded`: large record
  budget, `max_listing_name_bytes` below one directory's names → `Unbounded`;
  the API preview (`confirmed_delete_tests.rs`) shows `unbounded`, `null`
  identities, all options ineligible.
- `remote_contract_gap_delete_dir_stream_closes_on_nested_and_unobservable`:
  `.git` descendant and an unreadable directory each leave the fd count
  unchanged.
- `remote_contract_gap_delete_production_limits_name_the_listing_budget`.
- `remote_contract_gap_delete_detached_preview_is_ineligible` (API, real
  Git): detached linked worktree → four `detached_head`, `is_detached: true`,
  `consent.branch == ""`, echoing consent is `validation_failed`, no record
  changed anything.
- `remote_contract_gap_delete_detach_after_preview_is_refused_under_guard`:
  attached preview, `git checkout --detach` in the worktree, submit
  `skip_teardown: true` → `target_ineligible`, `refusal.reason:
  detached_head`, worktree present, ID retained.
- `remote_contract_gap_delete_attached_ahead_unknown_is_unchanged`: negative
  control that an attached worktree without merge base still reports
  `ahead_unknown` for `discard_commits_ahead`.

### Gate `remote-contract-gap-ledger` (`src/ai_command_runner.rs`)

Unit (fast, the `ManagedWriterLedger` test module):

- `remote_contract_gap_ledger_not_attempted_lease_releases_on_drop`.
- `remote_contract_gap_ledger_attempted_lease_stays_unknown`: mark
  attempted, drop, assert `Unknown` with the typed detail, again after a
  generous `sleep`, again after re-reading (no timer release).
- `remote_contract_gap_ledger_failed_spawn_releases`.
- `remote_contract_gap_ledger_spawned_without_pgid_stays_unknown`: bind
  `0`, settle, assert `Unknown`; negative control for the removed "empty
  pgids means never spawned" rule.
- `remote_contract_gap_ledger_bound_group_drop_is_probe_clearable`: real
  `sh -c 'sleep 30'` child in its own group, bind, drop handle without wait,
  assert `Unknown` while alive; `kill -KILL` the group from the test and reap;
  assert `Clear` with nothing killed by the ledger.
- `remote_contract_gap_ledger_release_unspawned_after_attempt_is_noop`.

Integration (heavy, `src/web/remote_control_api/tests/text_edit_tests.rs`):
`remote_contract_gap_ledger_text_edit_reports_typed_unknown_details` reads
`edit_blocked_detail` for the attempted and identity-less cases and
`editable: true` for the released ones; sibling roots stay `Clear`.

### Gate `remote-contract-gap-contract` (`tests/openapi_contract_tests.rs`)

- `remote_contract_gap_contract_is_published_end_to_end`: `merge_hook_failed`
  in the error enum, `merged_hook_failed`/`merge_failed` in
  `MergeAbortOutcome`, `MergeLandingEvidence`/`MergeHookEvidence` schemas,
  `runner_notices` and `SessionRunnerNotice`, the `signal` description text,
  `detached_head` in `ConfirmedDeleteReason`, `max_listing_name_bytes` in
  the capability description; removing any from a copy fails.
- `remote_contract_gap_contract_old_owner_payloads_decode`: fixtures of a
  `merge_abort` result, a session resource, and a `confirmed_delete`
  capability captured from `10da8ac45` decode with new members `None`.
- Existing `merge_abort_contract_errors`, session, and confirmed-delete
  contract cases are updated only where they enumerate reasons or error
  codes. The gate command runs the whole suite.

### Gates `remote-contract-gap-regression-*` and `remote-contract-gap-build`

After all six fixes are integrated, re-run the five existing filters one at
a time with `cargo test --lib --features web-monitoring,heavy-tests <filter>
-- --nocapture` (one declared gate per filter); each must select and pass a
nonzero count. Then, as the build gate, `cargo test`
(default features), `cargo test --features web-monitoring`, and `cargo clippy
--all-targets --features web-monitoring -- -D warnings`. Pre-existing
failures (known: `tests/client_cli_tests.rs::wait_proves_base_publication_only_when_the_remote_matches`;
occasional parallel-run flakes of `remote_merge_abort_failed_abort_is_reported_not_assumed`
and `test_list_changes_native_excludes_hidden`) are recorded against the
unmodified base commit and not fixed here.

### Expected edits to existing tests

- `remote_worktree_session_classification_and_retry_notifications`: the
  `is_retry_notification` assertions are removed with the function; the
  `Cancelled` case now needs a `LeaderReaped` input.
- `remote_worktree_session_stop_kills_the_process_group_and_reaps`: the
  `signal == "SIGTERM"` assertion stays valid (`sleep` honours SIGTERM) but
  now proves an observed fact.
- `remote_worktree_session_configured_retry_stays_inside_one_session`:
  `attempt == 2` stays; the tail assertion, if any, no longer expects `[Retry`.
- `remote_text_edit_dropped_lease_stays_unresolved_without_a_timer`: drops
  an `attempted` lease instead of a fresh one; same assertions.
- `remote_text_edit_unbound_lifecycle_activity_refuses` ("unresolved-lease"
  arrangement): marks the lease `attempted` before dropping.
- Capability `reasons` assertions for `merge_abort` and `confirmed_delete`,
  and `ALL_ERROR_CODES` length (`29` → `30`), `ConfirmedDeleteReason::ALL`
  (`19` → `20`).
- No other existing test changes; any further edit is reported as a
  deviation.

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

- No `--model` flag is passed to the wrapper. The review session is separate
  from the implementation session and reviews the six contract-gap fixes as
  one focused diff plus compatibility; it is not a repository audit, and
  adjacent improvements without a serious finding are not added.
- The worker records the job ID, the raw `stdout.log` path, the exact argv,
  the actually responding model from the stream, and the final verdict in
  the handoff. The prompt file is deleted after the run has consumed it.
- A running job is neither killed nor duplicated; its ID is saved, and long
  test runs inside it are waited to synchronous completion.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof. No push, release, install, owner restart, or global configuration
  edit is performed by the worker or the reviewer.
