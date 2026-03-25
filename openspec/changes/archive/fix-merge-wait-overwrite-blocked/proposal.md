# Change: apply_merge_wait_status が Blocked 状態を MergeWait で上書きするバグの修正

## Why

TUI の定期リフレッシュで呼ばれる `apply_merge_wait_status()` が、依存関係により `Blocked` 状態にある変更を `MergeWait` で上書きしてしまう。これにより、依存関係が未解決であるにもかかわらず変更が MergeWait 状態として表示され、ユーザーが M キーでマージを試行できてしまう。

## What Changes

- `src/tui/state.rs` の `apply_merge_wait_status()` メソッドの除外条件に `QueueStatus::Blocked` を追加
- 回帰テストを追加

## Impact

- Affected specs: `parallel-execution`（Dependent Change Skipping 要件に関連）
- Affected code: `src/tui/state.rs` の `apply_merge_wait_status()` メソッド（1行追加）
