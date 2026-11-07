# Design: retain completed changes in the `/api/v2` projection

Base: `06faae562d1d53b2723d0912f4095d3fd0d49d7e` (`v0.6.327` sources; the
deployed rc.2 owner adds release-only metadata on top of the same runtime).
Line numbers below are at that base.

## 1. Source facts this design is built on

1. **Two rebuild paths, both wholesale.**
   - Event path: `WebState::apply_dispatch` → `ExecutionEvent::ChangesRefreshed`
     arm (`src/web/state.rs:1486-1523`) builds `new_change_statuses` from the
     event's `changes`, copies `iteration_number` and 0/0-progress from the
     previous row of the same ID, then `state.changes = new_change_statuses`.
     In the TUI owner this event comes from the refresh task
     (`src/tui/runner.rs:915-1110`, every `AUTO_REFRESH_INTERVAL_SECS` = 5 s)
     through the process dispatcher (`src/tui/runner.rs:626-653`), so the
     dispatch always carries the authoritative reducer state.
   - Disk path: the Web/UDS refresh task (`src/web/mod.rs:521-541`, every
     `refresh_interval_secs` = 5 s) calls `refresh_from_disk_at`
     (`src/web/state.rs:1803-1888`), which lists `openspec/changes/*`
     (`openspec::list_changes_native_from`, skipping `archive/`) and calls
     `merge_workspace_observation` (`src/web/state.rs:1108-1189`); that builds
     a new snapshot with `from_changes_with_shared_state` under a non-blocking
     reducer read and replaces `*state`.
   - Neither path keeps an ID the observation does not list.
2. **Facts are pruned with the rows.** `OperatorFactsStore::observe_changes`
   (`src/web/operator_facts.rs:261-280`) retains `known_change_ids` and `facts`
   (timing, latest activity, attention, parallel, worktree) only for the
   observed IDs. `net_active` and `observed_statuses` are separate maps that
   are never pruned (`124-135`). The event path observes `changes ∪
   rejected_changes` (`src/web/state.rs:1766-1782`); the disk path observes the
   listing (`observe_changes_for_attention`, `831-836`).
3. **The reducer is the incarnation-long authority.**
   `OrchestratorState::apply_execution_event(ChangesRefreshed)`
   (`src/orchestration/state.rs:3030-3070`) adds new IDs and resets `Rejected`
   only; it never removes a `change_runtime` entry, `archived_changes`, or
   `task_progress`. `TerminalState::Merged`/`Pushed` cannot be superseded
   (`can_success_supersede_terminal`, `642-644`); dequeue, stop, retry, resolve,
   and run-stop guards all refuse them (`2016-2036`, `2050-2071`, `2106-2129`,
   `2225-2289`, `3005-3023`). The TUI owner creates the shared reducer once
   (`src/tui/runner.rs:503-507`); `cflx run` replaces it once, before any work
   (`src/orchestrator.rs:548-551`). `display_status` (`694-730`) never returns
   `archived`; it returns `merged`/`pushed` exactly for those terminals.
4. **Completion is authored after the base loses the proposal.**
   `attempt_merge` emits `ResolveStarted` for each change, then merges into the
   base (`src/parallel/merge.rs:1050-1065`); `handle_merge_and_cleanup` then
   runs `on_merged` and only afterwards sends `MergeCompleted`
   (`541-639`). With upstream publication the terminal is `PushCompleted` after
   `PushStarted` (reducer `2822-2839`). During that window the reducer status is
   `resolving` (`ActivityState::Resolving`) and the base no longer lists the
   change.
5. **The TUI keeps more, locally.** `update_changes_with_rejected`
   (`src/tui/state/processing_logic.rs:245-264`) keeps rows with execution
   history or status `archiving|archived|merged|merge wait|resolving|resolve
   pending|rejected|error`; `suppress_dismissed_rows` (`28-66`) enforces
   `d`/`D` dismissal, which is TUI-local by spec
   (`openspec/specs/tui-merged-row-dismissal/spec.md`).
6. **Reads are pure projection reads.** `reads.rs` `state`/`list_changes`/
   `get_change` (`183-240`) and `execution_status` (`342-372`, which iterates
   `snapshot.changes`) read one `Projection::snapshot()`; the projection
   advances `state_revision` only when the candidate differs
   (`projection.rs:276-322`). `project_snapshot` (`557-615`) maps every stored
   row through `classify_actions` (`645-762`), and `totals` come from
   `refresh_summary` (`src/web/state.rs:383-406`) over the same rows.
7. **Admission never reads the projected rows.** Mark, bulk mark, queue, Start,
   retry, stop-and-dequeue, force-stop-change, and resolve admission classify
   from the reducer, the mark store, and the parallel runtime
   (`src/orchestration/operator_command.rs:224-292`, `2236`, `2319`); mark
   reconciliation on refresh reads the event payload and the reducer
   (`src/orchestration/mark_reconciliation.rs:166-217`). The only production
   readers of projected rows outside `reads.rs` are the text-edit scope
   (`text_edit.rs:863-885`, which already refuses completed rows) and the
   refresh preview (`proposal_refresh.rs:98-127`).

## 2. Goals and non-goals

