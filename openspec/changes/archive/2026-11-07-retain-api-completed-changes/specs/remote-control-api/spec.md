## ADDED Requirements

### Requirement: Completed changes stay published for the owner incarnation

The `/api/v2` change projection — `snapshot.changes` of `GET /api/v2/state`, `GET /api/v2/changes`, `GET /api/v2/changes/{change_id}`, and the snapshot at every event's `state_revision` — MUST publish, for the rest of the owner incarnation, every change this incarnation has already published whose reducer-authored display status is `merged` or `pushed`, whether or not the latest successful workspace observation (an event-driven `ChangesRefreshed` or the periodic Web/UDS disk refresh) lists it. This includes a change that an earlier observation stopped publishing while it waited or failed — for example in `merge wait` after an `on_merged` hook failure, or in `error` after a publication failure — and that the reducer later completes: such a change MUST be published again no later than the first reducer event or workspace observation after that completion whose update reads reducer state. A change this incarnation has already published whose reducer-authored display status is `resolving` MUST be published under the same conditions while it is `resolving`.

A previously published row that a workspace observation does not list and whose deciding status is not `merged`, `pushed`, or `resolving` MUST be released by that observation and MUST NOT be published while it stays unlisted in such a status, exactly as before this requirement. The deciding status MUST be the reducer-authored display status from the reducer state the observation holds — the dispatch's authoritative state or the observation's non-blocking read — or, only when that observation cannot read reducer state, the row's last reducer-derived status. Only workspace observations release rows. A kept `resolving` row whose reducer-authored status has settled into another status therefore stays published, with the reducer-authored status each projection that reads the reducer publishes for it (for example `merge wait` or `error`), until the first observation that reads reducer state releases it; an observation that cannot read reducer state keeps it while its last reducer-derived status is still `resolving`. An update that cannot read reducer state MUST NOT publish a released change again. Retention and republication MUST NOT be inferred from absence, task completion, archive-directory presence, or `archive_complete`, and the reducer never authors `archived`.

To publish a released change again, the owner MAY keep, in process memory only, the last published row of every change it released. That memory MUST be private observability data: no row, total, run clock, net-active or status observation, worktree relation, or other resource may expose it or be derived from it; no scheduling, queue, mark, Start, retry, stop, dequeue, targeted force-stop, resolve, mark reconciliation, resume, acceptance, archive, or merge decision may read it; it MUST NOT be persisted or rehydrated; and every new owner process MUST start without it. A released change MUST be published again only when this incarnation published it before and the reducer authors `merged`, `pushed`, or `resolving` for it while the latest observation does not list it. When an observation lists a released change again, the owner MUST discard that memory and publish the change from the observation exactly as a change that was never kept, including new-arrival attention.

A kept or republished row MUST keep its exact `id`, its last observed listing fields (task counts, progress, progress status, dependencies, iteration), and the reducer-authored and process-local facts this owner holds for it: `archive_complete` and acceptance provenance from the reducer; `timing`, `latest_activity`, and attention as recorded from every event this owner applied, including events applied while it was released; and `net_active` as accumulated from the reducer statuses observed while it was published. Workspace-observation facts (`worktree`, `parallel`) MUST keep following the latest observation and MUST NOT be guessed. Rows the latest observation lists come first, in observation order, followed by rows kept from earlier publication in their previously published order, followed by rows published again in the order they were released. An observation that lists a published row's id again MUST replace it with exactly one row carrying the observation's listing fields and the reducer-authored status, without a duplicate and without new-arrival attention.

Retention and republication MUST happen inside the existing snapshot update, so a kept row is published in the same `state_revision` as the observation that kept it, a row published again is published in the same `state_revision` as the event or observation that published it again, and a repeated identical observation MUST NOT advance `state_revision`. A `changes_refreshed` event payload keeps counting the workspace observation; the snapshot at the event's revision remains the authority for rows.

Retention is process-local presentation state. No file, store, TTL, or later incarnation holds it, and no scheduling, queue, mark, Start, retry, stop, dequeue, targeted force-stop, resolve, mark reconciliation, resume, acceptance, archive, or merge decision may read it. Totals, action eligibility, the run clock, and net-active timing MUST classify published rows with the same shared rules as every other projected row and MUST NOT read released rows. TUI-local dismissal MUST NOT remove an API row, the API MUST NOT read TUI presentation state, and hiding or restoring rows in any client requires no Conflux command.

#### Scenario: Merged row survives the archive move and later refreshes

**Given**: this owner published change `alpha` and the reducer reports `alpha` as `merged`
**And**: `alpha`'s proposal directory has moved to `openspec/changes/archive/`, so workspace observations no longer list it
**When**: any number of event-driven `ChangesRefreshed` observations and periodic disk refreshes complete
**Then**: `GET /api/v2/changes`, `GET /api/v2/changes/alpha`, and `GET /api/v2/state` all publish `alpha` with `display_status: merged`
**And**: its `id`, last observed task counts and progress, `archive_complete`, `timing`, `latest_activity`, and `net_active` are unchanged
**And**: repeated identical observations do not advance `state_revision`

