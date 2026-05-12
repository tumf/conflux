## MODIFIED Requirements

### Requirement: TUI ステータス表示は Reducer から導出される

TUI の Change ステータス表示（文字列・色）は reducer-derived display status を最優先に同期しなければならない（MUST）。Refresh-time `merge_wait_ids` は archived-but-not-yet-merged workspace evidence から導出された display synchronization hint として扱ってよい（MAY）が、reducer-owned active, pending, or terminal status を `merge wait` に降格してはならない（MUST NOT）。

Specifically, refresh-derived `merge_wait_ids` MUST NOT overwrite reducer-derived `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error` for the same change. It MAY correct stale display-only rows only when the reducer snapshot does not own one of those stronger lifecycle states for the same change.

TUI display caches remain non-authoritative observability state and MUST NOT be used as scheduler dispatch, resume routing, acceptance, archive, or next-action decision inputs.

<!-- Expected canonical result after archive: `tui-state` will protect reducer-owned `resolving` and other active/pending/terminal statuses from refresh-derived false `merge wait`, while preserving stale display-only correction. -->

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
**And**: the reducer snapshot does not report `alpha` as `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error`
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` may be displayed as `merge wait`
**And**: the display correction does not enqueue, dispatch, archive, accept, or otherwise route workflow execution
