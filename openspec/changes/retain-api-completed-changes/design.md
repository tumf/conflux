# Design: retain completed changes in the `/api/v2` projection

Base: `06faae562d1d53b2723d0912f4095d3fd0d49d7e` (`v0.6.327` sources; the
deployed rc.2 owner adds release-only metadata on top of the same runtime).
Line numbers below are at that base.

## 1. Source facts this design is built on

1. **Two rebuild paths, both wholesale, and the only places a row leaves.**
   - Event path: `WebState::apply_dispatch` → `ExecutionEvent::ChangesRefreshed`
     arm (`src/web/state.rs:1486-1523`) builds `new_change_statuses` from the
     event's `changes`, copies `iteration_number` and 0/0-progress from the
     previous row of the same ID, then `state.changes = new_change_statuses`.
     In the TUI owner this event comes from the refresh task
     (`src/tui/runner.rs:915-1110`, every `AUTO_REFRESH_INTERVAL_SECS` = 5 s)
     through the process dispatcher (`src/tui/runner.rs:626-653`), so the
     dispatch carries the authoritative reducer state. The headless forwarder,
     disk refreshes, and tests use `apply_execution_event` (`1191-1198`), which
     carries none.
   - Disk path: the Web/UDS refresh task (`src/web/mod.rs:521-541`, every
     `refresh_interval_secs` = 5 s) calls `refresh_from_disk_at`
     (`src/web/state.rs:1803-1888`), which lists `openspec/changes/*`
     (`openspec::list_changes_native_from`, skipping `archive/`) and calls
     `merge_workspace_observation` (`1108-1189`). That takes the non-blocking
     reducer read (`1110-1122`), builds a snapshot with
     `from_changes_with_shared_state`, clones the previous snapshot under a
     read guard (`1125-1133`), and replaces `*state` under a later write guard
     (`1182-1185`). A dispatch applied between the clone and the write is
     overwritten.
   - Rows leave the published list only at `state.changes =
     new_change_statuses` (`1520`) and `*state = new_state` (`1184`); no other
     arm removes or replaces rows. Neither keeps an ID the observation does not
     list.
2. **Facts are pruned with the rows, and only published rows read them.**
   `OperatorFactsStore::observe_changes` (`src/web/operator_facts.rs:255-280`)
   retains `known_change_ids` and `facts` (timing, latest activity, attention,
   parallel, worktree) only for the observed IDs; an ID inserted after the
   baseline observation is newly detected, so "a re-created change is genuinely
   new again". `net_active` and `observed_statuses` are separate maps that are
   never pruned (`124-135`). The event path observes `changes ∪
   rejected_changes` (`src/web/state.rs:1766-1782`); the disk path observes the
   listing (`observe_changes_for_attention`, `831-836`).
   `operator_snapshot_with` reads `facts(id)` and `net_active(id)` only for the
   rows it projects (`926-947`) and feeds `observe_display_statuses_at` only
   those rows' statuses (`886-909`). `reproject_worktrees` writes only
   `facts[id].worktree`, from the observation's `worktree_paths`
   (`src/web/operator_facts.rs:325-338`); `worktree_owner` and `worktree_path`
   read `worktree_paths`, never `facts` (`161-176`). Nothing else enumerates
   `facts`.
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
4. **Completion is authored after the base loses the proposal — also on
   recovery.** `attempt_merge` emits `ResolveStarted` for each change, then
   merges into the base (`src/parallel/merge.rs:1050-1065`);
   `handle_merge_and_cleanup` then runs `on_merged` and only afterwards sends
   `MergeCompleted` (`541-639`). With upstream publication the terminal is
   `PushCompleted` after `PushStarted` (reducer `2822-2839`). During that window
   the reducer status is `resolving` and the base no longer lists the change.
   Recovery happens in the same absence: an `on_merged` failure blocks the
   merged transition (`src/parallel/merge.rs:209-211`) and the reducer authors
   `merge wait` (`src/orchestration/state.rs:2808-2811`); an explicit resolve
   (`ReducerCommand::ResolveMerge`, admitted at `2225-2247` and applied the way
   `src/orchestration/run_control.rs:1131` applies it) then reaches
   `MergeCompleted` → `merged` (`2816-2820`). A publication failure authors
   `error` through `PushFailed` (`2840-2844`); publication is resumable
   (`src/parallel/merge.rs:1089-1097`), and `PushCompleted` supersedes `Error`
   with `pushed` (`642-643`, `2829-2833`).
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
8. **Enrichment and the run clock read the published rows.** The post-match
   block of `apply_dispatch` (`src/web/state.rs:1654-1667`) enriches the stored
   rows from `dispatch.state`, else a non-blocking read-back, when the event set
   `updated`; `apply_reducer_derived_queue_statuses` (`261-283`) sets
   `queue_status` (`None` for `not queued`), `archive_complete`, a positive
   `iteration_number`, and the operator fields. The projection re-enriches its
   clone at every projection (`868-880`). `contextual_run_clock_step`
   (`1704-1750`) evaluates the resolve-side rule over the published rows'
   reducer statuses (stored statuses when the reducer cannot be read) against
   `RESOLVE_SETTLEMENT_PROCESSING_STATUSES` (`428-436`), mirroring the TUI's
   `try_transition_to_select` over the TUI's rows.

## 2. Goals and non-goals

Goals: a change this owner published and the reducer reports `merged` or
`pushed` is published for the rest of the incarnation — including a change an
observation released while it waited or failed and the reducer completed later —
coherent across list/get/state/execution-status/stream; the in-flight
integration window cannot lose it; no ordering of events, observations, or
reducer contention can erase the last copy of a published row before the reducer
completes it; nothing about admission, aggregates' meaning, clocks, or timing
changes; the guarantee is discoverable and additive.

Non-goals: publishing absent waits or errors, historical archives, persistence or
TTL, per-row flags, new routes or commands, TUI or reducer changes, Server code.

## 3. Decisions

### D1. Published rows in the stored snapshot; released rows in a private memory

Kept and republished rows are ordinary `ChangeStatus` entries in
`WebState.state.changes`, so every existing consumer — event-arm updates,
reducer enrichment, `refresh_summary`, `operator_snapshot_with`,
`contextual_run_clock_step`, `project_snapshot`, the event ring — sees them with
no second code path.

A row the owner stops publishing moves into one new private `WebState` field:

