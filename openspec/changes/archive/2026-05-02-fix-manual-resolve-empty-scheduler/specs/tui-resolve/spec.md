## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で即座に resolve を開始しなければならない（MUST）。`auto_resumable=true` は resolve カウンターによる判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row and no active apply changes are selected, the scheduler SHALL still consume the reducer-owned `ResolveWait` and attempt merge retry instead of treating the empty active change list as no work.

#### Scenario: manual resolve starts from archived merge wait without active changes

**Given**: a TUI row for change `alpha` is in `merge wait`
**And**: `alpha` is archive-complete and no longer present as an active `openspec/changes/alpha` entry
**And**: no parallel scheduler is currently running
**When**: the user presses `M` on `alpha`
**Then**: the reducer records `ResolveWait` for `alpha`
**And**: the scheduler starts and consumes that `ResolveWait`
**And**: the system attempts the preserved-worktree merge retry for `alpha`
**And**: the row does not remain indefinitely in `resolve pending` solely because the active change list was empty

#### Scenario: empty manual resolve startup without resolve wait remains no-op

**Given**: no active changes are selected for parallel execution
**And**: the shared reducer has no change in `ResolveWait`
**When**: a scheduler-owned run starts with an empty change list
**Then**: the run completes without dispatching apply, merge retry, or conflict resolve work