Goals: a change this owner published and the reducer reports `merged` or
`pushed` stays in the published list for the rest of the incarnation, coherent
across list/get/state/execution-status/stream; the in-flight integration window
cannot lose it; nothing about admission, aggregates' meaning, clocks, or timing
changes; the guarantee is discoverable and additive.

Non-goals: wait/error retention, historical archives, persistence or TTL,
per-row flags, new routes or commands, TUI or reducer changes, Server code.

## 3. Decisions

### D1. Retention lives in `WebState`'s stored snapshot

Carried rows are ordinary `ChangeStatus` entries in `WebState.state.changes`,
so every existing consumer — event-arm updates, reducer enrichment,
`refresh_summary`, `operator_snapshot_with`, `contextual_run_clock_step`,
`project_snapshot`, the event ring — sees them with no second code path.

Rejected alternatives and why:

- *Projection-layer overlay* (append remembered rows in
  `operator_snapshot_with`): the facts loop would overwrite timing with
  defaults for pruned IDs, the run-clock row set would diverge from the
  published rows, and a second row source would exist.
- *A completed-history resource or route*: extra API surface the requirement
  does not need; the Server already reads `changes`.
- *A per-row `retained` member*: a workspace-listing artifact clients must not
  branch on; no consumer needs it.
- *Persistence, TTL, or loading `openspec/changes/archive/`*: forbidden by the
  requirement and by Law 1; the reducer already answers for the incarnation.
- *Reading TUI rows*: the TUI's set is broader and dismissal-filtered; TUI
  dismissal must never remove an API row.
- *Retaining every `archive_complete` row or copying the TUI rule*: publishes
  waits and errors, which this change excludes.

### D2. What is retained

```rust
/// Reducer-authored display statuses that keep an already published row for the
/// rest of this owner incarnation once a workspace observation stops listing it.
pub const RETAINED_COMPLETION_STATUSES: [&str; 2] = ["merged", "pushed"];

/// Reducer-authored display statuses that keep an already published row through
/// workspace absence only until it settles: the owner is integrating the change
/// into the base right now.
pub const SETTLING_RETENTION_STATUSES: [&str; 1] = ["resolving"];

/// Lifetime published for retained rows: exactly this `instance_id`.
pub const COMPLETED_CHANGE_RETENTION_SCOPE: &str = "instance";

fn keeps_absent_row(display_status: &str) -> bool {
    RETAINED_COMPLETION_STATUSES.contains(&display_status)
        || SETTLING_RETENTION_STATUSES.contains(&display_status)
}
```

All three constants live in `src/web/state.rs` and are `pub` (the contract test
crate reads them through `completed_change_retention_capability()`).

- `merged` and `pushed` are the completed statuses the reducer can author; both
  satisfy the shared `is_completed_status`. `archived` is in the shared
  completed vocabulary but the reducer never authors it, so it is not listed and
  archive alone can never qualify.
- `resolving` is included because of fact 4: without it the standard merge path
  (and every `on_merged` hook duration) can drop the row before `MergeCompleted`
  exists. It is an active status, not a wait or an error. A row kept as
  `resolving` that settles into anything other than `merged`/`pushed` is
  released by the next observation that does not list it.
- Everything else absent is released exactly as today: `not queued` (whatever
  the task counts), `queued`, `blocked`, `stalled`, `preparing`, `applying`,
  `accepting`, `rejecting`, `archiving`, `merge wait`, `resolve pending`,
  `reject pending`, `error`, `rejected`, `stopped`.

### D3. Evidence

For a previously published row absent from the observation, the deciding
status is:

1. when the observation holds reducer state — the dispatch's authoritative
   `dispatch.state`, or the existing non-blocking `try_read` of the shared
   reducer — `reducer.display_status(&row.id)` (`"not queued"` for an untracked
   ID);
2. otherwise (no reducer bound, or the non-blocking read failed) the row's own
   last reducer-derived status, `row.queue_status.as_deref().unwrap_or("not
   queued")`.

This mirrors the existing disk-path rule for listed rows ("preserve
`queue_status` only when shared reducer state did not provide one",
`src/web/state.rs:1157-1160`). No other source is consulted: not absence, task
counts, the archive directory, `archive_complete`, `observed_statuses`, or the
TUI. Because `merged`/`pushed` are immutable in the reducer for the
incarnation (fact 3), a readable reducer can never downgrade a retained
completed row.

### D4. The carry helper

```rust
/// Append to `rows` (the observation's rows, already built) every row of
/// `previous` that `rows` does not list and whose deciding status (D3) keeps it
/// (`keeps_absent_row`), in `previous` order. Each carried row is `previous`'s
/// row unchanged except `queue_status = Some(deciding status)`. An ID is carried
/// at most once and never when `rows` lists it. Returns the carried IDs in order.
fn carry_retained_rows(
    rows: &mut Vec<ChangeStatus>,
    previous: &[ChangeStatus],
    reducer: Option<&crate::orchestration::state::OrchestratorState>,
) -> Vec<String>
```

Order rule: observation rows first in observation order, then carried rows in
their previously published relative order. With no carried row the result is
exactly today's list, so the order of every non-retaining test is unchanged.
Repeated identical observations therefore produce identical lists.