```rust
/// Rows this owner incarnation published and no longer publishes, in release
/// order, at most one per change id, never an id `state.changes` holds.
///
/// Private observability memory: never projected, never persisted, never read
/// by admission. A row leaves it only when the reducer authors `merged`,
/// `pushed`, or `resolving` for it (published again) or when an observation
/// lists its id again (discarded). Locked only while the `state` write guard
/// is held, and never across an `.await`.
released_rows: std::sync::Mutex<Vec<ChangeStatus>>,
```

`WebState::new` initializes it with `std::sync::Mutex::new(Vec::new())`. Every
lock uses `.lock().unwrap_or_else(|poisoned| poisoned.into_inner())`, the idiom
`dispatch_revisions` already uses (`1693-1695`).

Why every released row and not a selected few: a selective capture (for example
"only rows the reducer reports archived or `resolving` when they leave") decides
at release time, with only the reducer state that observation holds or, under
contention, the row's stored status. Neither is guaranteed to have seen the
archive: events reach `WebState` after the reducer, a forwarder delivery without
dispatch state enriches nothing when its read-back is contended, and the merge
can land on the base while the stored row still says `archiving` or `resolve
pending`. Any selection rule can therefore discard the only copy before the
reducer's completion becomes observable; remembering every released row cannot.
Publication stays selective: only a reducer-authored `merged`, `pushed`, or
`resolving` brings a row back (D3).

Rejected alternatives and why:

- *Selective capture on release*: above.
- *Publishing absent waits or errors* (`merge wait`, `error`, …), or copying the
  TUI rule: changes what frontends see for waits and errors; excluded.
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

### D2. Statuses that keep an absent row published

