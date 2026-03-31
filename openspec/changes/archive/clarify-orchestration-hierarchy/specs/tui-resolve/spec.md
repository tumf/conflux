## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で即座に resolve を開始しなければならない（MUST）。`auto_resumable=true` は resolve カウンターによる判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に1つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない。

#### Scenario: auto-resumable deferred with no active resolve

**Given**: 同一 Project 内で resolve が実行中でない（`is_resolving` が `false`）
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当 Change が `ResolveWait` に遷移し、`TuiCommand::ResolveMerge` が返され、resolve が即座に開始される

#### Scenario: auto-resumable deferred with active resolve

**Given**: 同一 Project 内で別の Change が resolve 中である（`is_resolving` が `true`）
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当 Change が `ResolveWait` に遷移し、`resolve_queue` に追加され、現在の resolve 完了後に自動開始される

#### Scenario: dirty-base-without-active-resolve-sends-resolve-failed

**Given**: Project 内の resolve_counter が 1（自分自身のみ）、base branch が dirty
**When**: resolve コマンドが base_dirty_reason() で dirty を検出する
**Then**: `ResolveFailed` イベントが送出され、Change は MergeWait に遷移する

### Requirement: resolve-merge-exclusive-execution

resolve_merge() が即時開始パスを取る際、システムは Project スコープの `is_resolving` フラグを即座に true に設定しなければならず（MUST）、同一 Project 内の後続の M キー操作がキュー追加パスに入ることを保証しなければならない（MUST）。

このフラグの影響範囲は **resolve 操作同士の直列化のみ** である。`start_processing`、`resume_processing`、`retry_error_changes` 等の apply/accept/archive パイプライン操作はこのフラグによってブロックされてはならない。

#### Scenario: consecutive-m-key-press-during-resolve

**Given**: change-a が MergeWait 状態で、同一 Project 内で resolve が実行中でない（is_resolving が false）
**When**: change-a に対して M キーを押す
**Then**: is_resolving が即座に true になり、TuiCommand::ResolveMerge(change-a) が返される

#### Scenario: second-m-key-queues-when-first-resolving

**Given**: change-a の resolve_merge() が即時開始され is_resolving が true
**When**: 同一 Project 内の MergeWait 状態の change-b に対して M キーを押す
**Then**: change-b は ResolveWait に遷移し、resolve キューに追加される（即時開始されない）

#### Scenario: start-processing-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（is_resolving が true）
**When**: ユーザーが start_processing を実行する
**Then**: 選択された Change のキュー追加と処理開始が正常に行われる（is_resolving はチェックされない）

#### Scenario: resume-processing-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（is_resolving が true）、AppMode が Stopped
**When**: ユーザーが resume_processing を実行する
**Then**: マークされた Change が Queued に遷移し処理が再開される（is_resolving はチェックされない）

#### Scenario: retry-error-not-blocked-by-resolving

**Given**: 同一 Project 内のある Change が Resolving 状態である（is_resolving が true）、AppMode が Error
**When**: ユーザーが retry_error_changes を実行する
**Then**: エラー状態の Change が Queued にリセットされリトライが開始される（is_resolving はチェックされない）
