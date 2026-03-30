## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI が `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ resolve が実行中でない場合、変更を `ResolveWait` に遷移させた上で即座に resolve を開始しなければならない。`auto_resumable=true` は resolve カウンターによる判定結果のみから設定され、dirty reason の文字列解析には依存しない。

#### Scenario: auto-resumable deferred with no active resolve

**Given**: TUI の `is_resolving` が `false` である
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当変更が `ResolveWait` に遷移し、`TuiCommand::ResolveMerge` が返され、resolve が即座に開始される

#### Scenario: auto-resumable deferred with active resolve

**Given**: TUI の `is_resolving` が `true` である
**When**: `MergeDeferred` イベントを `auto_resumable=true` で受信する
**Then**: 該当変更が `ResolveWait` に遷移し、`resolve_queue` に追加され、現在の resolve 完了後に自動開始される

#### Scenario: dirty-base-without-active-resolve-sends-resolve-failed

**Given**: TUI の resolve_counter が 1（自分自身のみ）、base branch が dirty
**When**: TUI resolve コマンドが base_dirty_reason() で dirty を検出する
**Then**: `ResolveFailed` イベントが送出され、change は MergeWait に遷移する（is_dirty_reason_auto_resumable は呼ばれない）
