---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-merged-row-dismissal/spec.md
  - openspec/specs/tui-state/spec.md
  - openspec/changes/archive/2026-11-05-add-remote-progress-timing/proposal.md
  - src/web/state.rs
  - src/web/operator_facts.rs
  - src/web/remote_control_api/projection.rs
  - src/tui/state/processing_logic.rs
  - src/parallel/merge.rs
  - src/orchestration/state.rs
  - docs/guides/USAGE.md
  - AGENTS.md
verifications:
  - id: retain-api-completed-owner
    requirement: Through the real dispatcher, WebState, reducer, projection, and production router, merged and pushed rows this owner published stay listed with their exact id, reducer-authored status, last observed progress, archive completion, timing, and net-active time across repeated event-driven and disk refreshes, archive absence, reads, and reconnects; resolving rows stay while resolving and a settled one is released only by an observation that reads the reducer; a change released in merge wait after an on_merged failure or in error after a publication failure is published again, coherently in state, list, get, and the stream, when the reducer completes it, with the facts of a control owner that kept listing it, and is published nowhere while released; no other absent row is published; reads change nothing; totals, actions, the run clock, and command admission are unchanged and never read released rows; TUI and API retain the same rows and agree after both recovered completions, and TUI dismissal leaves the API row; a new owner starts with no retained or released rows; capabilities publish the retention block and an older owner's omission decodes as unavailable; exactly the 21 named owner cases pass with none ignored
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/operator_snapshot_tests.rs
    evidence: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --lib --features web-monitoring retain_api_completed -- --nocapture
    rerun: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --lib --features web-monitoring retain_api_completed -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: retain-api-completed-contract
    requirement: The generated OpenAPI document publishes CompletedChangeRetentionCapability with its four required members, an optional CapabilitiesResponse.completed_change_retention member that decodes as absent from an older owner, and retention-describing InstanceSnapshot.changes and ChangesResponse.changes descriptions; the published capability payload validates against it, and a document missing the block fails the contract; exactly the 2 named contract cases pass with none ignored
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --features web-monitoring --test openapi_contract_tests retain_api_completed -- --nocapture
    rerun: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --features web-monitoring --test openapi_contract_tests retain_api_completed -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: retain-api-completed-git
    requirement: Against a real Git repository whose change is archived and merged into the base branch, the production disk refresh and the production listing behind ChangesRefreshed keep the row as resolving and then as merged with its task counts and archive completion, the production router serves one coherent row, reads and refreshes leave the repository byte-for-byte unchanged, and a new owner over the same repository publishes no row for the merged change; exactly the 1 named Git case passes with none ignored
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --features web-monitoring,heavy-tests --test e2e_git_worktree_tests retain_api_completed -- --nocapture
    rerun: TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --features web-monitoring,heavy-tests --test e2e_git_worktree_tests retain_api_completed -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retain completed changes in the `/api/v2` projection for the owner incarnation

**Change Type**: implementation

## Problem / Context

At base `06faae562` the TUI keeps a merged change on screen for the whole owner
process, but `/api/v2` loses it a few seconds after it completes:

- Both workspace-observation paths rebuild the published change list from the
  current scan and nothing else. The event path replaces `state.changes` in the
  `ChangesRefreshed` arm (`src/web/state.rs:1486-1523`); the periodic Web/UDS
  refresh (`src/web/mod.rs:521-541`, every 5 s) goes through
  `refresh_from_disk_at` → `merge_workspace_observation`
  (`src/web/state.rs:1108-1189`, `1803-1888`). A merged change's proposal
  directory has moved to `openspec/changes/archive/`, so the next observation
  drops its row: `GET /api/v2/changes` no longer lists it,
  `GET /api/v2/changes/{id}` answers 404, and the next state revision removes it.
- The process-local facts store forgets the same IDs:
  `OperatorFactsStore::observe_changes` (`src/web/operator_facts.rs:255-280`)
  drops `timing`, `latest_activity`, and attention for every ID the observation
  did not name.
- The TUI does the opposite (`src/tui/state/processing_logic.rs:245-264`): rows
  with execution history or a terminal/wait status survive absence, and `d`/`D`
  dismissal is TUI-local presentation (`openspec/specs/tui-merged-row-dismissal`).