#### Scenario: Pushed row is retained like a merged row

**Given**: this owner published change `beta` and the reducer reports `beta` as `pushed`
**When**: later observations no longer list `beta`
**Then**: `beta` stays published with `display_status: pushed` for the rest of the incarnation

#### Scenario: Row integrated after its proposal left the base stays while it resolves

**Given**: `alpha` is `resolving` and its merge already landed on the base branch, so the base no longer lists it
**When**: observations complete before the reducer authors the merge completion
**Then**: `alpha` stays published as `resolving`
**And**: after the reducer authors `merged`, `alpha` stays published for the rest of the incarnation

#### Scenario: Change released while waiting is published again when the reducer completes it

**Given**: this owner published `alpha`, its `on_merged` hook failed after its merge landed, and the reducer reports `merge wait`
**And**: an event-driven observation and a disk refresh that do not list `alpha` released it
**When**: an explicit resolve makes the reducer author `resolving` and then `merged` for `alpha`
**Then**: `alpha` is published again as `resolving` in the snapshot of the first update after that transition that reads reducer state, and as `merged` afterwards
**And**: it stays published for the rest of the incarnation with its last observed task counts, `archive_complete`, and the `timing`, `latest_activity`, `net_active`, and attention an owner that never stopped listing it would publish

#### Scenario: Change released after a publication failure is published again when publication completes

**Given**: this owner published `beta`, its publication failed, and the reducer reports `error`
**And**: observations that do not list `beta` released it
**When**: the resumed publication makes the reducer author `pushed` for `beta`
**Then**: `beta` is published again as `pushed` no later than the first update after that completion that reads reducer state
**And**: it stays published for the rest of the incarnation

#### Scenario: A settled resolving row is released only by an observation that reads the reducer

**Given**: `delta` is kept as `resolving` while the base no longer lists it
**When**: the reducer authors `merge wait` for `delta`
**Then**: until the next observation that reads reducer state, `delta` stays published with the status each projection that reads the reducer publishes for it, here `merge wait`
**And**: an observation that cannot read reducer state while the last reducer-derived status of `delta` is still `resolving` keeps it
**And**: the first observation that reads reducer state and does not list `delta` releases it

#### Scenario: Completion is never inferred

**Given**: absent rows whose reducer-authored status is `not queued` with every task complete, `merge wait` or `resolve pending` with archive completion recorded, `error`, `rejected`, or `stopped`, and a row for which no reducer evidence exists
**When**: an observation no longer lists them
**Then**: they leave the projection exactly as before this requirement
**And**: they stay unpublished for as long as the reducer authors none of `merged`, `pushed`, or `resolving` for them
**And**: no row is published as `merged` or `pushed` unless the reducer authored that status

#### Scenario: Released changes stay private

**Given**: `delta` was released while the reducer reports `merge wait`
**When**: a client reads state, changes, the `delta` row, execution status, the `delta` refresh preview, or the `delta` change text, or an observation reports a worktree path for `delta` without listing it
**Then**: no row, total, execution-status entry, or worktree relation names or counts `delta`
**And**: every `delta` read answers as it does for a change this owner never observed
**And**: no command admission, mark, queue intent, or run-clock decision reads what the owner remembers about `delta`

#### Scenario: Reappearance keeps one row

**Given**: published row `alpha` is `merged` and no longer listed
**When**: a later observation lists a change directory named `alpha` again
**Then**: the projection holds exactly one `alpha` row with the observation's listing fields and the reducer-authored status
**And**: the row is not reported as newly detected

#### Scenario: A released change listed again is observed as before

**Given**: released change `epsilon` is not published and the reducer reports `merge wait`
**When**: a later observation lists `epsilon` again
**Then**: the projection holds exactly one `epsilon` row built from that observation with the reducer-authored status
**And**: it is reported as newly detected with no facts carried from before its release, exactly as before this requirement

#### Scenario: Snapshot, list, get, and stream agree

**Given**: `alpha` is retained
**When**: a client reads state, list, and get and replays the event stream from its cursor
**Then**: every response names the same `instance_id` and `state_revision` and the same `alpha` row
**And**: a `changes_refreshed` event payload counts the workspace observation while the snapshot at that event's revision still publishes `alpha`
**And**: a client that reconnects with `GET /api/v2/state` and its `event_sequence` cursor observes no removal of `alpha`
**And**: a change published again by a reducer event is published in the snapshot at that event's revision

#### Scenario: Failed, contended, or empty refresh does not erase completion

**Given**: `alpha` is retained and `delta` is released
**When**: a refresh fails before producing an observation, the reducer cannot be read while an observation is applied, or the observation lists no change at all
**Then**: `alpha` stays published with its reducer-authored status
**And**: a refresh that fails before producing an observation changes no row and no revision
**And**: `delta` is published again once a later update that reads reducer state finds it `merged`, `pushed`, or `resolving`

#### Scenario: Retention authorizes nothing

