## Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI が `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ resolve が実行中でない場合、変更を `ResolveWait` に遷移させた上で即座に resolve を開始しなければならない。

#### Scenario: auto-resumable deferred with no active resolve

**Given**: TUI の `is_resolving` が `false` である
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当変更が `ResolveWait` に遷移し、`TuiCommand::ResolveMerge` が返され、resolve が即座に開始される

#### Scenario: auto-resumable deferred with active resolve

**Given**: TUI の `is_resolving` が `true` である
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当変更が `ResolveWait` に遷移し、`resolve_queue` に追加され、現在の resolve 完了後に自動開始される


### Requirement: resolve-merge-exclusive-execution

resolve_merge() が即時開始パスを取る際、is_resolving フラグを即座に true に設定し、後続の M キー操作がキュー追加パスに入ることを保証する。

#### Scenario: consecutive-m-key-press-during-resolve

**Given**: change-a が MergeWait 状態で、is_resolving が false
**When**: change-a に対して M キーを押す
**Then**: is_resolving が即座に true になり、TuiCommand::ResolveMerge(change-a) が返される

#### Scenario: second-m-key-queues-when-first-resolving

**Given**: change-a の resolve_merge() が即時開始され is_resolving が true
**When**: MergeWait 状態の change-b に対して M キーを押す
**Then**: change-b は ResolveWait に遷移し、resolve キューに追加される（即時開始されない）