```rust
/// Reducer-authored display statuses that keep an already published row for the
/// rest of this owner incarnation once a workspace observation stops listing it.
pub const RETAINED_COMPLETION_STATUSES: [&str; 2] = ["merged", "pushed"];

/// Reducer-authored display statuses that keep an already published row through
/// workspace absence only while they last: the owner is integrating the change
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
  exists, and a released change could not be shown while its explicit resolve
  or resumed publication runs. It is an active status, not a wait or an error.
- Every other status of an absent row releases it: `not queued` (whatever the
  task counts), `queued`, `blocked`, `stalled`, `preparing`, `applying`,
  `accepting`, `rejecting`, `archiving`, `merge wait`, `resolve pending`,
  `reject pending`, `error`, `rejected`, `stopped`.

### D3. Deciding status and the one reducer read per update

For a previously published row the observation does not list, the deciding
status is:

1. when the update holds a reducer read — `reducer.display_status(&row.id)`
   (`"not queued"` for an untracked ID);
2. otherwise (no reducer bound, or the non-blocking read failed) the row's own
   last reducer-derived status, `row.queue_status.as_deref().unwrap_or("not
   queued")`.

A released row is published again only under rule 1: an update without a reducer
read republishes nothing. No other source is consulted: not absence, task
counts, the archive directory, `archive_complete`, `observed_statuses`, the
private memory's stored status, or the TUI. Rule 2 mirrors the existing disk
rule for listed rows ("preserve `queue_status` only when shared reducer state
did not provide one", `src/web/state.rs:1157-1160`).

Each update takes exactly one reducer read and uses it for the retention step
and for that update's stored-row enrichment:

- event path: `dispatch.state` when present; otherwise, when the update needs
  one (D5), the existing non-blocking read-back of `1659-1664`
  (`self.shared_orchestrator_state.try_read()`, then `try_read()` of the
  reducer); otherwise none;
- disk path: `self.shared_orchestrator_state.read().await.clone()` before the
  `state` write guard, then `try_read()` of the reducer under it (D6).

Truthful consequences:

- A stored `merged`/`pushed` is final (fact 3), so rule 2 never keeps a row the
  reducer would release, and a readable reducer never downgrades a kept
  completed row.
- A stored `resolving` can be stale. Rows are released only by workspace
  observations, never by reducer events, so a kept `resolving` row that settles
  into another status stays published until the first observation that reads
  the reducer releases it; an observation under rule 2 keeps it while its stored
  status still says `resolving`. Meanwhile every projection that reads the
  reducer re-enriches it (`operator_snapshot_with`, fact 8), so it is published
  with its current reducer-authored status — for example `merge wait` or
  `error` — from the settling event until that release.
- A release is never destructive: the row moves into the private memory and is
  published again if the reducer later authors `merged`, `pushed`, or
  `resolving` for it.

### D4. Reconciliation helpers

```rust
/// What one workspace observation did to the published rows and the private
/// memory.
struct RetentionObservation {
    /// Published ids the observation does not list (kept, then published
    /// again), in published order.
    kept_ids: Vec<String>,
    /// Ids held in the private memory after the observation, in release order.
    private_ids: Vec<String>,
    /// Private ids the observation lists again; their memory was discarded.
    reappeared_ids: Vec<String>,
}

/// Reconcile one workspace observation with this incarnation's published rows.
///
/// `rows` holds the observation's rows, already built and preserved;
/// `previous` is the published list being replaced, moved out of the stored
/// snapshot; `released` is the private memory; `reducer` is the update's one
/// reducer read.
fn reconcile_observation(
    rows: &mut Vec<ChangeStatus>,
    previous: Vec<ChangeStatus>,
    released: &mut Vec<ChangeStatus>,
    reducer: Option<&crate::orchestration::state::OrchestratorState>,
) -> RetentionObservation

/// Move every private row whose reducer-authored display status keeps an absent
/// row out of `released`, in release order, each with `queue_status` set to
/// that status. The rows left behind keep their relative order.
fn take_republishable(
    released: &mut Vec<ChangeStatus>,
    reducer: &crate::orchestration::state::OrchestratorState,
) -> Vec<ChangeStatus>
```

`reconcile_observation`, exactly:

1. `listed` = the ids of `rows`.
2. Discard: remove every `released` entry whose id is in `listed`, keeping the
   others in order; the removed ids, in release order, are `reappeared_ids`.
3. For each row of `previous`, in order, whose id is not in `listed`: if its
   deciding status (D3) satisfies `keeps_absent_row`, set `queue_status =
   Some(deciding status)` and append it to `carried`; otherwise push it onto the
   end of `released`.
4. `rows.extend(carried)`; then, when `reducer` is `Some`,
   `rows.extend(take_republishable(released, reducer))`.
5. Return `kept_ids` (the ids appended in step 4, in order), `private_ids` (the
   ids of `released`, in order), and `reappeared_ids`.

Rows newly released in step 3 are also examined by step 4, under the same read,
and therefore stay private. The event path republishes outside observations
with `take_republishable` alone (D5).

Invariants after every update, observable through the published rows and the
facts (tests 4-10): (I1) at most one private entry per id; (I2) no id is both
published and private; (I3) the private memory is in release order; (I4) every
id this `WebState` ever published is either published or private until an
observation lists it again.

Cloning: rows are moved, never cloned — `previous` leaves the stored snapshot
through `std::mem::replace` (event path) or `std::mem::take` (disk path), and
carried, released, and republished rows move between the three vectors. Only id
strings are cloned, for `listed` and the three id lists.

Order rule: the observation's rows in observation order, then carried rows in
their previously published relative order, then rows published again by this
observation in release order. A reducer event that publishes rows again appends
them in release order (D5), and the next observation carries them in that
published order. With nothing kept, the result is exactly today's list, so the
order of every non-retaining test is unchanged; identical repeated observations
produce identical lists.

### D5. Event path (`apply_dispatch`)

- Next to the existing `updated` flag declare `let mut observation_previous:
  Option<Vec<ChangeStatus>> = None;`, and before the `state` write scope declare
  `let mut retention: Option<RetentionObservation> = None;` so
  `absorb_operator_facts` can read it after the scope.
- In the `ChangesRefreshed` arm keep the per-row preservation loop unchanged (it
  still looks up `state.changes` before the replacement, so a kept row listed
  again keeps `iteration_number` and non-zero progress, and a private row listed
  again is built fresh exactly as today) and replace `state.changes =
  new_change_statuses;` with `observation_previous =
  Some(std::mem::replace(&mut state.changes, new_change_statuses));`. Keep
  `refresh_summary` and `updated = true`.
- After the `match`, before `if updated`, still under the `state` write guard:
  lock `released_rows`; take the update's one reducer read (D3) — the read-back
  is taken only when `observation_previous.is_some() || updated ||
  !released.is_empty()`; then
  - with `Some(previous)` in `observation_previous`: `retention =
    Some(reconcile_observation(&mut state.changes, previous, &mut released,
    reducer))`;
  - otherwise, with a reducer read: `let republished =
    take_republishable(&mut released, reducer);` and, when it is non-empty,
    `state.changes.extend(republished)` and `updated = true`;
  - unlock `released_rows`.
- The existing `if updated` block enriches from that same read (replacing its
  own read-back) and calls `refresh_summary`. The read-back guards are dropped
  before the `state` scope ends. Nothing else in the arm or the block changes.
- `absorb_operator_facts` gains `retention: Option<&RetentionObservation>`
  (`None` for every other event). Its `ChangesRefreshed` branch observes
  `observed = changes ∪ rejected_changes ∪ kept_ids ∪ private_ids` through the
  D7 rule; `apply_parallel_eligibility` and `apply_worktree_paths` stay as they
  are. Nothing else in it changes. A row published again by a non-observation
  event needs no facts call: its id was already observed as private.

### D6. Disk path (`merge_workspace_observation`), one write guard

The function becomes one read-modify-write under a single `state` write guard,
which closes the window of fact 1 for published and private rows alike:

1. `let shared = self.shared_orchestrator_state.read().await.clone();` — the
   late-bound `Option<Arc<RwLock<OrchestratorState>>>`; the guard is released at
   the end of the statement.
2. `let mut state = self.state.write().await;`
3. `let reducer_guard = shared.as_ref().and_then(|shared| shared.try_read().ok());`
   — non-blocking; `reducer = reducer_guard.as_deref()`.
4. Build `new_state` exactly as today — `from_changes_with_shared_state(changes,
   reducer)`, the `is_resolving`, `app_mode`, and `persistent_scheduler_idle`
   preservation, and the per-row preservation loop — reading the previous
   values from `state` instead of from a clone.
5. Lock `released_rows`; `let retention = reconcile_observation(&mut
   new_state.changes, std::mem::take(&mut state.changes), &mut released,
   reducer);` unlock.
6. `refresh_summary(&mut new_state)`; `drop(reducer_guard)`; `*state =
   new_state`; drop `state`.
7. `observe_changes_for_attention` takes the observed ids and the reappeared ids
   instead of `&[Change]`: the listing's ids ∪ `kept_ids` ∪ `private_ids`, and
   `reappeared_ids`, under the D7 rule.
8. `self.sync_remote_control_projection().await` as today.

Lock order: the late-bound reducer slot (released in step 1) → `state` write →
reducer `try_read` → `released_rows`. The event path (D5) locks `released_rows`
before its read-back; that is safe because both acquisitions happen only under
the `state` write guard and neither waits — `try_read` fails instead of
blocking, and only the holder of the `state` write guard ever locks
`released_rows`. No `.await` runs while a reducer guard or `released_rows` is
held. `refresh_from_disk_at`, `update`, and
`seed_workspace_observation_for_tests` inherit the behavior unchanged.

### D7. Facts

At every observation the facts store observes the published ids plus the private
ids, so `timing`, `latest_activity`, attention, and known-ness survive release,
and every event applied while a row is private is recorded on its entry exactly
as for a published row. A private entry is unreadable through any resource
(fact 2): `facts(id)` and `net_active(id)` are read only for projected rows;
`worktree_owner` and `worktree_path` read the observation's `worktree_paths`;
`reproject_worktrees` and `apply_parallel_eligibility` write private entries
that nothing projects. `OperatorFactsStore` itself is not modified.

The observation rule, used by `absorb_operator_facts` and
`observe_changes_for_attention`: when `reappeared_ids` is non-empty, call
`observe_changes(observed − reappeared_ids)` and then `observe_changes(observed)`
under the same facts write guard; otherwise call `observe_changes(observed)`
once. The first call removes the reappeared entries, the second re-adds them as
new arrivals with empty facts — exactly what the store gives a change that was
never kept ("a re-created change is genuinely new again",
`operator_facts.rs:255-260`). The memory therefore never changes how an
unpublished change looks when it returns.

`net_active` and `observed_statuses` were never pruned. Because
`observe_display_statuses_at` is fed only projected rows, a private row's
net-active clock and observed status do not move while it is private and resume
from the first projection that publishes it again. In the recovery flows of fact
4 the private interval spans only `merge wait` or `error`, neither of which is
active, and the row is published again by the event that makes it `resolving`
or `pushed`, so its net-active time equals the TUI's (tests 15, 17, 18).

Workspace-observation facts keep their existing rules: `worktree` is `null` when
the latest observation reports no worktree path for the id (the disk path
computes paths only for listed ids; an event carries its own `worktree_paths`),
and `parallel.blocked_reason` is `not_committed` when the latest event
observation's committed set lacks the id (the disk path does not update
eligibility). These are current observations, not guesses, and a final status
blocks every action regardless (`classify_actions`).

### D8. Enrichment, reappearance, summary, timing

- Reducer enrichment (`apply_reducer_derived_queue_statuses` on the stored
  snapshot, and on the clone in `operator_snapshot_with`) runs over kept and
  republished rows like any other row: `queue_status`, `archive_complete`,
  `queue_intent`, `blocker`, `error_detail`, `acceptance_provenance`;
  `iteration_number` is only overwritten by a positive apply count, which
  `mark_archived` removed, so the last known value stays.
- A kept or republished row carries the deciding status in `queue_status`
  before enrichment, so a projection that cannot read the reducer still
  publishes a reducer-derived status.
- Reappearance of a published row: an observation that lists a kept ID builds
  that row from the observation; the preservation loop finds the kept row as the
  previous row and keeps `iteration_number` and non-zero progress over a 0/0
  listing; `reconcile_observation` skips the ID. One row, the observation's
  listing fields, the reducer's status, no new-arrival attention, `timing` and
  `net_active` kept — what the TUI shows. Because the reducer keeps one runtime
  entry per ID and `Merged` is immutable for the incarnation, a re-created
  directory for a merged ID shows, for example, `merged` 1/3. Nothing starts a
  new lifecycle for a reused ID: no reducer reset, execution episode, mark, or
  queue intent is created.
- Reappearance of a private ID: its memory is discarded (D4 step 2) and the row
  is the observation's row exactly as today — no previous row for the
  preservation loop, fresh facts, new-arrival attention (D7), the reducer's
  status.
- `refresh_summary` runs after retention in both paths; an event that publishes
  rows again sets `updated`, so the existing block recomputes totals.
- `ChangeTiming` boundaries change only on the existing run start/end events;
  retention adds none.

### D9. Coherence

Retention happens before the existing single projection transaction
(`project_execution_event` → `apply_state` for a dispatch,
`sync_remote_control_projection` → `apply_state_if_changed` for the disk path),
so a kept row appears in the same `state_revision` as its observation, a row
published again appears in the revision of the event or observation that
published it, and an identical repeated observation produces an equal candidate
and no revision. `changes_refreshed` payloads keep counting the observation
(`projection.rs:1289-1297`); the snapshot at the event's revision is the
authority for rows. `GET /api/v2/execution-status` lists the same rows because
it iterates `snapshot.changes`.

### D10. Non-authority

Retention changes the row set only. Admission (fact 7) reads none of it, and the
private memory is read by nothing but the two reconciliation helpers: not by
admission, `classify_actions`, `refresh_summary`, `contextual_run_clock_step`,
`operator_snapshot_with`, text edit, or the refresh preview. `classify_actions`
blocks every action of a `merged`/`pushed` row (`FinalStatus` and the shared
force-stop/refresh refusals). Totals keep their shared classifiers: kept
`merged`/`pushed` rows count as `completed`, a kept `resolving` row counts as
`in_progress` exactly as when listed, `pending` counts only `queued`, and private
rows count nowhere.

The run clock's resolve-side rule (`contextual_run_clock_step`) reads the
published rows: a kept `resolving` row is processing there, which is what the
TUI's `try_transition_to_select` sees because the TUI keeps that row. When
explicit resolve starts with the resolve lane free, API and TUI therefore
evaluate the same rows during an absent resolve and at its settlement. Private
rows are not read by the rule, exactly as before this change when such a row
was gone. A private `resolve pending` row waiting while another change occupies
the lane is a resolve-settlement processing status in the TUI but not in the
API's published rows: the API run clock can freeze earlier than the TUI's in
that interval (as on the existing API). No unconditional parity is claimed for
private processing rows. `net_active` observes `merged`/`pushed` as a closed status,
so nothing reopens or resets. Retention introduces no mark revocation or
creation: `execution_marked` on a published row is the shared store's value (an
archived change keeps its mark by existing spec), and Start/mark admission
refuse terminal targets as before.