### D5. Event path (`apply_dispatch`)

- Before the `state` write scope declare `let mut carried_ids: Vec<String> =
  Vec::new();`.
- In the `ChangesRefreshed` arm, after the existing per-row preservation loop
  and before `state.changes = new_change_statuses`, call
  `carry_retained_rows(&mut new_change_statuses, &state.changes, reducer)` where
  `reducer` is `dispatch.state` when present, otherwise the same non-blocking
  read the post-match enrichment already performs (`src/web/state.rs:1659-1664`),
  otherwise `None`. Store the result in `carried_ids`. The existing
  `refresh_summary`, `updated = true`, and post-match
  `apply_reducer_derived_queue_statuses` stay as they are and now cover the
  carried rows too.
- `absorb_operator_facts` gains a `carried_ids: &[String]` parameter (empty for
  every other event); its `ChangesRefreshed` branch observes
  `changes ∪ rejected_changes ∪ carried_ids`. Nothing else in it changes.

### D6. Disk path (`merge_workspace_observation`)

- Read the previous snapshot (`old_changes`, `old_app_mode`,
  `old_persistent_idle`, `old_is_resolving`) first, then take the one
  non-blocking reducer read, so the observation's enrichment and the carry
  decision share it and no reducer guard is held across an `.await`.
- Build `new_state` exactly as today (`from_changes_with_shared_state`, mode and
  `is_resolving` preservation, per-row preservation loop), then call
  `carry_retained_rows(&mut new_state.changes, &old_changes, reducer)` with that
  read (or `None`), then `refresh_summary(&mut new_state)` again so the totals
  include carried rows.
- Collect the published IDs (`new_state.changes`), drop the reducer guards,
  write the snapshot, and observe those IDs in the facts store:
  `observe_changes_for_attention` takes the published row IDs instead of the
  listing. `refresh_from_disk_at`, `update`, and
  `seed_workspace_observation_for_tests` inherit the behavior unchanged.

### D7. Facts

Because carried IDs are observed, a retained row keeps `timing`,
`latest_activity`, `newly_detected`/attention, and stays in
`known_change_ids`, so a reappearance is not "new" (the TUI keeps known IDs of
retained rows for the same reason). `net_active` and `observed_statuses` were
never pruned. The workspace-observation facts keep their existing rules:
`apply_worktree_paths` sets `worktree` to `null` for an ID the observation
gives no path for, and `apply_parallel_eligibility` reports `not_committed` for
an ID absent from `HEAD`. These are current observations, not guesses, and a
final status blocks every action regardless (`classify_actions`).
`OperatorFactsStore` itself is not modified.

### D8. Enrichment, reappearance, summary, timing

- Reducer enrichment (`apply_reducer_derived_queue_statuses` on the stored
  snapshot, and on the clone in `operator_snapshot_with`) runs over carried
  rows like any other row: `queue_status`, `archive_complete`, `queue_intent`,
  `blocker`, `error_detail`, `acceptance_provenance`; `iteration_number` is only
  overwritten by a positive apply count, which `mark_archived` removed, so the
  last known value stays.
- When the projection cannot read the reducer it publishes the stored
  `queue_status`, which for a carried row is the deciding status (D4).
- Reappearance: an observation that lists a carried ID builds that row from the
  observation; the existing preservation loop finds the carried row as the
  previous row and keeps `iteration_number` and non-zero progress over a 0/0
  listing. The carry step then skips the ID. One row, the observation's listing
  fields, the reducer's status (still `merged` within the incarnation, exactly
  as the TUI shows), no new-arrival attention.
- `refresh_summary` runs after the carry in both paths.
- `ChangeTiming` boundaries change only on the existing run start/end events;
  retention adds none.

### D9. Coherence

Carrying happens before the existing single projection transaction
(`project_execution_event` → `apply_state` for a dispatch,
`sync_remote_control_projection` → `apply_state_if_changed` for the disk path),
so a carried row appears in the same `state_revision` as its observation, and an
identical repeated observation produces an equal candidate and no revision.
`changes_refreshed` payloads keep counting the observation
(`projection.rs:1289-1297`); the snapshot at the event's revision is the
authority for rows. `GET /api/v2/execution-status` lists the same rows because
it iterates `snapshot.changes`.

### D10. Non-authority

Retention changes the row set only. Admission (fact 7) reads none of it.
`classify_actions` blocks every action of a `merged`/`pushed` row
(`FinalStatus` and the shared force-stop/refresh refusals). Totals keep their
shared classifiers: retained `merged`/`pushed` rows count as `completed`, a
retained `resolving` row counts as `in_progress` exactly as when listed, and
`pending` counts only `queued`. The run clock's resolve-side rule
(`contextual_run_clock_step`) now sees a retained `resolving` row as processing,
which is what the TUI's `try_transition_to_select` already sees because the TUI
keeps that row; completed rows are not processing. `net_active` observes
`merged`/`pushed` as a closed status, so nothing reopens or resets. Retention
introduces no mark revocation or creation: `execution_marked` on a retained row
is the shared store's value (an archived change keeps its mark by existing
spec), and Start/mark admission refuse terminal targets as before.

