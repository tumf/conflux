## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で scheduler-owned resolve retry intent を開始または通知しなければならない（MUST）。`auto_resumable=true` は resolve カウンターまたは reducer が観測する base-mutating lane occupancy による判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row, any visible `resolve pending` state MUST correspond to reducer-owned retry membership that the scheduler can consume. The TUI MUST NOT leave a row at `resolve pending` solely because a local display transition occurred while the reducer rejected or dropped the same retry intent.

When a scheduler is already running because other changes are applying, accepting, or archiving, pressing `M` on a `MergeWait` row MUST notify the existing scheduler only after reducer-owned retry intent is accepted. The row may display `resolve pending` while waiting on scheduler/base-lane capacity, but it MUST eventually transition through scheduler events to `resolving` / `merged` or back to `merge wait` with visible failure/defer evidence.

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