### D11. Lifetime and memory scope

Published, kept, and private rows live in the `WebState` of one process. A new
process starts with `WebState::new(initial)` — an empty private memory — and a
fresh reducer and projection (new `instance_id`), observes only
`openspec/changes/*`, scans no archive, and therefore publishes no row for a
change an earlier incarnation merged; a reducer completion for an ID this
`WebState` never published publishes nothing. Nothing is written anywhere. The
private memory holds at most one `ChangeStatus` per change ID this incarnation
published and stopped publishing, and the facts store keeps one entry per such
ID: bounded by the IDs the incarnation observed, the same bound as the reducer's
never-pruned `change_runtime` map, with no TTL or eviction other than discard on
reappearance.

### D12. Capability block

`src/web/remote_control_api/dto.rs`:

```rust
/// Whether changes this owner published stay in `changes` once the reducer
/// reports them complete, even after the workspace stops listing them, and for
/// how long.
///
/// `Default` is the *unavailable* answer, which is what an older owner's
/// omission decodes as: such an owner may drop a merged row as soon as its
/// proposal leaves `openspec/changes/`. Hiding or restoring rows is client-local
/// and needs no command; key that state by `instance_id`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct CompletedChangeRetentionCapability {
    /// True when this owner publishes, for the rest of its incarnation, every
    /// change it already published while the reducer reports it `merged` or
    /// `pushed` — including one it stopped publishing while it waited or failed.
    pub available: bool,
    /// Reducer-authored statuses that keep such a change published for the rest
    /// of the incarnation whether or not the workspace lists it: exactly `merged`
    /// and `pushed`.
    pub retained_statuses: Vec<String>,
    /// Reducer-authored statuses that keep such a change published while the
    /// workspace does not list it only for as long as the status lasts: exactly
    /// `resolving`.
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
The private memory adds no member, schema, route, or command.

Descriptions (they are the OpenAPI property descriptions):

- `InstanceSnapshot.changes` and `ChangesResponse.changes`: "Projected changes:
  every change the current workspace observation lists, followed by every change
  this owner incarnation already published that the observation no longer lists
  while the reducer reports it `merged` or `pushed` (published for the rest of
  the incarnation, also after an unpublished wait or failure) or `resolving`
  (published while it lasts). See
  `CapabilitiesResponse.completed_change_retention`."
- `SnapshotTotals`: "Aggregate counts over the projected changes, rows kept by
  `completed_change_retention` included; changes the owner no longer publishes
  are not counted."

### D13. Observable consequences on existing surfaces

Owner-side facts, asserted by the tests of section 5:

- A kept `merged`/`pushed` row: `GET /api/v2/changes/{id}` answers 200 instead
  of 404; `GET /api/v2/execution-status` lists it;
  `GET /api/v2/proposals/{id}/refresh-preview` answers 200 with `eligible:
  false`, `refusal: terminal_target` instead of 404; the change text route still
  answers 404 `not_found`; `totals.total`/`totals.completed` include it. Its
  `worktree` is `null` exactly when the latest observation reports no worktree
  path for its ID, and its `parallel.blocked_reason` is `not_committed` exactly
  when the latest event observation's committed set lacks its ID (D7).
- A kept `resolving` row that settled: published with its current reducer status
  (`merge wait`, `error`, …) until the next reducer-reading observation releases
  it (D3).
- A private row: everything exactly as today for an absent change — not listed,
  404 on get, refresh preview, and text, absent from totals, execution status,
  and the run-clock rows; events naming it stream as today. Published again, it
  carries the facts this owner recorded for it (D7).

Client consequences derived from those owner facts. They are not validated by
this change, whose scope adds no `cflx client` code or test:

- `cflx client mark|unmark <id>` on a kept row submits and settles `unchanged`
  with the non-markable-target reason instead of a local `change_not_found`;
  `force-stop-change` and `refresh-proposal` return `target_ineligible` instead
  of `change_not_found`.
- `cflx client wait <id>` certifies a kept `merged`/`pushed` row against the
  repository as before; when certification fails the outcome is the existing
  `change_requires_action` for a settled row rather than `change_not_found`.
- While a settled row stays published as `merge wait` or `error` until the next
  reducer-reading observation, a concurrent `wait` observes that status and
  returns `change_requires_action` (exit 27); before this change the row's
  disappearance kept such a wait observing.
- For a private ID, `wait` behaves as for an absent change today until the row
  is published again.

No member, schema, or meaning changes. These consequences are documented, not
coded.

### D14. Failure behavior

- A refresh that fails before producing an observation (`list_changes_native_from`
  error, e.g. `openspec/changes` is a file) returns `Err` before taking the
  `state` guard: no row, private entry, or revision changes. The TUI refresh
  task reports a failure as a log and dispatches no `ChangesRefreshed`.
- An observation that lists nothing (`openspec/changes` absent) releases every
  row that is not kept into the private memory and keeps the kept ones.
- No reducer readable: D3 rule 2 for kept rows, and nothing is published again.
  No reducer bound at all: stored statuses were never reducer-derived, so
  nothing is kept and nothing private is ever published again.
- An untracked ID under a readable reducer is `not queued` and is released.

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

Every new test name starts with `retain_api_completed_`; there are exactly 24
(owner gate 21, contract gate 2, Git gate 1, section 7). No new test carries
`#[ignore]` or `cfg_attr(..., ignore)`. The owner and contract cases are bounded:
each finishes in well under one second, with no sleeps and no wall-clock
assertion. The Git case lives in `tests/e2e_git_worktree_tests.rs`, whose
existing file-level `#![cfg(feature = "heavy-tests")]` is the only gate; the
`feature = "heavy"` spelling in `AGENTS.md` names no Cargo feature and must not
be used. Instants are injected with the existing
`pin_observation_clock_for_tests` / progress-timing `Clock` seams.

