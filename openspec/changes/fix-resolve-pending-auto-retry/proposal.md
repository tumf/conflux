# Fix: auto-resumable resolve pending が先行 resolve 完了後も残り続ける

**Change Type**: implementation

## Problem / Context

TUI の parallel モードにおいて、`MergeDeferred(auto_resumable=true)` を受信した際に `is_resolving == false` の場合、`handle_merge_deferred()` は変更を `QueueStatus::ResolveWait`（"resolve pending"）に遷移させるが、TUI 側の `resolve_queue` には追加せず、自動リトライのための `TuiCommand::ResolveMerge` も返さない。

結果として、先行するマージ/resolve が完了しても、resolve pending のまま残り続け、ユーザが手動で M キーを押さない限りマージが進まない。

### 根本原因

`src/tui/state.rs` の `handle_merge_deferred()` 内（line 2031-2041）:

```rust
} else if auto_resumable {
    // ResolveWait に設定するが、resolve_queue にも追加せず、
    // TuiCommand も返さない → 誰もリトライをトリガーしない
    change.queue_status = QueueStatus::ResolveWait;
}
```

parallel executor 側の `merge_deferred_changes` + `retry_deferred_merges()` は executor がアクティブな間のみ機能するため、executor 完了後のイベントや TUI 手動操作経由では機能しない。

## Proposed Solution

`handle_merge_deferred()` で `auto_resumable=true && !is_resolving` の場合に、`resolve_queue` に追加し、resolve を即時開始する `TuiCommand::ResolveMerge` を返すようにする。

これにより:
1. resolve が実行中でない場合 → 即座に resolve 開始
2. resolve が実行中の場合 → resolve_queue に追加されて順番待ち（既存の動作）

`handle_merge_deferred()` の戻り値を `()` から `Option<TuiCommand>` に変更する。

## Acceptance Criteria

1. `MergeDeferred(auto_resumable=true)` 受信時に resolve が未実行であれば、自動的に resolve が開始される
2. resolve 実行中に受信した場合は、既存の resolve_queue 経由で後続リトライされる
3. `auto_resumable=false` の場合は MergeWait のまま（変更なし）
4. 既存テストが全て pass する
5. `cargo clippy -- -D warnings` が pass する

## Out of Scope

- parallel executor 側の `retry_deferred_merges()` ロジック変更
- Web ダッシュボードでの resolve pending 表示
- headless モードでの同様のバグ（存在する場合は別プロポーザル）
