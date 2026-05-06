## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で scheduler-owned resolve retry intent を開始または通知しなければならない（MUST）。`auto_resumable=true` は resolve カウンターまたは reducer が観測する base-mutating lane occupancy による判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row and no active changes are selected, the scheduler SHALL still consume the reducer-owned `ResolveWait` and attempt merge retry instead of treating the empty active change list as no work.

When no scheduler is running and the TUI starts a scheduler-owned run with an empty active change list solely to consume manual resolve intent, startup MUST preserve the existing shared reducer `ResolveWait` entries until the executor has synchronized and attempted that work.

When a scheduler is already running because other changes are applying, accepting, or archiving, pressing `M` on a `MergeWait` row MUST notify the existing scheduler and MUST leave retry execution owned by that scheduler. The row may display `resolve pending` while waiting for capacity, but it MUST eventually transition through scheduler events to `resolving` / `merged` or back to `merge wait` with visible failure/defer evidence.

#### Scenario: manual resolve starts from archived merge wait without active changes

**Given**: a TUI row for change `alpha` is in `merge wait`
**And**: `alpha` is archive-complete and no longer present as an active `openspec/changes/alpha` entry
**And**: no parallel scheduler is currently running
**When**: the user presses `M` on `alpha`
**Then**: the reducer records `ResolveWait` for `alpha`
**And**: the scheduler starts without clearing that reducer-owned `ResolveWait`
**And**: the scheduler consumes that `ResolveWait`
**And**: the system attempts the preserved-worktree merge retry for `alpha`
**And**: the row does not remain indefinitely in `resolve pending` solely because the active change list was empty

#### Scenario: empty manual resolve startup preserves reducer state

**Given**: the shared reducer contains `alpha` in `ResolveWait`
**And**: no active changes are selected for parallel execution
**When**: `run_orchestrator_parallel` starts with an empty active change list to handle manual resolve
**Then**: startup MUST NOT replace the shared reducer with an empty `OrchestratorState`
**And**: `alpha` remains visible through `resolve_wait_change_ids()` until scheduler retry handling consumes or clears it

#### Scenario: empty manual resolve startup without resolve wait remains no-op

**Given**: no active changes are selected for parallel execution
**And**: the shared reducer has no change in `ResolveWait`
**When**: a scheduler-owned run starts with an empty change list
**Then**: the run completes without dispatching apply, merge retry, or conflict resolve work

#### Scenario: selected-change startup still resets run state

**Given**: one or more active changes are selected for parallel execution
**And**: the shared reducer contains stale runtime state from a previous run
**When**: `run_orchestrator_parallel` starts with the selected change IDs
**Then**: startup initializes a fresh parallel run state for those selected changes
**And**: it re-applies queued intent only for the selected change IDs
**And**: stale `ResolveWait` from the previous run does not leak into the new selected run

#### Scenario: manual resolve while another change is applying notifies existing scheduler

**Given**: the parallel scheduler is running because change `beta` is applying
**And**: change `alpha` is visible in the TUI as `merge wait`
**When**: the user presses `M` on `alpha`
**Then**: the TUI records reducer-owned `ResolveWait` for `alpha`
**And**: the command handler notifies the existing scheduler instead of executing merge directly
**And**: `beta` is not stopped or suppressed by the resolve intent
**When**: scheduler-owned retry for `alpha` succeeds or fails with manual deferral
**Then**: the TUI leaves `resolve pending` through the corresponding reducer event and displays `merged`, `resolving`, or `merge wait` rather than staying pending indefinitely