Shared sequences: **merge(id)** = `ApplyStarted`, `ApplyCompleted{revision}`,
`ArchiveStarted`, `ChangeArchived`, `ResolveStarted`, `MergeCompleted{revision}`;
**push(id)** = the same up to `ChangeArchived`, then `PushStarted{remote,
branch}`, `PushCompleted{remote, branch}`; **hookfail(id)** = merge(id) up to
`ResolveStarted`, then `HookFailed{hook_type: "on_merged"}` (reducer `merge
wait`); **resolve(id)** = `ReducerCommand::ResolveMerge(id)` applied to the
reducer through its write guard (as `run_control.rs:1131` does), then
`ResolveStarted`, `MergeCompleted{revision}`; **pushfail(id)** = push(id) up to
`PushStarted`, then `PushFailed` (reducer `error`); **resume_push(id)** =
`PushStarted`, `PushCompleted` — the resumed publication. Each change completes
or fails before the next is archived, so the resolve lane is free. "Published"
means present in both the stored rows and the projected
`snapshot.changes`; "private" means absent from both while the test later
proves the memory with a completion that publishes it again.

### 5.1 `src/web/state.rs` (`mod tests`, unit)

Helpers: a `WebState::new(&[])` bound to an `OrchestratorState` reducer, a
`deliver` that applies an event to the reducer and then to
`apply_execution_event`, a `command` that applies a `ReducerCommand` through the
reducer write guard, and a `ChangesRefreshed` builder whose
`committed_change_ids` are the listed IDs.

1. `retain_api_completed_statuses_are_the_reducer_authored_completion_vocabulary`
   — the two constants equal `["merged","pushed"]` and `["resolving"]`; each
   retained status satisfies `is_completed_status`, `resolving` satisfies
   `is_active_status`, `archived` is completed but not retained; a reducer
   driven through merge(id)/push(id) authors `merged`/`pushed`, through
   `ChangeArchived` alone `resolving`, through hookfail(id) `merge wait`, and
   through pushfail(id) `error`;
   `completed_change_retention_capability()` equals `{available: true,
   retained_statuses: [merged, pushed], settling_statuses: [resolving], scope:
   instance}`.
2. `retain_api_completed_refresh_event_keeps_merged_and_pushed_rows` — rows
   alpha 2/2, beta 1/1, gamma 0/3; merge(alpha), push(beta); three
   `ChangesRefreshed{[gamma]}`: published rows are `[gamma, alpha, beta]`, alpha
   `merged` and beta `pushed` with their task counts, progress,
   `archive_complete: true`, and the `iteration_number` captured before
   absence; the projection revision after the third refresh equals the one
   after the first.
