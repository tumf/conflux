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
  - docs/guides/USAGE.md
  - AGENTS.md
verifications:
  - id: retain-api-completed-owner
    requirement: Through the real dispatcher, WebState, reducer, projection, and production router, merged and pushed rows this owner published stay listed with their exact id, reducer-authored status, last observed progress, archive completion, timing, and net-active time across repeated event-driven and disk refreshes, archive absence, reads, and reconnects; resolving rows stay only until they settle; no other absent row is retained; list, get, state, and stream agree at one revision; reads change nothing; totals, actions, the run clock, and command admission are unchanged; TUI and API retain the same rows and TUI dismissal leaves the API row; a new owner starts empty; capabilities publish the retention block and an older owner's omission decodes as unavailable
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
    requirement: The generated OpenAPI document publishes CompletedChangeRetentionCapability with its four required members, an optional CapabilitiesResponse.completed_change_retention member that decodes as absent from an older owner, and retention-describing InstanceSnapshot.changes and ChangesResponse.changes descriptions; the published capability payload validates against it, and a document missing the block fails the contract
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
    requirement: Against a real Git repository whose change is archived and merged into the base branch, the production disk refresh and the production listing behind ChangesRefreshed keep the row as resolving and then as merged with its task counts and archive completion, the production router serves one coherent row, reads and refreshes leave the repository byte-for-byte unchanged, and a new owner over the same repository publishes no row for the merged change
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
  `OperatorFactsStore::observe_changes` (`src/web/operator_facts.rs:261-280`)
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

A downstream Server hides and restores merged rows locally (Web OP17/OP18) and
needs the owner to keep publishing them for as long as the owner lives, with a
discoverable guarantee. The five API families already delivered (bounded text
edit, worktree sessions, confirmed deletion, abort-on-conflict merge, and
archive completion/run clock/net-active) are consumed unchanged and are not
reimplemented here; the only new scope is retention.

## Proposed Solution

- Keep the retained rows in the existing published list inside `WebState`'s own
  stored snapshot — owner-local presentation state with no new route, resource,
  command, or row member.
- At every workspace observation, carry forward each previously published row
  that the observation no longer lists when the reducer-authored display status
  is `merged` or `pushed` (kept for the rest of the incarnation) or `resolving`
  (kept only until it settles). The status comes from the reducer state the
  observation already holds — the dispatch's authoritative state or the
  existing non-blocking read — and, only when none is readable, from the row's
  last reducer-derived `queue_status`. Nothing is inferred from absence, task
  completion, the archive directory, or `archive_complete`.
- Apply the same rule in both rebuild paths through one helper, append carried
  rows after the observation's rows in their previous order, recompute the
  summary, and observe the published IDs (observation plus carried) in the
  facts store so `timing`, `latest_activity`, and attention survive.
- Publish `CapabilitiesResponse.completed_change_retention`
  (`CompletedChangeRetentionCapability { available, retained_statuses,
  settling_statuses, scope }`, always `true`/`[merged, pushed]`/`[resolving]`/
  `instance` on this build, `#[serde(default)]` so an older owner decodes as
  unavailable), register it in the generated OpenAPI document, and describe
  retention on `InstanceSnapshot.changes` and `ChangesResponse.changes`.
- Document the guarantee, its lifetime, its observable consequences, and the
  consumer rule that client-local hide state is keyed by `instance_id`.

The exact algorithm, the evidence rule, every edited function, the observable
consequences for existing surfaces, the test plan, the gates, and the
independent review launch contract are fixed in `design.md`.

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
2. A previously published `resolving` row absent from the listing stays until it
   settles; it continues as `merged`/`pushed`, and any other settlement is
   released by the next observation that does not list it.
3. Absent rows reported `not queued` with all tasks done, `merge wait` or
   `resolve pending` with archive completion, `error`, `rejected`, `stopped`,
   or with no reducer evidence are released exactly as before; no row is ever
   published `merged`/`pushed` unless the reducer authored it.
4. A refresh that fails before producing an observation changes no row and no
   revision; an observation applied while the reducer is write-locked keeps a
   row whose last reducer-derived status is `merged`; an empty observation keeps
   retained rows. Same-ID reappearance yields one row with the observation's
   listing fields, the reducer-authored status, and no new-arrival attention.