- The reducer already holds the authoritative answer for the whole incarnation.
  Its `ChangesRefreshed` handling never prunes runtime entries
  (`src/orchestration/state.rs:3030-3070`), `Merged` and `Pushed` can never be
  superseded (`can_success_supersede_terminal`, `642-644`; `2150-2155`;
  `2829-2839`), and `display_status` never authors `archived` (`694-730`).
- Completion is authored *after* the proposal leaves the base. The merge path
  emits `ResolveStarted`, lands the merge on the base branch, runs `on_merged`,
  and only then sends `MergeCompleted` (`src/parallel/merge.rs:1050-1065`,
  `541-639`). A refresh inside that window sees the change absent while the
  reducer still reports `resolving`, so a rule that kept only `merged` rows
  would drop the row before it could ever be published as merged.
- Completion can also come after a wait or a failure in the same absence. An
  `on_merged` failure leaves the change in `merge wait` (`merge.rs:209-211`,
  `state.rs:2808-2811`) until an explicit resolve completes it as `merged`
  (`2225-2247`, `2816-2820`); a publication failure leaves it in `error`
  (`2840-2844`) until resumed publication completes it as `pushed`
  (`merge.rs:1089-1097`, `state.rs:642-643`, `2829-2833`). The TUI keeps those
  rows throughout; a projection that forgets a row the moment it stops
  publishing it can never show their completion.

A downstream Server hides and restores merged rows locally (Web OP17/OP18) and
needs the owner to keep publishing them for as long as the owner lives, with a
discoverable guarantee. The five API families already delivered (bounded text
edit, worktree sessions, confirmed deletion, abort-on-conflict merge, and
archive completion/run clock/net-active) are consumed unchanged and are not
reimplemented here; the only new scope is retention.

## Proposed Solution

- Keep published rows in the existing list inside `WebState`'s own stored
  snapshot — owner-local presentation state with no new route, resource,
  command, or row member.
- At every workspace observation, carry forward each previously published row
  that the observation no longer lists when its reducer-authored display status
  is `merged` or `pushed` (kept for the rest of the incarnation) or `resolving`
  (kept while it lasts). The status comes from the one reducer read the update
  holds — the dispatch's authoritative state or the existing non-blocking
  read — and, only when none is readable, from the row's last reducer-derived
  `queue_status`. Rows are released only by observations, so a settled
  `resolving` row is released by the first observation that reads the reducer.
- Move every row an observation releases into a private, process-local memory
  beside the snapshot (`released_rows`: last published row per ID, release
  order, at most one per ID). At every reducer event and every observation that
  reads the reducer, publish again each remembered row the reducer now reports
  `merged`, `pushed`, or `resolving`. The memory is never projected, persisted,
  or read by admission; a new process starts without it; an observation that
  lists a remembered ID again discards the memory and treats the change exactly
  as today. Nothing is inferred from absence, task completion, the archive
  directory, or `archive_complete`.
- Do both rebuild paths through one helper, make the disk path one
  read-modify-write under a single `state` write guard, append kept rows after
  the observation's rows in their previous order and republished rows after
  them in release order, recompute the summary, and observe the published and
  remembered IDs in the facts store so `timing`, `latest_activity`, and
  attention survive release.
- Publish `CapabilitiesResponse.completed_change_retention`
  (`CompletedChangeRetentionCapability { available, retained_statuses,
  settling_statuses, scope }`, always `true`/`[merged, pushed]`/`[resolving]`/
  `instance` on this build, `#[serde(default)]` so an older owner decodes as
  unavailable), register it in the generated OpenAPI document, and describe
  retention on `InstanceSnapshot.changes` and `ChangesResponse.changes`.
- Document the guarantee, its lifetime and memory scope, its observable
  consequences, and the consumer rule that client-local hide state is keyed by
  `instance_id`.

The exact algorithm, the evidence rule, the locks and their order, every edited
function, the observable consequences for existing surfaces, the test plan, the
gates, and the independent review launch contract are fixed in `design.md`.

## Acceptance Criteria

1. Through the real `EventDispatcher` → `WebEventSink` → `WebState` → projection
   → production router (`crate::web::remote_control_router`), a change this owner
   published and the reducer reports `merged` (or `pushed`) stays in `GET
   /api/v2/changes`, `GET /api/v2/changes/{id}` (200), and `GET /api/v2/state`
   after its proposal leaves the listing, across at least three event-driven
   refreshes and three disk-path refreshes, with exact `id`, reducer-authored
   status, last observed `completed_tasks`/`total_tasks`/`progress_percent`/
   `progress_status`, `archive_complete: true`, `timing`, `latest_activity`, and
   `net_active` unchanged, one `instance_id`, and one `state_revision` that does
   not advance after the absence revision.