3. `retain_api_completed_disk_observation_keeps_merged_and_pushed_rows` — the
   same through three `update(&[gamma])` calls (the `merge_workspace_observation`
   path).
4. `retain_api_completed_never_retains_without_reducer_completion` — absent
   rows `done` (3/3, `not queued`), `waiting` (hookfail → `merge wait`),
   `pending` (archived while a still-listed row `lane` is `resolving` → `resolve
   pending`), `failed` (`ProcessingError`), `rejected` (`ChangeRejected`),
   `stopped` (`ReducerCommand::StopChange`), and `orphan` on a second `WebState`
   with no reducer bound (its stored `queue_status` is `None`): after one event
   refresh and one `update` that list only `lane`, none of them is published and
   `lane` is; two more refreshes of each kind and a repeated
   `ProcessingError{failed}` keep all of them private; `totals.total` counts only
   published rows.
5. `retain_api_completed_settled_row_is_released_then_republished_on_completion`
   — alpha: merge(alpha) up to `ResolveStarted`, one event refresh without alpha
   keeps it `resolving`, counted `in_progress`; `MergeCompleted` → `merged`,
   kept over one event refresh and one `update`. delta: hookfail(delta) with
   one event refresh without delta after `ResolveStarted` (kept `resolving`);
   after `HookFailed` delta is still published and stored and projected as
   `merge wait`; the next event refresh without delta makes it private and one
   `update` without delta keeps it private; resolve(delta): the snapshot
   produced by the `ResolveStarted` delivery publishes delta `resolving` after
   alpha, `MergeCompleted` makes it `merged`, and one event refresh plus one
   `update` keep `[alpha, delta]` with delta's last observed 2/2 and
   `archive_complete: true`.
6. `retain_api_completed_reappearance_updates_one_row_in_place` — alpha merged
   and kept with 2/2; a refresh listing alpha 1/3 again yields exactly one
   alpha, 1/3, `merged`, attention `none`, in observation order; a refresh
   listing alpha 0/0 keeps 1/3; a refresh without alpha keeps it with 1/3.
   epsilon: hookfail(epsilon) and one refresh without it make it private; a
   refresh listing epsilon 1/1 again yields exactly one epsilon with the
   observation's 1/1, the reducer's `merge wait`, attention `new`, and the
   `timing` and `latest_activity` of an ID this `WebState` never observed; a
   later resolve(epsilon) leaves exactly one epsilon, `merged`.
7. `retain_api_completed_order_is_observation_then_retained_then_republished` —
   rows a, b, c, d, e; merge(b), hookfail(d), hookfail(e); refresh `[a, c]` →
   `[a, c, b]`; resolve(e) → `[a, c, b, e]`; resolve(d) → `[a, c, b, e, d]`;
   three refreshes `[a, c]` → the same order and no revision change after the
   first; refresh `[a, c, f]` → `[a, c, f, b, e, d]`.
8. `retain_api_completed_contended_reducer_keeps_the_last_reducer_status` —
   alpha merged and kept (stored `merged`), gamma listed `not queued`, delta kept
   `resolving` (archived, `ResolveStarted`, one refresh without delta). Holding
   the reducer write guard, the test applies `HookFailed{delta, on_merged}`
   through the guard and calls `update(&[beta])` and
   `apply_execution_event(ChangesRefreshed{[beta]})`: alpha is kept `merged`,
   gamma is private, delta is kept `resolving` (stored fallback). After the
   guard is dropped, `sync_remote_control_projection` publishes delta `merge
   wait` (the transient of D3), and the next `update(&[beta])` makes delta
   private. Holding the guard again, the test applies
   `ReducerCommand::ResolveMerge(delta)`, `ResolveStarted`, and
   `MergeCompleted` to the reducer only and calls `update(&[beta])` and
   `apply_execution_event(MergeCompleted{delta})`: delta stays private. After
   the guard is dropped, one more `update(&[beta])` publishes delta `merged`
   after alpha.
9. `retain_api_completed_failed_or_empty_refresh_keeps_completed_rows` — alpha
   merged and kept, gamma listed, delta private after hookfail(delta) and one
   refresh without it; `refresh_from_disk_at` on a `tempfile::tempdir()` root
   whose `openspec/changes` is a regular file returns `Err` and leaves the
   stored snapshot, the projected snapshot, and the revision unchanged; on a
   temp root without `openspec/changes` it returns `Ok`, alpha is kept and gamma
   becomes private; resolve(delta) afterwards publishes delta `merged`.
10. `retain_api_completed_facts_survive_absence_and_release` — with pinned
    instants, `ProcessingStarted` and merge(alpha) produce `timing` with both
    boundaries, `latest_activity.event_type == "merge_completed"`, and closed
    `net_active` with `accumulated_ms > 0`; after three event refreshes and two
    `update`s without alpha, the projected `timing`, `latest_activity`,
    `net_active`, and `attention` equal the values captured before absence,
    `worktree` is `null`, and `parallel.blocked_reason` is `not_committed`. A
    control `WebState` bound to the same reducer and pinned clock receives
    every delivery at the same instant, but its observations keep listing delta
    and epsilon (after the shared initial observation, each refresh is applied to
    the two `WebState`s only, each with its own listing; the reducer never prunes
    on `ChangesRefreshed`, fact 3); the tested `WebState` runs hookfail(delta) then resolve(delta),
    and pushfail(epsilon) then resume_push(epsilon), with one event refresh and
    one `update` that omit both between each failure and its recovery. After
    completion the tested projection's delta and epsilon equal the control's in
    `display_status`, `completed_tasks`, `total_tasks`, `progress_percent`,
    `archive_complete`, `iteration_number`, `timing`, `latest_activity`,
    `net_active`, `attention`, and `worktree`.

### 5.2 `src/web/remote_control_api/tests/operator_snapshot_tests.rs` (integration)

