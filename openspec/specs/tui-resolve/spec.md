## Purpose

TUI における resolve 操作のライフサイクル（自動トリガー、直列化、キューイング）を定義し、resolve 操作同士の排他制御と apply/accept/archive パイプラインの非ブロック保証を規定する。

## Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で scheduler-owned resolve retry intent を開始または通知しなければならない（MUST）。`auto_resumable=true` は resolve カウンターまたは reducer が観測する base-mutating lane occupancy による判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row, any visible `resolve pending` state MUST correspond to reducer-owned retry membership that the scheduler can consume. The TUI MUST NOT leave a row at `resolve pending` solely because a local display transition occurred while the reducer rejected or dropped the same retry intent. Conversely, after the reducer accepts manual resolve intent, refresh-derived `merge_wait_ids` MUST NOT revert the visible row from `resolve pending` to `merge wait` while the scheduler-owned retry remains pending.

When a scheduler is already running because other changes are applying, accepting, or archiving, pressing `M` on a `MergeWait` row MUST notify the existing scheduler only after reducer-owned retry intent is accepted. The row may display `resolve pending` while waiting on scheduler/base-lane capacity, but it MUST eventually transition through scheduler events to `resolving` / `merged` or back to `merge wait` with visible failure/defer evidence.

When a manual merge retry starts through the resolve lifecycle and the successful repository integration is reported as `MergeCompleted` rather than `ResolveCompleted`, the TUI MUST treat that `MergeCompleted` event as closing the local resolve lifecycle. It MUST clear any stale `is_resolving` reservation and MUST dispatch the next queued resolve retry intent, if one exists.

<!-- Expected canonical result after archive: `tui-resolve` will explicitly close local resolve lifecycle state on MergeCompleted success paths used by manual merge retry, preventing stale resolve reservations from blocking later M-key scheduler notifications. -->

#### Scenario: manual resolve pending remains scheduler-consumable after archived merge wait

**Given**: a TUI row for change `alpha` is visible as `merge wait`
**And**: `alpha` is archive-complete, not yet merged into the base branch, and remains repository-visible merge-retry work
**When**: the user presses `M` on `alpha`
**Then**: the reducer records scheduler-consumable `ResolveWait` for `alpha`
**And**: the scheduler can later consume that retry intent after queue notification or slot release
**And**: `alpha` does not remain indefinitely in `resolve pending` solely because the pending state was display-only

#### Scenario: reducer-rejected manual resolve does not become false pending

**Given**: a TUI row for change `alpha` appears retryable locally
**And**: reducer-owned state determines `alpha` is not actually eligible for `ResolveMerge`
**When**: the user presses `M` on `alpha`
**Then**: the command does not leave `alpha` in a persistent `resolve pending` state
**And**: scheduler notification is not treated as accepted retry work for `alpha`
**And**: the user-visible state returns to a truthful blocker or terminal status with visible evidence

#### Scenario: accepted manual resolve pending survives refresh merge-wait evidence

**Given**: a TUI row for change `alpha` is visible as `resolve pending`
**And**: reducer-owned state contains scheduler-consumable `ResolveWait` for `alpha`
**And**: no `ResolveFailed`, `MergeDeferred(auto_resumable=false)`, `ResolveCompleted`, or `MergeCompleted` event has cleared that intent
**When**: the periodic refresh reports `alpha` in `merge_wait_ids`
**Then**: the row remains visible as `resolve pending`
**And**: the scheduler-owned retry intent remains available for dispatch
**And**: the row returns to `merge wait` only after explicit failure or manual-deferral evidence

#### Scenario: merge-completed-closes-manual-resolve-lifecycle

**Given**: change `alpha` started a manual merge retry from a `merge wait` row through the TUI resolve lifecycle
**And**: the TUI local `is_resolving` flag is reserved for that retry
**When**: `alpha` emits `MergeCompleted` after successful repository integration
**Then**: the TUI clears the local `is_resolving` reservation
**And**: a later `M` press on another `merge wait` row can emit `TuiCommand::ResolveMerge` instead of becoming display-only `resolve pending`

#### Scenario: merge-completed-dispatches-next-queued-resolve

**Given**: change `alpha` started a manual merge retry through the TUI resolve lifecycle
**And**: change `beta` is queued in the TUI local resolve queue
**When**: `alpha` emits `MergeCompleted`
**Then**: the TUI marks `alpha` as `merged`
**And**: the TUI sets `beta` to `resolve pending`
**And**: the event handler returns `TuiCommand::ResolveMerge(beta)` so the scheduler can be notified

### Requirement: resolve-merge-exclusive-execution

When a user requests merge resolution (`M` key) on a `MergeWait` change while another resolve is in progress, the change must transition to `ResolveWait` and remain in that state until the resolve is actually started or explicitly cancelled. The transition must be synchronized to both the TUI-local state and the shared orchestrator reducer.

A queued resolve MUST be advanced when the active resolve lifecycle completes through either `ResolveCompleted` or `MergeCompleted`, because parallel manual merge retry reports successful repository integration with `MergeCompleted`.

<!-- Expected canonical result after archive: `tui-resolve-queue` will specify that queued resolve advancement is triggered by both ResolveCompleted and MergeCompleted success events when they close an active resolve lifecycle. -->

#### Scenario: queued-resolve-survives-refresh

**Given**: Change A is in `Resolving` state and Change B is in `MergeWait` state in the TUI
**When**: The user presses `M` on Change B, then a `ChangesRefreshed` event fires with Change B's workspace still in `Archived` state
**Then**: Change B remains in `ResolveWait` ("resolve pending") in both the TUI display and the shared reducer state

#### Scenario: queued-resolve-eventually-executes

**Given**: Change B has been queued for resolve via `M` key while Change A was resolving
**When**: Change A's resolve completes
**Then**: Change B's resolve is started from the queue

#### Scenario: queued-resolve-advances-after-merge-completed

**Given**: Change A was started from the TUI resolve lifecycle and has Change B queued behind it
**When**: Change A emits `MergeCompleted` instead of `ResolveCompleted`
**Then**: Change B's resolve retry command is emitted from the queue
**And**: Change B does not remain indefinitely in `resolve pending` because the prior resolve lifecycle ended with a merge completion event
