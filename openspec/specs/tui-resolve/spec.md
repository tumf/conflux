## Purpose

TUI における resolve 操作のライフサイクル（自動トリガー、直列化、キューイング）を定義し、resolve 操作同士の排他制御と apply/accept/archive パイプラインの非ブロック保証を規定する。

## Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で即座に resolve を開始しなければならない（MUST）。`auto_resumable=true` は resolve カウンターによる判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row and no active apply changes are selected, the scheduler SHALL still consume the reducer-owned `ResolveWait` and attempt merge retry instead of treating the empty active change list as no work.

When no scheduler is running and the TUI starts a scheduler-owned run with an empty active change list solely to consume manual resolve intent, startup MUST preserve the existing shared reducer `ResolveWait` entries until the executor has synchronized and attempted that work.

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

### Requirement: resolve-merge-exclusive-execution

`resolve_merge()` が即時開始パスを取る際、システムは Project スコープの `is_resolving` フラグを即座に `true` に設定しなければならず（MUST）、同一 Project 内の後続の M キー操作がキュー追加パスに入ることを保証しなければならない（MUST）。

このフラグの影響範囲は **resolve 操作同士の直列化のみ** である。`start_processing`、`resume_processing`、`retry_error_changes` 等の apply/accept/archive パイプライン操作はこのフラグによってブロックされてはならない（MUST NOT）。

本 Requirement は旧 spec 内で 2 回重複していた同名 Requirement を 1 つに統合したものである。

#### Scenario: consecutive-m-key-press-during-resolve

**Given**: change-a が `MergeWait` 状態で、同一 Project 内で resolve が実行中でない（`is_resolving` が `false`）
**When**: change-a に対して M キーを押す
**Then**: `is_resolving` が即座に `true` になり、`TuiCommand::ResolveMerge(change-a)` が返される

#### Scenario: second-m-key-queues-when-first-resolving

**Given**: change-a の `resolve_merge()` が即時開始され `is_resolving` が `true`
**When**: 同一 Project 内の `MergeWait` 状態の change-b に対して M キーを押す
**Then**: change-b は `ResolveWait` に遷移し、resolve キューに追加される（即時開始されない）

#### Scenario: start-processing-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（`is_resolving` が `true`）
**When**: ユーザーが `start_processing` を実行する
**Then**: 選択された Change のキュー追加と処理開始が正常に行われる（`is_resolving` はチェックされない）

#### Scenario: resume-processing-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（`is_resolving` が `true`）、`AppMode` が `Stopped`
**When**: ユーザーが `resume_processing` を実行する
**Then**: マークされた Change が `Queued` に遷移し処理が再開される（`is_resolving` はチェックされない）

#### Scenario: retry-error-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（`is_resolving` が `true`）、`AppMode` が `Error`
**When**: ユーザーが `retry_error_changes` を実行する
**Then**: エラー状態の Change が `Queued` にリセットされリトライが開始される（`is_resolving` はチェックされない）