Helper `retention_owner(ids)`: `Arc<WebState>` bound to a reducer, an
`ExecutionMarkStore`, and repo root `/repo`; an `EventDispatcher::new(reducer,
vec![Arc::new(WebEventSink::new(web))])`; the production router
`crate::web::remote_control_router(&WebConfig::enabled(0, "127.0.0.1".into()),
web)`; an initial dispatched `ChangesRefreshed` listing `ids`. Requests use the
existing `get`/`send`/`status_and_json` helpers; resolve's `ResolveMerge` is
applied through the dispatcher's reducer guard before its events are dispatched.

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
    `changes_refreshed` at `R`, and alpha is still published. Republication:
    dispatch pushfail(beta) and one refresh without beta (`/api/v2/changes/beta`
    404), record cursor `c1`, dispatch resume_push(beta); beta is first published
    by the first of those two dispatches after which the reducer reports beta
    `resolving` or `pushed`, and at that dispatch's produced revision and at the
    final one `/api/v2/state`, `/api/v2/changes`, and `/api/v2/changes/beta`
    (200) agree; `events_after(c1)` and the SSE replay from `c1` list the same
    `push_started` and `push_completed` events, each at the revision whose
    snapshot publishes beta exactly when the reducer status after it keeps beta.
12. `retain_api_completed_reads_and_private_memory_have_no_side_effects` — with
    alpha kept and delta private (hookfail(delta), one refresh without delta),
    capture revision, event sequence, `registry_sizes()`, the reducer's
    `all_display_statuses()`, and `marked_ids()`; read twice each of
    `/api/v2/state`, `/api/v2/changes`, `/api/v2/changes/alpha`,
    `/api/v2/changes/delta`, `/api/v2/capabilities`, `/api/v2/execution-status`,
    `/api/v2/proposals/alpha/refresh-preview?source_ref=main`,
    `/api/v2/proposals/delta/refresh-preview?source_ref=main`,
    `/api/v2/changes/alpha/text?path=proposal.md`,
    `/api/v2/changes/delta/text?path=proposal.md`, and the SSE replay; the
    captured values are unchanged; the alpha preview is 200 with `eligible:
    false`, `refusal: terminal_target`; the alpha text read is 404 `not_found`;
    the delta get, preview, and text reads answer the same status and error code
    as the same reads for an ID this owner never observed; no `changes` list,
    execution-status entry, or total names or counts delta; the
    capabilities block equals `completed_change_retention_capability()`. A
    dispatched `ChangesRefreshed` whose `worktree_paths` names delta without
    listing it leaves every published row's `worktree` equal to what that
    observation reports for the row's own ID and publishes no delta. resolve(delta)
    then publishes delta `merged`: the reads left the memory intact.
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
    keeps alpha; owner B, built fresh over a listing without alpha, publishes no
    alpha, publishes beta, has a different `instance_id`, and reports `scope:
    instance`; merge(alpha) dispatched through owner B's own dispatcher still
    publishes no alpha in B (B never published it, so B holds no memory of it);
    owner A still publishes alpha.

### 5.3 `src/web/remote_control_api/tests/progress_timing_tests.rs` (integration, TUI parity)

Reuse `Fixture`, `Clock`, `start`, `web_event`, `sync_tui`, and
`assert_net_active_parity`. Add `refresh_both(seconds, listed)`: apply one
`ChangesRefreshed` to the reducer and the web store at `seconds`, then
`sync_tui(seconds)`, then `tui.handle_orchestrator_event(event)` — the order the
runner delivers it. "Run-clock parity" is checked with the existing
`Fixture::assert_run_clock_parity(started, frozen)` helper: it compares the API
`run_clock` (`started_at`, `ended_at`, `elapsed_ms`) to the TUI's
`orchestration_started_at` and `orchestration_elapsed`, both advanced from the
same injected event instants. The TUI does not expose a `RunClockView` or use
the Web `RunClock` struct. Follow the event delivery ordering in
`remote_progress_timing_run_clock_resolve_settlement_matches_tui` for settlement
steps. Only assert parity for the free-lane resolve and publication cases in
this test plan, not for private `resolve pending` while another change owns the
lane.

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
    `timing` equals the value captured at completion; run-clock parity holds at
    the absent-resolving refreshes 6 and 11, at the settlements 7 and 12, and at
    13, 14, and 15.
16. `retain_api_completed_tui_dismissal_leaves_the_api_row` — same arrangement
    through 13; capture the API snapshot and revision; focus alpha
    (`cursor_index` = alpha's TUI index) and `dismiss_merged_row()`; the TUI
    rows lack alpha, the API snapshot and revision are unchanged;
    `refresh_both(14, [gamma])` keeps alpha hidden in the TUI and published,
    unchanged, in the API; `dismiss_all_merged_rows()` leaves the API unchanged.
17. `retain_api_completed_tui_and_api_agree_after_on_merged_hook_recovery` —
    fixture alpha 2/2, gamma; start at 0; alpha `ApplyStarted` 1,
    `ApplyCompleted` 2, `ArchiveStarted` 3, `ChangeArchived` 4, `ResolveStarted`
    5; `refresh_both(6, [gamma])` keeps alpha `resolving` on both; `HookFailed{
    on_merged}` at 7 → `merge wait` on both; `refresh_both(8, [gamma])` and
    `update(&[gamma])` at 8: the TUI keeps alpha `merge wait`, the API row set is
    the TUI's minus alpha and the API totals exclude it; at 9 the test applies
    `ReducerCommand::ResolveMerge(alpha)` and delivers `ResolveStarted`: the API
    publishes alpha `resolving` in the snapshot that delivery produced;
    `MergeCompleted` at 10; `refresh_both` and `update(&[gamma])` at 11 and 12.
    From 9 on the TUI and API row-ID sets are equal and alpha's display status,
    archive completion, and task counts (2/2) agree; at 10, 11, and 12 alpha is
    `merged` on both and `assert_net_active_parity` holds; run-clock parity
    holds at every step from 6 through 12.
18. `retain_api_completed_tui_and_api_agree_after_push_failure_recovery` —
    fixture beta 1/1, gamma; start at 0; beta `ApplyStarted` 1,
    `ApplyCompleted` 2, `ArchiveStarted` 3, `ChangeArchived` 4, `PushStarted` 5;
    `refresh_both(6, [gamma])` keeps beta `resolving` on both; `PushFailed` at 7
    → `error` on both; `refresh_both(8, [gamma])` and `update(&[gamma])` at 8:
    the TUI keeps beta `error`, the API row set is the TUI's minus beta and the
    API totals exclude it; resumed publication: `PushStarted` at 9 leaves the
    reducer's Error terminal unchanged and beta private; `PushCompleted` at 10
    authors `pushed`, so the API first republishes beta in that dispatch's
    snapshot. Then `refresh_both` and `update(&[gamma])` at 11 and 12. From
    that first API publication onward, the TUI and API row-ID sets
    are equal and beta's display status, archive completion, and task counts
    (1/1) agree; at 10, 11, and 12 beta is `pushed` on both and
    `assert_net_active_parity` holds; run-clock parity holds at every step from
    6 through 12.

### 5.4 `src/web/remote_control_api/tests/parallel_control_tests.rs` (integration, admission)

19. `retain_api_completed_retained_and_private_rows_admit_like_listed_rows` —
    `alpha_fixture(placement)` builds `Wired::new(&["alpha","beta"])`, observes
    both in `running`, sets alpha's mark, applies merge(alpha) (placements
    `ListedMerged`, `KeptMerged`) or hookfail(alpha) (placements
    `ListedMergeWait`, `PrivateMergeWait`) to the reducer and the web store, and
    for `KeptMerged` and `PrivateMergeWait` observes `[beta]` only, so alpha is
    kept `merged` or private. For each of `SetExecutionMark{alpha, true}`,
    `SetExecutionMark{alpha, false}`, `SetAllExecutionMarks{}`,
    `SetQueueIntent{alpha, true}`, `RetryChange{alpha}`,
    `RetryErrors{[alpha]}`, `StopAndDequeue{alpha}`, `ForceStopChange{alpha}`,
    `ResolveMerge{alpha}`, and `Start`, fresh fixtures of each pair
    (`ListedMerged`/`KeptMerged`, `ListedMergeWait`/`PrivateMergeWait`) execute
    it through `SharedServiceExecutor`; the settlement (`changed` flag or error
    code), `marked_ids()`, scheduler calls, both reducer statuses, and alpha's
    queue intent are equal within each pair; in `KeptMerged` alpha stays
    `merged` and marked, no scheduler call targets it, and its projected
    `actions` equal those of `ListedMerged`.

