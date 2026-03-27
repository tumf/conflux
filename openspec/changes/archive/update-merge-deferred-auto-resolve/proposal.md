# Change: 自動再評価で merge deferred を resolve へ進める

**Change Type**: implementation

## Why
- 並列実行で base への merge が他 change と衝突すると、後続 change が `pending` ではなく `MergeWait` に入ることがある。
- 現行仕様では、`MergeDeferred` は resolve 実行中でない限り `MergeWait` を維持し、先行 merge 完了後も自動で `ResolveWait` / `Resolving` に進まない。
- その結果、先行 merge によって依存条件が解消された後も、後続 change が手動 `M` を待つ見かけ上の停止状態に残りやすい。

## What Changes
- `MergeDeferred` の原因を「手動介入が必要な競合」と「先行 merge / resolve 完了後に再評価すべき待機」に分ける。
- 先行 merge または resolve の完了後、`MergeDeferred` で待機していた change を自動再評価し、競合が残る場合は `ResolveWait`、即時処理可能なら `Resolving` または merge 再試行へ進める。
- `MergeWait` は引き続き手動介入が必要なケース専用にし、自動再開可能な待機を `ResolveWait` 系のライフサイクルで表現する。
- TUI / reducer / parallel scheduler の状態遷移を揃え、先行 merge 完了後に stuck したように見える状態を解消する。

## Impact
- Affected specs: `parallel-execution`, `orchestration-state`, `tui-architecture`
- Affected code: `src/parallel/queue_state.rs`, `src/parallel/dispatch.rs`, `src/orchestration/state.rs`, `src/tui/state.rs`, `src/tui/runner.rs`

## Acceptance Criteria
- 先行 merge 完了待ちが原因の `MergeDeferred` は、完了後の再評価で自動的に `MergeWait` 以外の進行可能状態へ遷移する。
- 手動解決が必要な真の競合だけが `MergeWait` に残る。
- TUI 表示、shared reducer、parallel scheduler で `MergeWait` / `ResolveWait` の意味が一致する。
- 先行 merge 完了後も後続 change が `MergeWait` に残り続ける回帰を再現テストで防止できる。

## Out of Scope
- merge/resolve コマンド自体のアルゴリズム刷新
- 新しい VCS バックエンドの追加
- TUI のキー割り当てや基本操作体系の変更
