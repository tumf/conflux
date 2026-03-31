## Context

パラレルスケジューラ (`orchestration.rs`) は `tokio::select!` ベースのイベントループで動作する。現在、`join_set.join_next()` arm 内で `handle_workspace_completion` を await し、その中で merge + コンフリクト解決（AI エージェント呼び出し）を同期的に実行する。コンフリクト解決は数分〜数十分かかるため、その間スケジューラループ全体がブロックされる。

## Goals / Non-Goals

- Goals: resolve 中もスケジューラループが回り、queued change の dispatch が継続される
- Non-Goals: merge/resolve の並列実行（`global_merge_lock` による排他は維持）

## Decisions

- **merge+resolve をバックグラウンド tokio task に spawn する**: `handle_workspace_completion` 内で、成功した archive の merge 処理を `tokio::spawn` でバックグラウンドに移す。`select!` arm は即座に完了し、スケジューラループが継続する。
- **merge 結果の通知**: spawn したタスクの結果は `mpsc::channel` 経由でスケジューラに返す。スケジューラの `select!` に新しい arm を追加して merge 結果を受信する。
- **`retry_deferred_merges` の呼び出し**: merge 成功時にバックグラウンドタスク内から呼び出す。ただし `self` の可変参照が必要なため、結果をチャネルで返してスケジューラ側で呼び出す方が安全。
- **`global_merge_lock` は維持**: merge の排他制御は現行通り。バックグラウンドタスク内でロックを取得する。

## Risks / Trade-offs

- **`&mut self` の共有**: `ParallelExecutor` のメソッドをバックグラウンドタスクから呼ぶには、必要なデータ（`workspace_manager`, `config`, `event_tx` 等）を Arc で共有するか、結果をチャネルで返してスケジューラ側で処理する必要がある。後者の方が安全で変更量が少ない。
- **merge 結果の遅延処理**: merge 結果がスケジューラの次の `select!` iteration で処理されるため、微小な遅延が生じる。実用上問題なし。

## Open Questions

- `retry_deferred_merges` は merge 結果受信時にスケジューラ側で呼ぶか、バックグラウンドタスク内で呼ぶか → スケジューラ側で呼ぶのが `&mut self` の安全性の観点から推奨