### D11. Lifetime

The carried rows live in the `WebState` of one process. A new process starts
with `WebState::new(initial)` and a fresh reducer and projection (new
`instance_id`), observes only `openspec/changes/*`, and therefore publishes no
row for a change an earlier incarnation merged. Nothing is written anywhere.

### D12. Capability block

`src/web/remote_control_api/dto.rs`:

```rust
/// Whether completed changes this owner published stay in `changes` after the
/// workspace stops listing them, and for how long.
///
/// `Default` is the *unavailable* answer, which is what an older owner's
/// omission decodes as: such an owner may drop a merged row as soon as its
/// proposal leaves `openspec/changes/`. Hiding or restoring rows is client-local
/// and needs no command; key that state by `instance_id`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct CompletedChangeRetentionCapability {
    /// True when this owner keeps every published `merged` or `pushed` row for
    /// the rest of its incarnation.
    pub available: bool,
    /// Reducer-authored statuses that keep a published row for the rest of the
    /// incarnation: exactly `merged` and `pushed`.
    pub retained_statuses: Vec<String>,
    /// Reducer-authored statuses that keep a published row through workspace
    /// absence only until it settles: exactly `resolving`.
    pub settling_statuses: Vec<String>,
    /// Lifetime of retained rows: always `instance` — exactly as long as this
    /// `instance_id`; never persisted and never rehydrated by a later instance.
    pub scope: String,
}
```

`CapabilitiesResponse` gains, after `worktree_sessions`:

```rust
    /// Completed-change retention guarantee of this owner.
    ///
    /// `#[serde(default)]` so a client can still read an older owner, where the
    /// absent field decodes as `available: false` — never as available.
    #[serde(default)]
    pub completed_change_retention: CompletedChangeRetentionCapability,
