## Purpose

TUI における resolve 操作のライフサイクル（自動トリガー、直列化、キューイング）を定義し、resolve 操作同士の排他制御と apply/accept/archive パイプラインの非ブロック保証を規定する。

## Requirements

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
