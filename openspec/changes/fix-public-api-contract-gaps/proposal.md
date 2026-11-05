---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - openspec/changes/archive/2026-11-05-add-remote-abort-merge/proposal.md
  - openspec/changes/archive/2026-11-05-add-remote-session-command/proposal.md
  - openspec/changes/archive/2026-11-05-add-remote-confirmed-delete/proposal.md
  - openspec/changes/archive/2026-11-05-add-remote-bounded-edit/proposal.md
  - openspec/changes/archive/2026-11-05-add-remote-progress-timing/proposal.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-contract-gap-merge
    requirement: The abort-on-conflict merge carries a typed merge_abort result on a hook failure after a landed merge and on a Git failure before any merge landed, with landing evidence re-read from the repository, no compensation, and merge_worktree unchanged
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-session
    requirement: Session attempt and runner notices come from a typed runner channel, output.lines holds only child stdout/stderr, forged child lines change no typed member, and signal is the reaped leader's signal including SIGKILL escalation or null when unknown, with the TUI text path unchanged
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops/session/tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-delete
    requirement: The consent oracle applies record and listing-name budgets per readdir entry before retaining names and closes every directory stream on every path, and a detached worktree preview reports every option ineligible with detached_head while existing confirmed-delete behavior is unchanged
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops/consent_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-ledger
    requirement: Unscoped writer leases release only on a positively known not-attempted or failed spawn, stay Unknown for an attempted spawn with unknown outcome or a spawn without process-group identity, and keep probe-clearable and descendant-retaining behavior for bound groups, with no kill authority added
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/ai_command_runner.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_contract_gap -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-contract
    requirement: Generated OpenAPI publishes the new error code, outcome values, merge and hook schemas, runner_notices, corrected signal description, and detached_head reason; older owners and their payloads decode as before; the full contract suite passes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-regression-session
    requirement: The existing configured-command session group keeps passing with a nonzero count after the typed-notice, child-only tail, and observed-signal fixes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/worktree_session_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-regression-edit
    requirement: The existing bounded text edit group keeps passing with a nonzero count after the spawn-outcome lease release fix
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/text_edit_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-regression-delete
    requirement: The existing confirmed-delete group keeps passing with a nonzero count after the budgeted readdir and detached_head fixes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/confirmed_delete_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-regression-merge
    requirement: The existing merge-abort group keeps passing with a nonzero count after the landed-merge and hook evidence fix
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/merge_abort_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-regression-timing
    requirement: The existing progress and timing group keeps passing with a nonzero count, proving the untouched fifth extension is not affected
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/progress_timing_tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_progress_timing -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_progress_timing -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-contract-gap-build
    requirement: Default-feature and web-monitoring builds and clippy stay clean, with any pre-existing baseline failure recorded against the unmodified base commit rather than fixed
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

# Fix public API contract gaps left by the five remote extensions

**Change Type**: implementation

## Problem / Context

The five remote extensions — abort-on-conflict merge, configured-command
worktree sessions, confirmed destructive deletion, bounded text edit, and
progress/timing facts — are implemented, integrated, and archived on
`feature/public-api-provider-extensions` at `10da8ac450e9ebf9bbbfc4a014e347c703647fe6`.
The independent re-run at that commit passed all 174 selected tests
(`/home/tumf/work/conflux-api-provider-20261105-verification.md`,
raw logs under `/home/tumf/work/conflux-api-provider-20261105-evidence/`).
The same verification recorded six places where the implementation does not
meet the contract the canonical `remote-control-api` spec already states, or
where a published owner fact is not actually owner-controlled. None of them
is a new feature; each is a gap between a promise the spec makes and what
the code at `10da8ac45` does:

1. **Merge abort: hook failure after a landed merge is untyped.**
   `WorktreeService::merge_worktree` runs `on_merged` after the merge commit
   landed and returns `WorktreeOpError::Internal` when it fails
   (`src/worktree_ops/service.rs:1439-1460`); `merge_abort_failure_result`
   maps only `PredictedConflict` and `MergeConflictAborted`
   (`src/web/remote_control_api/worktrees.rs:762-800`), so the record settles
   `internal_error` with `result: null`. The same happens to a Git failure
   after eligibility but before the merge (`src/worktree_ops/git_backend.rs:208`).
   The spec promises the `merge_abort` result "on success and on every
   refusal after eligibility", and a client cannot tell "base moved, hook
   failed" from "nothing happened".
