## ADDED Requirements

### Requirement: Completed changes stay published for the owner incarnation

The `/api/v2` change projection — `snapshot.changes` of `GET /api/v2/state`, `GET /api/v2/changes`, `GET /api/v2/changes/{change_id}`, and the snapshot at every event's `state_revision` — MUST keep a row this owner incarnation already published when a successful workspace observation (an event-driven `ChangesRefreshed` or the periodic Web/UDS disk refresh) no longer lists it and the reducer-authored display status of that change is `merged` or `pushed`. Such a row MUST stay published for the rest of the incarnation. A previously published row whose reducer-authored display status is `resolving` MUST stay published through the same absence only until it settles: once its reducer-authored status is no longer `resolving`, `merged`, or `pushed`, the next observation that does not list it MUST release it. Every other absent row MUST leave the projection exactly as before this requirement.

The status that decides retention MUST be the one reported by the reducer state the observation holds — the dispatch's authoritative state or the observation's non-blocking read — or, when no reducer state can be read for that observation, the row's last reducer-derived status. Retention MUST NOT be inferred from absence, task completion, archive-directory presence, or `archive_complete`, and the reducer never authors `archived`.

A retained row MUST keep its exact `id`, its last observed listing fields (task counts, progress, progress status, dependencies, iteration), and the reducer-authored and process-local facts this owner holds for it (`archive_complete`, acceptance provenance, `timing`, `latest_activity`, `net_active`). Workspace-observation facts (`worktree`, `parallel`) MUST keep following the current observation and MUST NOT be guessed. Rows the observation lists come first, in observation order, followed by retained rows in their previously published order. An observation that lists a retained ID again MUST replace it with exactly one row carrying the observation's listing fields and the reducer-authored status, without a duplicate and without new-arrival attention.

Retention MUST happen inside the existing snapshot update, so a retained row is published in the same `state_revision` as the observation that kept it, and a repeated identical observation MUST NOT advance `state_revision`. A `changes_refreshed` event payload keeps counting the workspace observation; the snapshot at the event's revision remains the authority for rows.

Retention is process-local presentation state. No file, store, TTL, or later incarnation holds it, and no scheduling, queue, mark, Start, retry, stop, dequeue, targeted force-stop, resolve, mark reconciliation, resume, acceptance, archive, or merge decision may read it. Totals, action eligibility, the run clock, and net-active timing MUST classify retained rows with the same shared rules as every other projected row. TUI-local dismissal MUST NOT remove an API row, the API MUST NOT read TUI presentation state, and hiding or restoring rows in any client requires no Conflux command.

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

#### Scenario: Row integrated after its proposal left the base stays until it settles

**Given**: `alpha` is `resolving` and its merge already landed on the base branch, so the base no longer lists it
**When**: observations complete before the reducer authors the merge completion
**Then**: `alpha` stays published as `resolving`
**And**: after the reducer authors `merged`, `alpha` stays published for the rest of the incarnation
**And**: if `alpha` settles into any status other than `merged` or `pushed`, the next observation that does not list it releases it

#### Scenario: Completion is never inferred

**Given**: absent rows whose reducer-authored status is `not queued` with every task complete, `merge wait` or `resolve pending` with archive completion recorded, `error`, `rejected`, or `stopped`, and a row for which no reducer evidence exists
**When**: an observation no longer lists them
**Then**: they leave the projection exactly as before this requirement
**And**: no row is published as `merged` or `pushed` unless the reducer authored that status

#### Scenario: Reappearance keeps one row

**Given**: retained row `alpha` is `merged`
**When**: a later observation lists a change directory named `alpha` again
**Then**: the projection holds exactly one `alpha` row with the observation's listing fields and the reducer-authored status
**And**: the row is not reported as newly detected

#### Scenario: Snapshot, list, get, and stream agree

**Given**: `alpha` is retained
**When**: a client reads state, list, and get and replays the event stream from its cursor
**Then**: every response names the same `instance_id` and `state_revision` and the same `alpha` row
**And**: a `changes_refreshed` event payload counts the workspace observation while the snapshot at that event's revision still publishes `alpha`
**And**: a client that reconnects with `GET /api/v2/state` and its `event_sequence` cursor observes no removal of `alpha`

#### Scenario: Failed, contended, or empty refresh does not erase completion

**Given**: `alpha` is retained
**When**: a refresh fails before producing an observation, the reducer cannot be read while an observation is applied, or the observation lists no change at all
**Then**: `alpha` stays published with its reducer-authored status
**And**: a refresh that fails before producing an observation changes no row and no revision

#### Scenario: Retention authorizes nothing

**Given**: `alpha` is retained as `merged` and may still carry an execution mark set before it completed
**When**: a client submits mark, unmark, bulk mark, queue intent, Start, retry, retry-errors, stop-and-dequeue, targeted force-stop, or resolve for `alpha`
**Then**: each settles exactly as it settles for a listed `merged` row in the same state
**And**: no queue intent, mark, scheduler, retry edge, resolve reservation, or mode effect results
**And**: `alpha.actions` reports every action blocked
**And**: totals count `alpha` as completed and never as pending or in progress
**And**: neither the run clock nor `alpha.net_active` reopens or resets

#### Scenario: Reads have no side effects

**Given**: `alpha` is retained
**When**: a client repeatedly reads state, changes, the `alpha` row, capabilities, execution status, the `alpha` refresh preview, and the event replay
**Then**: `state_revision`, `event_sequence`, the command and idempotency registries, reducer state, execution marks, and every repository file and Git ref are unchanged

#### Scenario: TUI and API retain the same completed rows

**Given**: the TUI and the API observe the same owner
**When**: `alpha` reaches `merged`, `beta` reaches `pushed`, and both are later absent from every observation
**Then**: both frontends keep `alpha` and `beta` with the same `id`, display status, task counts, archive completion, and net-active time

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
**And**: nothing is persisted or rehydrated from the earlier incarnation or from archived history

### Requirement: Completed change retention is discoverable

`GET /api/v2/capabilities` MUST publish `completed_change_retention` with `available: true`, `retained_statuses` exactly `merged` and `pushed`, `settling_statuses` exactly `resolving`, and `scope: instance`, meaning retained rows live exactly as long as the published `instance_id`. The generated OpenAPI document MUST publish the `CompletedChangeRetentionCapability` schema with those four members required, MUST publish `CapabilitiesResponse.completed_change_retention` as an optional member, and MUST describe retention on `InstanceSnapshot.changes` and `ChangesResponse.changes`. A capabilities payload that omits the block MUST decode as `available: false` with empty status lists and an empty scope — never as available — while a payload that omits any pre-existing required member MUST still be rejected. The block adds no route, command, or change-resource member, and `cflx client` MUST keep working against owners with and without it; no client may require it to start or to operate. Published documentation MUST tell consumers that locally hidden or dismissed rows are scoped by `instance_id` and must be cleared or rekeyed when it changes.

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