2. A previously published `resolving` row absent from the listing stays while it
   is `resolving` and continues as `merged`/`pushed`. If it settles into any
   other status it is published with that reducer status until the first
   observation that reads the reducer releases it; an observation that cannot
   read the reducer keeps it while its stored status is still `resolving`.
3. A change released in `merge wait` after an `on_merged` failure, or in
   `error` after a publication failure, with an event-driven and a disk refresh
   between the failure and the recovery, is published again — at the first
   reducer event or observation that reads the reducer after the reducer authors
   `resolving`, `merged`, or `pushed`, in that update's revision, consistently
   across state/list/get and the event stream — and then stays published as
   `merged`/`pushed` with its last observed task counts and `archive_complete`
   and the same `timing`, `latest_activity`, `net_active`, and attention as a
   control owner that kept listing it.
4. Absent rows reported `not queued` with all tasks done, `merge wait` or
   `resolve pending` with archive completion, `error`, `rejected`, `stopped`,
   or with no reducer evidence are not published, exactly as before, for as long
   as the reducer authors none of `merged`, `pushed`, or `resolving` for them;
   no row is ever published `merged`/`pushed` unless the reducer authored it.
5. A released change is published nowhere: not in state/list (absent),
   get/refresh-preview/text (answered as for a never-observed ID), execution
   status, totals, the run-clock rows, or any published row's worktree relation,
   even when an observation reports a worktree path for it; reads leave its
   memory intact; no command admission reads it; a released ID listed again is
   one row from the observation, newly detected with fresh facts, exactly as
   before.
6. A refresh that fails before producing an observation changes no row, memory,
   or revision; an observation applied while the reducer is write-locked keeps a
   row whose last reducer-derived status is `merged` and publishes nothing
   again; an empty observation keeps retained rows. Same-ID reappearance of a
   published row yields one row with the observation's listing fields, the
   reducer-authored status (for example `merged` 1/3), and no new-arrival
   attention.
7. Event replay (`events_after` and the `/api/v2/events` SSE replay) carries the
   same revision as state/list/get, `changes_refreshed` payloads count the
   observation, and a reconnecting client (`GET /api/v2/state` then its cursor)
   sees no removal. Repeated reads change no revision, event sequence, command
   or idempotency registry, reducer status, mark, file, or Git ref.
8. For a retained merged row, and for a released `merge wait` change, every
   lifecycle command (`set_execution_mark` on and off, `set_all_execution_marks`,
   `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`,
   `force_stop_change`, `resolve_merge`, `start` with a lingering mark) settles
   exactly as for the same change while it was still listed, with no scheduler,
   queue, mark, retry-edge, reservation, or mode effect from retention; the
   retained row's `actions` are all blocked; totals count it as completed only;
   the run clock and its net-active time neither reopen nor reset.
9. The TUI and the API, driven by one event sequence at injected instants, keep
   the same retained IDs with the same display status, task counts, archive
   completion, and net-active time, including after `HookFailed` → resolve →
   `MergeCompleted` and `PushFailed` → resumed publication → `PushCompleted`
   with absent refreshes between; the API `run_clock` equals the TUI's Elapsed
   clock during absent `resolving` refreshes, at each settlement, and afterwards;
   TUI `d` dismissal leaves the API row and revision unchanged; no Conflux
   command exists for hiding or restoring rows.
10. A new owner over the same workspace starts with a new `instance_id`, no row
    for a change merged by the earlier incarnation, and no memory of released
    rows; a reducer completion for an ID it never published publishes nothing;
    present changes are observed independently; nothing is persisted or
    rehydrated.
11. `GET /api/v2/capabilities` publishes `completed_change_retention` exactly as
    above; an older owner's capabilities (the existing `legacy_owner_shape`
    fixture) decode as `available: false`; the generated OpenAPI document
    publishes the block as an optional member with four required members and the
    retention descriptions; a payload missing a pre-existing required member is
    still rejected; the closed command set is unchanged.