2. **Session `attempt` is inferred from child text.** The session registry
   increments `attempt` when a stderr line starts with `[Retry `
   (`src/worktree_ops/session.rs:1088-1103, 1213-1216`). Any child that prints
   that prefix changes an owner fact.
3. **Session output mixes runner diagnostics with child output, and `signal`
   is the requested signal.** The runner sends its notices — retry, launch
   failure, shutdown refusal, runtime limit, inactivity, and the
   process-group cleanup report with the PGID — as `OutputLine::Stderr` on
   the same channel as the child's stderr (`src/ai_command_runner.rs:1582,
   1662, 1908, 2318, 2376, 2697-2724`), while the spec says `output.lines`
   is "the command's own stdout/stderr". `classify` hardcodes
   `Some("SIGTERM")` for `Cancelled` and `RuntimeLimit`
   (`src/worktree_ops/session.rs:1228, 1231`) and the runner discards the
   real `TerminationOutcome` from `terminate_with_timeout`
   (`src/ai_command_runner.rs:1842-1888`), so a SIGKILL escalation is
   reported as SIGTERM.
4. **Confirmed-delete walk materializes a whole directory before the record
   cap.** `read_dir_names` pushes every entry into a `Vec` and only the
   caller counts records (`src/worktree_ops/consent.rs:677-705, 876-910`), so
   one directory with millions of entries is unbounded in memory even though
   the oracle's budget is "fail closed".
5. **Detached HEAD preview and command disagree.** The preview never checks
   `is_detached` or an empty branch (`src/worktree_ops/service/confirmed.rs:41-75,
   355-413`), publishes `consent.branch: ""`, and on the real backend every
   option reads `ahead_unknown` only because inspection skipped the worktree;
   `ConfirmedDeleteParams::validate()` refuses the blank branch as
   `validation_failed` (`src/web/remote_control_api/dto.rs:609`). The reason
   is wrong and the published consent can never be accepted.
6. **Writer lease dropped before a PGID is bound is `Unknown` forever.**
   `WriterLease::drop` marks any unsettled lease `Unresolved`, and
   `coverage_under` can clear an unresolved lease only through a bound
   process group (`src/ai_command_runner.rs:900-914, 1046-1058`). In
   `execute_streaming_with_retry` the lease is bound only after
   `ManagedChild::new` succeeds and settled by `settle_after_reap`, which
   removes a lease with no PGID as "never spawned" (`1697-1701, 2403,
   1015-1043`) — so a failure between a real spawn and managed-child
   construction is wrongly cleared, and a task that never reached settlement
   is never clearable.

## Proposed Solution

One compatibility change, additive on the wire, that closes the six gaps
without weakening a single existing promise:

- **Typed merge landing and hook evidence.** `MergeAbortOutcome` gains
  `merged_hook_failed` and `merge_failed`; `MergeAbortResult` gains two
  nullable objects, `merge` (`landed: confirmed | not_landed | unknown`,
  `head_after`, `merge_head_present`, `clean_after`, `detail`) and `hook`
  (`ran`, `exit_ok`, `detail`), both re-read from the repository after the
  failure. A hook failure after a landed merge settles the new error code
  `merge_hook_failed` (409) with `recovery: local_or_tui_required`; a pre-merge
  Git failure keeps `internal_error` but carries `outcome: merge_failed`. The
  service distinguishes them with two new `WorktreeOpError` variants whose
  `Display` text equals today's messages, so the TUI's operator text and the
  `MergeFailed` event are unchanged; `merge_worktree` keeps mapping both to
  `internal_error` with no result. No re-merge, abort, reset, or hook re-run.
- **Typed runner notices for sessions.** `AiCommandRunner` gains a
  `RunnerNotice` enum (`RetryScheduled { next_attempt, max_attempts, cause,
  delay_ms }`, `AttemptStarted { attempt }`, `InactivityTerminated`,
  `RuntimeLimitReached`, `LaunchFailed`,
  `ShutdownRefused { stage }`, `ProcessGroupCleanupUnconfirmed { pgid,
  force_killed }`, `LeaderReaped { attempt, exit_code, signal }`,
  `LeaderNotReaped { attempt }`) and a second streaming entry point that
  publishes those on a dedicated channel and sends **no** runner text on the
  output channel. The existing entry point keeps emitting the same stderr
  lines, so the TUI terminal, parallel executor, and shell command path are
  untouched. The session registry uses the typed entry point, sets later
  `attempt` values from `AttemptStarted` after a successful spawn, not from
  a retry merely scheduled during its delay; it retains notices in a bounded
  `runner_notices` list, stores only child lines in `output.lines`, and
  derives `signal`/`exit_code` from the final `LeaderReaped` — `SIGTERM`,
  `SIGKILL`, or a real exit code — and publishes `null` for both when the
  leader was not reaped. The `[Retry ` prefix check is deleted.
- **Bounded `readdir`.** `read_dir_names` becomes a budgeted iteration: each
  entry is counted as a record before its name is retained, name bytes are
  charged against a new `ConsentLimits::max_listing_name_bytes` (64 MiB in
  production), the first entry past either budget stops the listing with
  `Stop::Unbounded`, and the `DIR*` is owned by a guard that calls `closedir`
  on drop. Depth 256, the combined record/byte budget, and every identity
  rule are untouched.
- **`detached_head` reason.** `classify_confirmed_delete` and
  `classify_consent` check `is_detached || branch.is_empty()` before every
  other fact and return the new closed `ConfirmedDeleteReason::DetachedHead`;
  the preview reports it for all four options, and the command refuses it as
  `target_ineligible` under the guard. `validate()` keeps refusing a blank
  `consent.branch`, so no detached destructive support exists remotely.
- **Spawn-outcome state on the lease.** `WriterLease` records
  `SpawnOutcome::{NotAttempted, Attempted, Failed, Spawned { pgid:
  Option<u32> }}`. The runner marks `Attempted` immediately before `spawn`,
  `Failed` on a spawn `Err`, and binds the PGID immediately after a
  successful spawn, before `ManagedChild::new`. Drop and settlement release
  only `NotAttempted` and `Failed`; `Attempted` and `Spawned { pgid: None }`
  stay `Unknown` with a typed detail; `Spawned { pgid: Some }` keeps today's
  probe-clearable and descendant-retaining rules.

## Acceptance Criteria

1. Against a real Git fixture, an abort-on-conflict merge whose `on_merged`
   hook exits non-zero leaves the merge commit on base, settles
   `merge_hook_failed` with `outcome: merged_hook_failed`, `merge.landed:
   confirmed`, `hook.ran: true`, `hook.exit_ok: false`, runs the hook exactly
   once, and `merge_worktree` with the same hook still settles
   `internal_error` with `result: null`. A clean-check failure settles
   `internal_error` with `outcome: merge_failed`, `merge.landed: not_landed`,
   `hook.ran: false`; an unreadable re-read settles `merge.landed: unknown`.
   Every existing `remote_merge_abort` case passes unchanged except the
   capability `reasons` assertion.
2. A session whose child prints `[Retry 1/3] …` and a fake PGID diagnostic
   finishes with `attempt: 1`, empty `runner_notices`, and both lines in
   `output.lines` verbatim. A real crash-retry finishes with `attempt: 2`,
   one `retry_scheduled` notice, and no runner text in the tail; the TUI
   terminal path still prints `[Retry 1/1]` to stderr for the same command.
   If stop cancels the delay after scheduling but before a second spawn,
   `attempt` stays `1` and the scheduled notice remains visible.
3. Stopping a session whose command ignores SIGTERM settles `stopped`,
   `signal: SIGKILL`, `terminated: true`, `attempt: 1`, with the whole group
   gone; a command that honours SIGTERM settles `signal: SIGTERM`; a
   non-reaped leader settles `signal: null`, `exit_code: null`.
4. With injected `max_records: 5`, a directory holding 20 entries yields
   `Unbounded` with the walker's record counter at most 6 and at most 6 names
   retained; with a small `max_listing_name_bytes` the same holds for name
   bytes; every existing `remote_confirmed_delete` case passes unchanged
   except the capability `reasons` assertion.
5. A detached worktree's preview reports all four options
   `blocked_reason: detached_head`, publishes `is_detached: true` and an
   empty `consent.branch`, and echoing that consent is `validation_failed`;
   detaching after an attached preview makes the command `target_ineligible`
   with `refusal.reason: detached_head` and nothing removed.
6. A lease refused before spawn or whose spawn failed releases to `Clear`; a
   lease marked `attempted` and dropped, or `spawned` without a PGID, stays
   `managed_process_live` with a typed detail across repeated reads and a
   generous wait; a bound-group lease dropped without reap becomes
   `editable: true` only after the group is proven empty, with nothing
   killed. `remote_text_edit_dropped_lease_stays_unresolved_without_a_timer`
   is rewritten to drop an `attempted` lease and keeps its assertion.
7. OpenAPI publishes `merge_hook_failed`, `merged_hook_failed`,
   `merge_failed`, the `merge` and `hook` schemas, `runner_notices`, the new
   `signal` description, `max_listing_name_bytes` in the documented budget,
   and `detached_head`; payloads from an owner built at `10da8ac45` decode
   with every new member absent; the full `openapi_contract_tests` suite
   passes.
8. The five existing groups re-run green with nonzero counts, and default,
   `web-monitoring`, and clippy regressions are reported against the
   pre-change baseline with any pre-existing failure named, not fixed here.

## Explicit Completion Conditions

Every new case carries the test-name prefix `remote_contract_gap`; the four
lib gates select it with nonzero passed counts, and a zero-test selection
fails. The contract gate runs the full `openapi_contract_tests` suite. The
five regression gates re-run `remote_worktree_session`, `remote_text_edit`,
`remote_confirmed_delete`, `remote_merge_abort`, and `remote_progress_timing`
one filter at a time after integration of all six fixes, and the build gate
runs clippy with the default and `web-monitoring` suites reported alongside. `cflx openspec validate --strict` and
`--archive-gate` are structural checks and are not implementation PASS.
Tasks stay unchecked until the implementation is verified in a separately
authorized execution, and this proposal is not an independent review of
itself.

## Scope / Preserved Contracts

Implementation touches `src/worktree_ops/service.rs` (two error variants),
`src/worktree_ops/git_backend.rs` and `src/vcs/git/commands/merge.rs`
(landing re-read), `src/web/remote_control_api/{dto.rs,worktrees.rs}`
(outcomes, evidence objects, error code, reasons), `src/ai_command_runner.rs`
(`RunnerNotice`, typed entry point, `SpawnOutcome`, bind order),
`src/worktree_ops/session.rs` and `src/web/remote_control_api/worktree_sessions.rs`
(typed attempt, child-only tail, observed signal, `runner_notices`),
`src/worktree_ops/consent.rs` and `src/worktree_ops/service/confirmed.rs`
(budgeted `readdir`, `detached_head`), `src/web/openapi.rs`, focused tests,
`docs/guides/USAGE.md`, and `AGENTS.md`. Unchanged: `merge_worktree`,
`delete_worktree`, `DeleteOptions`, `classify_delete_eligibility`, the
text-emitting runner entry point and every consumer of it, the retry
policy, the 5-second escalation, `RunCommandScope` coverage, canonical cwd
containment, `force_stop_change`, every `cflx client`/MCP verb, and every
existing wire member. No new dependency, no Conflux-owned write into any
worktree, no kill authority added.

## Out of Scope / Rollout Handoff

Not included: NAME_MAX handling in text edit, the empty `RunDispatched`
clock difference, Server-side UI or relay, a broad audit, the known
`tests/client_cli_tests.rs::wait_proves_base_publication_only_when_the_remote_matches`
failure, redesigning the TUI dirty/ahead consent, any new feature, push,
release, install, deployment, owner stop or restart, and global
configuration or authentication changes. The implementation is executed
later through the existing Herdr-managed cflx owner; this proposal marks,
starts, or merges nothing. The independent review of the implementation is
launched by the parent per the contract in `design.md`.