```

`src/web/state.rs` gains `pub fn completed_change_retention_capability() ->
CompletedChangeRetentionCapability` returning `available: true`, the two
constant lists, and `COMPLETED_CHANGE_RETENTION_SCOPE`. `reads::capabilities`
publishes it unconditionally (retention is a property of this build's
projection, like `proposal_refresh.preview_available`). `src/web/openapi.rs`
registers `CompletedChangeRetentionCapability` in `components(schemas(...))`.
`src/client/session.rs` `observation_for_test` sets
`completed_change_retention: Default::default()`; no client code reads the block.

Descriptions (they are the OpenAPI property descriptions):

- `InstanceSnapshot.changes` and `ChangesResponse.changes`: "Projected changes:
  every change the current workspace observation lists, followed by every row
  this owner incarnation already published that the observation no longer
  lists while the reducer reports it `merged` or `pushed` (kept for the rest of
  the incarnation) or `resolving` (kept until it settles). See
  `CapabilitiesResponse.completed_change_retention`."
- `SnapshotTotals`: "Aggregate counts over the projected changes, rows kept by
  `completed_change_retention` included."

### D13. Observable consequences on existing surfaces

For a change this owner merged or pushed in this incarnation, after its
proposal left the listing (all consistent with how the same row behaves while
it is still listed today):

- `GET /api/v2/changes/{id}` answers 200 with the row instead of 404;
  `GET /api/v2/execution-status` lists it.
- `GET /api/v2/proposals/{id}/refresh-preview` answers 200 with `eligible:
  false`, `refusal: terminal_target` instead of 404; the change text route still
  answers 404 `not_found`.
- `cflx client mark|unmark <id>` submits and settles `unchanged` with the
  non-markable-target reason instead of a local `change_not_found`;
  `force-stop-change` and `refresh-proposal` return `target_ineligible` instead
  of `change_not_found`.
- `cflx client wait <id>` certifies the `merged`/`pushed` row against the
  repository as before; when certification fails the outcome is the existing
  `change_requires_action` for a settled row rather than `change_not_found`.
- `totals.total`/`totals.completed` include retained rows.

No member, schema, or meaning changes. These consequences are documented, not
coded.

### D14. Failure behavior

- A refresh that fails before producing an observation (`list_changes_native_from`
  error, e.g. `openspec/changes` is a file) returns `Err` before touching the
  snapshot: no row and no revision changes. The TUI refresh task reports a
  failure as a log and dispatches no `ChangesRefreshed`.
- An observation that lists nothing (`openspec/changes` absent) releases every
  non-retained row and keeps retained ones.
- No reducer readable: D3 rule 2. No reducer bound at all: stored statuses were
  never reducer-derived, so nothing is retained.
- An untracked ID under a readable reducer is `not queued` and is released.

### Known limitation (deliberate)

A post-archive row released after settling into `merge wait` (an `on_merged`
failure) or `error` (a publication failure) is not republished when a later
explicit resolve or retry completes it, because the row is no longer in the
list and its listing never returns. Closing that gap requires retaining those
waits/errors or a hidden memory of released rows, both excluded here. The
behavior is pinned by `retain_api_completed_resolving_row_is_kept_only_until_it_settles`
and recorded as an open product decision in `proposal.md` Future Work.

## 4. Change boundary

May change: `src/web/state.rs`, `src/web/remote_control_api/dto.rs`,
`src/web/remote_control_api/reads.rs`, `src/web/openapi.rs`,
`src/client/session.rs` (test helper field only), the test files in section 5,
`docs/guides/USAGE.md`, `AGENTS.md`.

Must not change: `src/orchestration/**`, `src/tui/**`, `src/parallel/**`,
`src/web/operator_facts.rs`, `src/web/remote_control_api/projection.rs`,
`stream.rs`, `commands.rs`, `executor.rs`, `text_edit.rs`,
`proposal_refresh.rs`, `src/client/**` outside the helper field, `web/**`,
`skills/**`, `Cargo.toml`, `Cargo.lock`, any configuration, and the
constitution.

## 5. Test plan

Every new test name starts with `retain_api_completed_`. Instants are injected
with the existing `pin_observation_clock_for_tests` / progress-timing `Clock`
seams; no assertion depends on wall-clock timing. Shared lifecycle sequences
used below: **merge(id)** = `ApplyStarted`, `ApplyCompleted{revision}`,
`ArchiveStarted`, `ChangeArchived`, `ResolveStarted`, `MergeCompleted{revision}`;
**push(id)** = the same up to `ChangeArchived`, then `PushStarted{remote,
branch}`, `PushCompleted{remote, branch}`. Each change completes before the next
is archived, so the resolve lane is free.

### 5.1 `src/web/state.rs` (`mod tests`, unit)

Helpers: a `WebState::new(&[])` bound to an `OrchestratorState` reducer, an
`deliver` that applies an event to the reducer and then to
`apply_execution_event`, and a `ChangesRefreshed` builder.

1. `retain_api_completed_statuses_are_the_reducer_authored_completion_vocabulary`
   — the two constants equal `["merged","pushed"]` and `["resolving"]`; each
   retained status satisfies `is_completed_status`, `resolving` satisfies
   `is_active_status`, `archived` is completed but not retained; a reducer
   driven through merge(id)/push(id) authors `merged`/`pushed`, and through
   `ChangeArchived` alone authors `resolving`;
   `completed_change_retention_capability()` equals `{available: true,
   retained_statuses: [merged, pushed], settling_statuses: [resolving], scope:
   instance}`.
2. `retain_api_completed_refresh_event_keeps_merged_and_pushed_rows` — rows
   alpha 2/2, beta 1/1, gamma 0/3; merge(alpha), push(beta); three
   `ChangesRefreshed{[gamma]}`: stored and projected rows are `[gamma, alpha,
   beta]`, alpha `merged` and beta `pushed` with their task counts, progress,
   `archive_complete: true`, and the `iteration_number` captured before
   absence; the projection revision after the third refresh equals the one
   after the first.
3. `retain_api_completed_disk_observation_keeps_merged_and_pushed_rows` — the
   same through three `update(&[gamma])` calls (the `merge_workspace_observation`
   path).
4. `retain_api_completed_never_retains_without_reducer_completion` — absent
   rows `done` (3/3, `not queued`), `waiting` (archived, then `HookFailed{hook_type:
   "on_merged"}` → `merge wait`), `pending` (archived while a still-listed row
   `lane` is `resolving` → `resolve pending`), `failed` (`ProcessingError`), `rejected`
   (`ChangeRejected`), `stopped` (`ReducerCommand::StopChange`), and `orphan` on a
   second `WebState` with no reducer bound (its stored `queue_status` is
   `None`): after one event refresh and one `update` that list only `lane`, none
   of them is published and `lane` is.
5. `retain_api_completed_resolving_row_is_kept_only_until_it_settles` — alpha
   archived and `ResolveStarted`, refresh without alpha → kept as `resolving`
   and counted `in_progress`; `MergeCompleted` → `merged`, kept over two more
   refreshes. delta archived (`resolving`), refresh without delta → kept;
   `HookFailed{on_merged}` → next refresh releases delta; a later
   `MergeCompleted{delta}` does not republish it (the known limitation).
6. `retain_api_completed_reappearance_updates_one_row_in_place` — alpha merged
   and retained with 2/2; a refresh listing alpha 1/3 again yields exactly one
   alpha, 1/3, `merged`, attention `none`, in observation order; a refresh
   listing alpha 0/0 keeps 1/3; a refresh without alpha retains it with 1/3.
7. `retain_api_completed_order_is_observation_then_retained_and_stable` — rows
   a, b, c, d; merge(b), merge(d); refresh `[a, c]` → `[a, c, b, d]`; three
   repeats → same order and no revision change; refresh `[a, c, e]` →
   `[a, c, e, b, d]`.
8. `retain_api_completed_contended_reducer_keeps_the_last_reducer_status` —
   alpha merged (stored `merged`), gamma `not queued`; while the test holds the
   reducer write guard it calls `update(&[beta])` and
   `apply_execution_event(ChangesRefreshed{[beta]})`: alpha is kept as `merged`,
   gamma is released; after dropping the guard and
   `sync_remote_control_projection`, alpha is still `merged`.
9. `retain_api_completed_failed_or_empty_refresh_keeps_completed_rows` — alpha
   merged and retained, gamma listed; `refresh_from_disk_at` on a
   `tempfile::tempdir()` root whose `openspec/changes` is a regular file returns
   `Err` and leaves the stored snapshot, the projected snapshot, and the
   revision unchanged; on a temp root without `openspec/changes` it returns `Ok`,
   alpha is kept and gamma released.
10. `retain_api_completed_facts_survive_absence` — with pinned instants,
    `ProcessingStarted` and merge(alpha) produce `timing` with both boundaries,
    `latest_activity.event_type == "merge_completed"`, and closed `net_active`
    with `accumulated_ms > 0`; after three event refreshes and two `update`s
    without alpha, the projected `timing`, `latest_activity`, `net_active`, and
    `attention` are equal to the values captured before absence, `worktree` is
    `null`, and `parallel.blocked_reason` is `not_committed`.

### 5.2 `src/web/remote_control_api/tests/operator_snapshot_tests.rs` (integration)

Helper `retention_owner(ids)`: `Arc<WebState>` bound to a reducer, an
`ExecutionMarkStore`, and repo root `/repo`; an `EventDispatcher::new(reducer,
vec![Arc::new(WebEventSink::new(web))])`; the production router
`crate::web::remote_control_router(&WebConfig::enabled(0, "127.0.0.1".into()),
web)`; an initial dispatched `ChangesRefreshed` listing `ids`. Requests use the
existing `get`/`send`/`status_and_json` helpers.

11. `retain_api_completed_list_get_state_and_stream_agree_across_refreshes` —
    dispatch merge(alpha), record cursor `c0`, dispatch a refresh without alpha,
    read `/api/v2/state` → revision `R`, sequence `S`. Three times: dispatch a
    refresh without alpha and call `update` without alpha; then
    `/api/v2/changes`, `/api/v2/changes/alpha` (200), and `/api/v2/state` carry
    one `instance_id`, `state_revision == R`, and an identical alpha row
    (`merged`). `projection.events_after(S)` replays only events at revision
    `R`, every `changes_refreshed` payload has `changes: 1`. The SSE route
    `/api/v2/events?after_sequence={c0}` replays the same sequences as
    `events_after(c0)` (bounded frame count, generous per-chunk safeguard
    timeout) and its last frame carries `R`. Reconnect: `/api/v2/state` → `S2`,
    `events_after(S2)` is empty, one more refresh adds exactly one
    `changes_refreshed` at `R`, and alpha is still published.
12. `retain_api_completed_reads_and_reconnect_have_no_side_effects` — with alpha
    retained, capture revision, event sequence, `registry_sizes()`, the
    reducer's `all_display_statuses()`, and `marked_ids()`; read twice each of
    `/api/v2/state`, `/api/v2/changes`, `/api/v2/changes/alpha`,
    `/api/v2/capabilities`, `/api/v2/execution-status`,
    `/api/v2/proposals/alpha/refresh-preview?source_ref=main`,
    `/api/v2/changes/alpha/text?path=proposal.md`, and the SSE replay; the
    captured values are unchanged; the preview is 200 with `eligible: false`,
    `refusal: terminal_target`; the text read is 404 `not_found`; the
    capabilities block equals `completed_change_retention_capability()`;
    execution status lists alpha.
13. `retain_api_completed_totals_actions_and_clocks_stay_truthful` — rows alpha,
    beta, gamma; an accepted `RunDispatched` opens the run clock at a pinned
    instant, merge(alpha) at pinned instants, `AllCompleted` freezes it; capture
    `run_clock` and alpha `net_active`; after three refreshes and two `update`s
    without alpha: totals `{total: 3, completed: 1, in_progress: 0, pending: 0}`,
    alpha `actions == change_actions_for_test(app_mode, "merged", None)` with
    every member `allowed: false`, `run_clock` and alpha `net_active` equal the
    captured values, and the revision did not move; after `AddToQueue(beta)` and
    one refresh, `pending: 1` and alpha is still counted only as completed.
14. `retain_api_completed_new_owner_starts_without_previous_rows` — owner A
    retains alpha; owner B, built fresh over a listing without alpha, publishes
    no alpha, publishes beta, has a different `instance_id`, and reports
    `scope: instance`; owner A still publishes alpha.

### 5.3 `src/web/remote_control_api/tests/progress_timing_tests.rs` (integration, TUI parity)

Reuse `Fixture`, `Clock`, `start`, `web_event`, `sync_tui`, and
`assert_net_active_parity`. Add `refresh_both(seconds, listed)`: apply one
`ChangesRefreshed` to the reducer and the web store at `seconds`, then
`sync_tui(seconds)`, then `tui.handle_orchestrator_event(event)` — the order the
runner delivers it.

15. `retain_api_completed_tui_and_api_retain_the_same_completed_rows` — fixture
    alpha, beta, gamma; start at 0; alpha merge at 1-5 with `ResolveStarted` at
    5; `refresh_both(6, [beta, gamma])` keeps alpha `resolving` on both;
    `MergeCompleted` at 7; beta archive/`PushStarted` at 8-10;
    `refresh_both(11, [gamma])` keeps beta `resolving` on both; `PushCompleted`
    at 12; `refresh_both` at 13, 14, 15 with `[gamma]`. At every refresh the
    TUI row-ID set equals the API row-ID set; for alpha and beta the TUI
    `display_status_cache`, `archive_complete_cache`, `completed_tasks`,
    `total_tasks` equal the API `display_status`, `archive_complete`,
    `completed_tasks`, `total_tasks`; `assert_net_active_parity` holds; the API
    `timing` equals the value captured at completion.
16. `retain_api_completed_tui_dismissal_leaves_the_api_row` — same arrangement
    through 13; capture the API snapshot and revision; focus alpha
    (`cursor_index` = alpha's TUI index) and `dismiss_merged_row()`; the TUI
    rows lack alpha, the API snapshot and revision are unchanged;
    `refresh_both(14, [gamma])` keeps alpha hidden in the TUI and published,
    unchanged, in the API; `dismiss_all_merged_rows()` leaves the API unchanged.

### 5.4 `src/web/remote_control_api/tests/parallel_control_tests.rs` (integration, admission)

17. `retain_api_completed_retained_row_admission_equals_listed_completed_row` —
    `completed_alpha(retained)` builds `Wired::new(&["alpha","beta"])`, observes
    both in `running`, sets alpha's mark, applies merge(alpha) to the reducer
    and the web store, and, when `retained`, observes `[beta]` only; alpha is
    projected `merged` either way. For each of `SetExecutionMark{alpha, true}`,
    `SetExecutionMark{alpha, false}`, `SetAllExecutionMarks{}`,
    `SetQueueIntent{alpha, true}`, `RetryChange{alpha}`,
    `RetryErrors{[alpha]}`, `StopAndDequeue{alpha}`, `ForceStopChange{alpha}`,
    `ResolveMerge{alpha}`, and `Start`, a fresh listed fixture and a fresh
    retained fixture execute it through `SharedServiceExecutor`; the settlement
    (`changed` flag or error code), `marked_ids()`, scheduler calls, both
    reducer statuses, and alpha's queue intent are equal between the two; in
    the retained fixture alpha stays `merged` and marked and no scheduler call
    targets it; alpha's projected `actions` are equal between the two.

### 5.5 `src/web/remote_control_api/tests/dto_tests.rs` (compatibility fixture)

18. `retain_api_completed_older_owner_capabilities_decode_as_unavailable` — in
    `legacy_owner_shape`, `legacy_capabilities()` decodes with
    `completed_change_retention == CompletedChangeRetentionCapability::default()`;
    the same payload without `limits` is still rejected naming `limits`.
19. `retain_api_completed_current_capabilities_publish_the_block` — `GET
    /api/v2/capabilities` on the test harness router serializes
    `completed_change_retention` equal to
    `completed_change_retention_capability()` and round-trips through
    `CapabilitiesResponse`; `SUPPORTED_COMMANDS` still has 23 entries and none
    contains `dismiss`, `hide`, `restore`, or `retain`.

### 5.6 `tests/openapi_contract_tests.rs` (integration)

20. `retain_api_completed_capability_is_published_and_optional` —
    `retention_contract_errors(&doc)` is empty: `CompletedChangeRetentionCapability`
    publishes exactly and requires `available` (boolean), `retained_statuses`
    and `settling_statuses` (string arrays), and `scope` (string);
    `CapabilitiesResponse.completed_change_retention` references it and is not
    required; the `changes` descriptions of `InstanceSnapshot` and
    `ChangesResponse` contain `completed_change_retention`. The value of
    `conflux::web::state::completed_change_retention_capability()` validates
    against the schema with the shared `validate` helper and equals the literal
    `{available: true, retained_statuses: [merged, pushed], settling_statuses:
    [resolving], scope: instance}`.
21. `retain_api_completed_omitting_the_capability_fails_the_contract` — a
    document with the member removed, and one with it added to `required`, each
    make `retention_contract_errors` name `completed_change_retention`.

### 5.7 `tests/e2e_git_worktree_tests.rs` (integration, real Git, `heavy-tests` file)

22. `retain_api_completed_git_merge_survives_production_refreshes` — uses
    `stale_refresh_support`; fails (does not skip) when the fixture cannot be
    built. Declare alpha (`tasks.md` `- [x] one`/`- [x] two`) and beta
    (`- [ ] one`) on `main`; bind a reducer to `Arc<WebState>`, an
    `EventDispatcher` with `WebEventSink`, and
    `conflux::web::remote_control_router`. `refresh_from_disk_at(&repo)`
    publishes alpha 2/2 and beta 0/1. A `production_refresh` dispatches
    `ChangesRefreshed` built from `openspec::list_changes_native_from` and
    `list_rejected_changes_native_from` (the two reads the TUI refresh task
    performs) with `committed_change_ids` = the listed IDs. Dispatch
    `ProcessingStarted`, `ApplyStarted`, `ApplyCompleted`, `ArchiveStarted`,
    `ChangeArchived`, `ResolveStarted` for alpha. On an `add_worktree("alpha")`
    worktree run `git mv openspec/changes/alpha
    openspec/changes/archive/2026-11-06-alpha`, commit, and `git merge --no-ff`
    the branch into `main`; the base no longer has `openspec/changes/alpha`.
    `refresh_from_disk_at` plus `production_refresh` keep alpha `resolving` with
    2/2 and `archive_complete: true`; dispatch `MergeCompleted`; three rounds of
    `refresh_from_disk_at` plus `production_refresh` keep alpha `merged` with
    2/2 and `archive_complete: true` and no revision change after the first
    round; `/api/v2/changes`, `/api/v2/changes/alpha`, and `/api/v2/state`
    agree. `tree_snapshot(&repo)`, `git rev-parse HEAD`, `git status
    --porcelain=v1 -uall`, and `git for-each-ref` are identical before and after
    the reads and the last refreshes. A fresh reducer and `WebState` refreshed
    from the same repository publish beta and no alpha, with a different
    `instance_id`.

## 6. Expected edits to existing tests

None are expected; the only existing-file edit outside new cases is the
`completed_change_retention: Default::default()` field in
`src/client/session.rs::observation_for_test`. Candidates were checked at the
base: `test_update_drops_an_archived_change` (no reducer, so nothing is
retained), the client wait disappearance tests (synthetic snapshots written to
the projection directly), and the progress-timing, mark-event, and
cross-adapter suites (rows stay listed). If an existing test fails only because
a retained row is now present, it may be updated to assert that retained row,
and each such edit is listed in `tasks.md` Notes with its reason; no other
existing assertion may change.

## 7. Gates and execution environment (host asagi)

Declared gates (one focused command each, distinct selections):

| Verification ID | Command (leading assignments apply to the child only) |
| --- | --- |
| `retain-api-completed-owner` | `TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --lib --features web-monitoring retain_api_completed -- --nocapture` |
| `retain-api-completed-contract` | same prefix, `/home/tumf/.cargo/bin/cargo test --features web-monitoring --test openapi_contract_tests retain_api_completed -- --nocapture` |
| `retain-api-completed-git` | same prefix, `/home/tumf/.cargo/bin/cargo test --features web-monitoring,heavy-tests --test e2e_git_worktree_tests retain_api_completed -- --nocapture` |

- `/tmp` on asagi refuses writes under the user's quota, so every child uses
  `TMPDIR`/`TMP`/`TEMP=/home/tumf/tmp/work/rt-retain` (short, so socket
  fixtures stay under the path limit) and builds into
  `CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target` (SSD).
  `/home/tumf/tmp/work/rt-retain` must exist before a gate starts
  (`mkdir -p /home/tumf/tmp/work/rt-retain`); a missing directory makes the
  build fail closed.
- Executables resolve to `/home/tumf/.cargo/bin/cargo` (cargo 1.99.1) and
  `/usr/bin/git`. No Python is involved; on asagi Python is only ever run
  through `uv`.
- A gate passes only with a nonzero passed count and zero failures, read from
  the command's own output and exit status, never from a wrapper.
- Raw gate and observation output is kept outside the repository under
  `/home/tumf/tmp/work/retain-api-completed-changes-evidence/`.
- Run long commands through agent-exec and wait for terminal completion; do not
  poll with sleeps.

## 8. Independent review launch contract (for the implementation worker)

Carried verbatim from the request:

```text
HOME=/home/tumf PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin; /home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file <absolute UUID prompt under /home/tumf/tmp/work/> -- /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose --include-partial-messages --dangerously-skip-permissions --print ; no --model, direct claude or built-in Agent/Task fable. Wait existing running job, no kill/duplicate. Return raw job path/argv/actual init and responding model/final verdict. This is independent implementation review later, not author self-review. Focused read-only review <=10 shell calls/10 extra files, no external search/services. No push/install/owner restart.
```

Operational reading of the same contract:

- Write the prompt to an absolute path such as
  `/home/tumf/tmp/work/retain-api-completed-changes-evidence/review-<uuid>.md`
  (a fresh UUID, under `/home/tumf/tmp/work/`), and launch exactly the command
  above with that path. Never pass `--model`, never start `claude` directly,
  and never use the built-in Agent/Task tools or a Fable subagent instead.
- If `run` returns while the job is still running, wait for that same job
  (`/home/tumf/.cargo/bin/agent-exec wait --forever <job_id>`, or the agent-exec
  MCP `wait` once). Never kill, abandon, restart, or duplicate it.
- Record in `tasks.md` Notes: the job ID, the raw job directory and its
  `stdout.log` path, the exact argv, the model named by the stream's `init`
  event, the model that actually responded, and the reviewer's final verdict.
- The reviewer reviews the change-scoped implementation diff against this
  design, read-only, within 10 shell calls and 10 files beyond the diff, with
  no external search or service. The review is independent of the
  implementation session and is not the author's self-review; a wrapper or
  `wait` exit status `0` is not acceptance.
- Neither the worker nor the reviewer pushes, releases, installs, edits global
  configuration, or restarts or replaces an owner.

## 9. Rollout and downstream notes

The change is additive. An older owner keeps dropping rows; a consumer reads
`completed_change_retention.available` and must treat rows as possibly
transient when it is `false`, without refusing to start. A consumer that hides
rows locally (for example the Server's OP17/OP18 individual and bulk
dismiss/restore) keys that state by `instance_id` and clears or rekeys it when
`instance_id` changes; no Server code changes here. Running owners are not
restarted by this change.