12. A bounded real Git fixture (archive move committed on a worktree branch and
    merged with `--no-ff` into the base) driven through `refresh_from_disk_at`
    and the production listing functions behind `ChangesRefreshed` keeps the row
    as `resolving` and then `merged`, serves it coherently through the production
    router, leaves the repository unchanged across reads and refreshes, and a
    new owner over the merged repository publishes no row for it. It rebuilds
    the event payload from the production listing reads rather than running the
    TUI refresh task itself; that evidence boundary is stated in `design.md`
    5.7.

## Explicit Completion Conditions

There are exactly 24 new tests, every name starting with `retain_api_completed_`,
listed by name in `design.md` section 5. Each declared gate passes only with
exactly its expected count — `retain-api-completed-owner` 21 (tests 1-21),
`retain-api-completed-contract` 2 (tests 22-23), `retain-api-completed-git` 1
(test 24) — reported by the command's own summary with `0 failed` and `0
ignored`, and with every expected name present once as `ok`; any other count, a
missing or extra name, an ignored case, or a zero-test selection fails the gate.
No new test carries `#[ignore]`; the owner and contract cases are bounded, and
the Git case runs only under the existing `heavy-tests` feature (never the
nonexistent `heavy` feature). The cases, files, and assertions listed in
`design.md` exist and pass. `docs/guides/USAGE.md` and `AGENTS.md` describe the
guarantee as implemented. No existing assertion changes except under the rule in
`design.md` ("Expected edits to existing tests"). Broad suites, clippy, fmt, and
the OpenAPI baseline diff are recorded as separate observations in `tasks.md`
Notes, not as gates. The nested independent implementation review is launched
exactly per the contract in `design.md` and its artifacts are recorded. Proposal
validation is not implementation evidence: tasks stay unchecked until
implemented and verified in a separately authorized execution, and this
proposal is not an independent review of itself.

## Scope / Preserved Contracts

Implementation may touch `src/web/state.rs`, `src/web/remote_control_api/dto.rs`,
`src/web/remote_control_api/reads.rs`, `src/web/openapi.rs`,
`src/client/session.rs` (test helper field only), the focused tests listed in
`design.md`, `docs/guides/USAGE.md`, and `AGENTS.md`. Unchanged: the reducer and
every reducer transition, the TUI (including its broader retention and its
dismissal), `OperatorFactsStore` code, `projection.rs` (`project_snapshot`,
`classify_actions`, `describe_event`), every `ChangeResource`/`InstanceSnapshot`/
`SnapshotTotals` member and meaning, `ChangeTiming`, `net_active`, `run_clock`,
`archive_complete`, events and their payloads, the closed command set, command
admission, mark reconciliation and settlement, completion sinks and
subscriptions, `cflx client` code and envelopes, the embedded Web console, and
every file outside the owner process. No new dependency, route, command,
persisted file, or configuration key. Client-visible consequences in `design.md`
D13 are derived from owner-side facts and are not validated as client behavior
by this change.

The constitution is satisfied without amendment: retained rows and the private
memory of released rows are ephemeral in-memory observability state discarded on
restart and never read by any next-action decision (Law 1), completion is
published only from reducer-authored status (Law 3), and nothing is written to
any worktree (Law 4).

The tracked `.pre-commit-config.yaml` runs rustfmt and clippy only when staged
paths match `^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$`; it is
staged-path scoped, not unconditional, so it is not treated as the owner of
repository-wide verification.

## Out of Scope

- Publishing absent rows while they wait or fail (`merge wait`, `resolve
  pending`, `error`, `stopped`, `rejected`) beyond the settling window of
  `design.md` D3, or copying the TUI's execution-history rule. Such rows stay
  unpublished while they wait or fail; publishing them again once the reducer
  completes them is in scope.
- Historical archived changes at startup, persistence, TTLs, and rehydration.
- A per-row "retained" member, a completed-history resource, or any dismissal
  command.
- Server code, Server hide caches, and the Server Web UI.

## Future Work

These are narrative handoffs, not active tasks, and none is claimed to have
passed:

- The Server-side end-to-end observation — browser individual and bulk
  hide/restore (OP17/OP18) over a retention-capable owner, followed by an owner
  restart that rekeys or clears the Server's hide cache — is a separate
  downstream integration observation owned by the Server, not a Conflux
  completion gate.
- Releasing a binary with this change, pinning it, and replacing the running
  owner (`eeb5a02e3c1edd25c129716524471026`) happen under separate authority;
  this change performs none of them.
