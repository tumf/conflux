## MODIFIED Requirements

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