**Given**: `alpha` is retained as `merged` and may still carry an execution mark set before it completed
**When**: a client submits mark, unmark, bulk mark, queue intent, Start, retry, retry-errors, stop-and-dequeue, targeted force-stop, or resolve for `alpha`
**Then**: each settles exactly as it settles for a listed `merged` row in the same state
**And**: each settles for a released `merge wait` change exactly as for the same change while listed
**And**: no queue intent, mark, scheduler, retry edge, resolve reservation, or mode effect results from retention
**And**: `alpha.actions` reports every action blocked
**And**: totals count `alpha` as completed and never as pending or in progress
**And**: neither the run clock nor `alpha.net_active` reopens or resets

#### Scenario: Reads have no side effects

**Given**: `alpha` is retained and `delta` is released
**When**: a client repeatedly reads state, changes, the `alpha` and `delta` rows, capabilities, execution status, the refresh previews, and the event replay
**Then**: `state_revision`, `event_sequence`, the command and idempotency registries, reducer state, execution marks, and every repository file and Git ref are unchanged
**And**: `delta` is still published again when the reducer later completes it

#### Scenario: TUI and API retain the same completed rows

**Given**: the TUI and the API observe the same owner
**When**: `alpha` reaches `merged`, `beta` reaches `pushed`, `gamma` reaches `merged` through an explicit resolve after an `on_merged` failure, and `delta` reaches `pushed` through resumed publication after a publication failure, and all of them are later absent from every observation
**Then**: after each completion both frontends keep the completed change with the same `id`, display status, task counts, archive completion, and net-active time
**And**: when explicit resolve is requested with the resolve lane free, the API `run_clock` equals the TUI's Elapsed clock during the absent `resolving` refresh, at its settlement, and afterwards; no parity is guaranteed while a private `resolve pending` row waits for a busy lane

#### Scenario: TUI dismissal leaves the API row

**Given**: the TUI and the API both retain merged row `alpha`
**When**: the operator dismisses `alpha` in the TUI and later observations still do not list it
**Then**: the API `alpha` row and `state_revision` are unchanged by the dismissal
**And**: no Conflux command, reducer change, or Git mutation is involved in hiding or restoring a row in any client

#### Scenario: A new owner starts without retained rows

**Given**: an owner retained `alpha` and is replaced by a new process
**When**: the new owner observes the same workspace
**Then**: it publishes a new `instance_id` and no `alpha` row
**And**: changes still present in the workspace are observed independently
**And**: a reducer completion for a change the new owner never published publishes nothing
**And**: nothing is persisted or rehydrated from the earlier incarnation or from archived history

### Requirement: Completed change retention is discoverable

`GET /api/v2/capabilities` MUST publish `completed_change_retention` with `available: true`, `retained_statuses` exactly `merged` and `pushed`, `settling_statuses` exactly `resolving`, and `scope: instance`, meaning that every change this owner incarnation published is published, as specified by the retention requirement, while the reducer reports one of those statuses — including after it was released while waiting or failing — and that retained rows live exactly as long as the published `instance_id`. The generated OpenAPI document MUST publish the `CompletedChangeRetentionCapability` schema with those four members required, MUST publish `CapabilitiesResponse.completed_change_retention` as an optional member, and MUST describe retention on `InstanceSnapshot.changes` and `ChangesResponse.changes`. A capabilities payload that omits the block MUST decode as `available: false` with empty status lists and an empty scope — never as available — while a payload that omits any pre-existing required member MUST still be rejected. The block adds no route, command, or change-resource member, and the owner's private memory of released rows adds none either. `cflx client` MUST keep working against owners with and without the block; no client may require it to start or to operate. Published documentation MUST tell consumers that locally hidden or dismissed rows are scoped by `instance_id` and must be cleared or rekeyed when it changes.

#### Scenario: Current owner advertises the guarantee

**Given**: an owner built with this requirement serves `/api/v2`
**When**: a client reads `GET /api/v2/capabilities`
**Then**: `completed_change_retention` reports `available: true`, `retained_statuses: [merged, pushed]`, `settling_statuses: [resolving]`, and `scope: instance`
**And**: the closed command set is unchanged

#### Scenario: Older owner decodes as unavailable

**Given**: a capabilities payload from an owner that predates this requirement
**When**: a client built with this requirement decodes it
**Then**: `completed_change_retention.available` is `false` and the status lists and scope are empty
**And**: `status`, `wait`, and `mark` keep working against that owner
**And**: a payload missing a pre-existing required member is still rejected

#### Scenario: Generated contract publishes the block as optional

**Given**: a consumer reads the canonical generated OpenAPI document
**When**: it inspects `CapabilitiesResponse` and `CompletedChangeRetentionCapability`
**Then**: `completed_change_retention` is published and is not required
**And**: `available`, `retained_statuses`, `settling_statuses`, and `scope` are published and required
**And**: the `changes` descriptions of `InstanceSnapshot` and `ChangesResponse` name `completed_change_retention`