### 5.5 `src/web/remote_control_api/tests/dto_tests.rs` (compatibility fixture)

20. `retain_api_completed_older_owner_capabilities_decode_as_unavailable` — in
    `legacy_owner_shape`, `legacy_capabilities()` decodes with
    `completed_change_retention == CompletedChangeRetentionCapability::default()`;
    the same payload without `limits` is still rejected naming `limits`.
21. `retain_api_completed_current_capabilities_publish_the_block` — `GET
    /api/v2/capabilities` on the test harness router serializes
    `completed_change_retention` equal to
    `completed_change_retention_capability()` and round-trips through
    `CapabilitiesResponse`; `SUPPORTED_COMMANDS` still has 23 entries and none
    contains `dismiss`, `hide`, `restore`, or `retain`.

### 5.6 `tests/openapi_contract_tests.rs` (integration)

22. `retain_api_completed_capability_is_published_and_optional` —
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
23. `retain_api_completed_omitting_the_capability_fails_the_contract` — a
    document with the member removed, and one with it added to `required`, each
    make `retention_contract_errors` name `completed_change_retention`.

### 5.7 `tests/e2e_git_worktree_tests.rs` (integration, real Git, `heavy-tests` file)

24. `retain_api_completed_git_merge_survives_production_refreshes` — uses
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

    Evidence boundary: the case exercises the production listing functions,
    `refresh_from_disk_at`, and the production dispatcher → `WebEventSink` →
    `WebState` → projection → router. It rebuilds the `ChangesRefreshed` payload
    from the same two listing reads instead of running the TUI refresh task
    (`src/tui/runner.rs:915-1110`, unchanged by this change), so how that task
    fills the other payload members (worktree paths, committed and uncommitted
    IDs) is outside this gate; the unit and integration cases cover those
    members with explicit payloads.

## 6. Expected edits to existing tests

None are expected; the only existing-file edit outside new cases is the
`completed_change_retention: Default::default()` field in
`src/client/session.rs::observation_for_test`. Candidates were checked at the
base: `test_update_drops_an_archived_change` (no reducer, so nothing is kept and
the private row is never published again), the client wait disappearance tests
(synthetic snapshots written to the projection directly), and the
progress-timing, mark-event, and cross-adapter suites (rows stay listed). A
change listed again after release still becomes a new arrival with fresh facts
(D7), so attention tests keep their meaning. Two kinds of edit are permitted,
each listed in `tasks.md` Notes with its reason: an existing test that fails only
because a kept row is now published may assert that row; an existing test that
inspects `OperatorFactsStore` entries through `WebState` for a change that
disappeared may assert that the entry is unprojected and is reset when the
change is listed again. No other existing assertion may change.

## 7. Gates and execution environment (host asagi)

Declared gates (one focused command each, distinct selections):

| Verification ID | Command (leading assignments apply to the child only) | Expected |
| --- | --- | --- |
| `retain-api-completed-owner` | `TMPDIR=/home/tumf/tmp/work/rt-retain TMP=/home/tumf/tmp/work/rt-retain TEMP=/home/tumf/tmp/work/rt-retain CARGO_TARGET_DIR=/home/tumf/tmp/work/retain-api-completed-target /home/tumf/.cargo/bin/cargo test --lib --features web-monitoring retain_api_completed -- --nocapture` | 21 passed, 0 failed, 0 ignored |
| `retain-api-completed-contract` | same prefix, `/home/tumf/.cargo/bin/cargo test --features web-monitoring --test openapi_contract_tests retain_api_completed -- --nocapture` | 2 passed, 0 failed, 0 ignored |
| `retain-api-completed-git` | same prefix, `/home/tumf/.cargo/bin/cargo test --features web-monitoring,heavy-tests --test e2e_git_worktree_tests retain_api_completed -- --nocapture` | 1 passed, 0 failed, 0 ignored |

Expected names, each exactly once with result `ok`:

- `retain-api-completed-owner` (21): tests 1-10 in `src/web/state.rs`, 11-14 in
  `operator_snapshot_tests.rs`, 15-18 in `progress_timing_tests.rs`, 19 in
  `parallel_control_tests.rs`, and 20-21 in `dto_tests.rs`, by the names in
  section 5.
- `retain-api-completed-contract` (2): tests 22 and 23.
- `retain-api-completed-git` (1): test 24.

A gate passes only when the command exits 0, its own summary line reports
exactly the expected passed count with `0 failed` and `0 ignored`, and every
expected name appears once as `... ok`. Any other count, a missing, extra, or
renamed `retain_api_completed_` case, an ignored case, a zero-test selection, or
a partial subset is a failed gate, read from the command's own output and exit
status, never from a wrapper.

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