5. Event replay (`events_after` and the `/api/v2/events` SSE replay) carries the
   same revision as state/list/get, `changes_refreshed` payloads count the
   observation, and a reconnecting client (`GET /api/v2/state` then its cursor)
   sees no removal. Repeated reads change no revision, event sequence, command
   or idempotency registry, reducer status, mark, file, or Git ref.
6. For a retained merged row, every lifecycle command (`set_execution_mark` on
   and off, `set_all_execution_marks`, `set_queue_intent`, `retry_change`,
   `retry_errors`, `stop_and_dequeue`, `force_stop_change`, `resolve_merge`,
   `start` with a lingering mark) settles exactly as for the same row while it
   was still listed, with no scheduler, queue, mark, retry-edge, reservation, or
   mode effect; its `actions` are all blocked; totals count it as completed only;
   the run clock and its net-active time neither reopen nor reset.
7. The TUI and the API, driven by one event sequence at injected instants, keep
   the same retained IDs with the same display status, task counts, archive
   completion, and net-active time; TUI `d` dismissal leaves the API row and
   revision unchanged; no Conflux command exists for hiding or restoring rows.
8. A new owner over the same workspace starts with a new `instance_id` and no
   row for a change merged by the earlier incarnation; present changes are
   observed independently; nothing is persisted or rehydrated.
9. `GET /api/v2/capabilities` publishes `completed_change_retention` exactly as
   above; an older owner's capabilities (the existing `legacy_owner_shape`
   fixture) decode as `available: false`; the generated OpenAPI document
   publishes the block as an optional member with four required members and the
   retention descriptions; a payload missing a pre-existing required member is
   still rejected; the closed command set is unchanged.
10. A bounded real Git fixture (archive move committed on a worktree branch and
    merged with `--no-ff` into the base) driven through `refresh_from_disk_at`
    and the production listing functions behind `ChangesRefreshed` keeps the row
    as `resolving` and then `merged`, serves it coherently through the production
    router, leaves the repository unchanged across reads and refreshes, and a
    new owner over the merged repository publishes no row for it.

## Explicit Completion Conditions

Every new test name starts with `retain_api_completed_`, and the three declared
gates each select and pass a nonzero number of them; a zero-test selection is a
failure. The cases, files, and assertions listed in `design.md` ("Test plan")
exist and pass. `docs/guides/USAGE.md` and `AGENTS.md` describe the guarantee as
implemented. No existing assertion changes except under the rule in `design.md`
("Expected edits to existing tests"). Broad suites, clippy, fmt, and the OpenAPI
baseline diff are recorded as separate observations in `tasks.md` Notes, not as
gates. The nested independent implementation review is launched exactly per the
contract in `design.md` and its artifacts are recorded. Proposal validation is
not implementation evidence: tasks stay unchecked until implemented and verified
in a separately authorized execution, and this proposal is not an independent
review of itself.

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
persisted file, or configuration key.

The constitution is satisfied without amendment: retention is ephemeral
in-memory observability state discarded on restart (Law 1), completion comes
only from reducer-authored status (Law 3), and nothing is written to any
worktree (Law 4).

The tracked `.pre-commit-config.yaml` runs rustfmt and clippy only when staged
paths match `^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$`; it is
staged-path scoped, not unconditional, so it is not treated as the owner of
repository-wide verification.

## Out of Scope

- Retaining absent rows in waits or errors (`merge wait`, `resolve pending`,
  `error`, `stopped`, `rejected`) or copying the TUI's execution-history rule.
  An `on_merged` failure or a publication failure after the proposal left the
  base therefore releases the row, and a later completion of that ID is not
  republished (see `design.md`, "Known limitation").
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
- Whether post-archive rows that settle into `merge wait` or `error` after their
  proposal left the base should also be retained is an open product decision;
  this change deliberately excludes wait and error retention.
- Releasing a binary with this change, pinning it, and replacing the running
  owner (`eeb5a02e3c1edd25c129716524471026`) happen under separate authority;
  this change performs none of them.
