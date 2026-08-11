## MODIFIED Requirements

### Requirement: TUI ステータス表示は Reducer から導出される

TUI の Change ステータス表示（文字列・色）は reducer-derived display status を最優先に同期しなければならない（MUST）。Refresh-time `merge_wait_ids` は archived-but-not-yet-merged workspace evidence から導出された display synchronization hint として扱ってよい（MAY）が、reducer-owned active, pending, or terminal status を `merge wait` に降格してはならない（MUST NOT）。

Specifically, refresh-derived `merge_wait_ids` MUST NOT overwrite any status classified by the shared active-status vocabulary, including `preparing`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving`. It also MUST NOT overwrite reducer-derived `resolve pending`, `reject pending`, `merged`, `rejected`, `error`, or explicit `not queued` stop/dequeue state for the same change. It MAY correct stale display-only rows only when the reducer snapshot does not own one of those stronger lifecycle states for the same change.

TUI display caches remain non-authoritative observability state and MUST NOT be used as scheduler dispatch, resume routing, acceptance, archive, merge, or next-action decision inputs.

Stale archive lifecycle events MUST NOT regress a row that is already displayed as `merged` back to `archiving`. Archive-start display updates MAY mark non-terminal rows as `archiving`, but MUST preserve reducer-owned terminal success display when a stale event arrives after merge completion.

#### Scenario: refresh-derived merge wait does not overwrite active archive

**Given**: change `alpha` has received `ArchiveStarted`
**And**: the reducer and TUI display report `alpha` as `archiving`
**And**: repository inspection observes the workspace as archive-complete but not integrated into base
**When**: the TUI handles `ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: the reducer and TUI continue to report `alpha` as `archiving`
**And**: the row is not temporarily displayed as `merge wait`
**And**: the refresh does not change queue intent, execution marks, or workflow routing

#### Scenario: refresh-derived merge wait does not overwrite shared active statuses

**Given**: the reducer snapshot reports `alpha` with a status classified by the shared active-status vocabulary
**When**: the TUI handles `ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed with that active status
**And**: every status classified as active by the shared active-status vocabulary is protected by the same rule

#### Scenario: refresh-derived merge wait does not overwrite resolving

**Given**: change `alpha` is displayed as `resolving`
**And**: the reducer snapshot reports `alpha` as `resolving`
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as `resolving`
**And**: the row is not temporarily reverted to `merge wait`

#### Scenario: refresh-derived merge wait does not overwrite accepted manual resolve pending

**Given**: change `alpha` is displayed as `resolve pending`
**And**: the reducer snapshot reports `alpha` as `resolve pending` because scheduler-owned retry intent exists
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as `resolve pending`
**And**: the row is not reverted to `merge wait` solely from refresh-derived evidence

#### Scenario: refresh-derived merge wait does not overwrite terminal or error states

**Given**: change `alpha` is displayed as `merged`, `rejected`, or `error`
**When**: the TUI handles a stale `OrchestratorEvent::ChangesRefreshed` that includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as its reducer-derived state
**And**: the row is not regressed to `merge wait`

#### Scenario: refresh-derived merge wait corrects stale display-only pending

**Given**: change `alpha` is displayed locally as `resolve pending`
**And**: the reducer snapshot does not report `alpha` as active, pending, terminal, error, or explicitly stopped/dequeued
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` may be displayed as `merge wait`
**And**: the display correction does not enqueue, dispatch, archive, accept, merge, or otherwise route workflow execution

#### Scenario: fresh process restores archived workspace merge wait

**Given**: a fresh process has no reducer lifecycle history for `alpha`
**And**: repository inspection observes an archived-but-not-integrated workspace for `alpha`
**When**: refresh publishes `alpha` in `merge_wait_ids`
**Then**: the existing startup reconciliation may display `alpha` as `merge wait`
**And**: explicit manual resolve remains available through reducer-owned command handling

#### Scenario: stale archive start does not overwrite merged display

**Given**: change `alpha` is displayed as `merged`
**And**: a stale archive lifecycle event is received for `alpha`
**When**: the TUI handles `OrchestratorEvent::ArchiveStarted` for `alpha`
**Then**: `alpha` remains displayed as `merged`
**And**: the row is not regressed to `archiving`
**And**: this display protection does not enqueue, dispatch, archive, accept, or otherwise route workflow execution
