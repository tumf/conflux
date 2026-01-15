# Change: parallel 実行時に Web 画面のステータスが更新されない問題の修正

## なぜ（Why）
`--web` を有効にした状態で `--parallel` 実行すると、WebSocket は接続済み（Connected）にも関わらず、ダッシュボード上の変更ステータスが `pending` のまま更新されない。

現状の parallel 実行は Git worktree 内で `tasks.md` の進捗が更新されるため、ベース作業ツリー側の `openspec/changes/**/tasks.md` をポーリングしても進捗が反映されないケースがある。

## 何を変えるか（What Changes）
- parallel 実行（`--parallel`）のイベント（`ExecutionEvent`）を Web 監視状態（`WebState`）に橋渡しし、ダッシュボードのステータス・進捗がリアルタイムに更新されるようにする。
- WebSocket の `state_update` メッセージは、ダッシュボード互換性のため **常に変更一覧の全件スナップショット** を送信する（部分更新のみの送信を避ける）。

## 影響範囲（Impact）
- 影響する仕様: `web-monitoring`
- 関連する実装領域（参考）:
  - parallel 実行のイベント発行（`ExecutionEvent::ProgressUpdated` など）
  - Web 監視状態（`WebState`）と WebSocket ブロードキャスト

## 非ゴール（Non-Goals）
- TUI の表示・イベント処理仕様の変更
- Web UI の描画ロジックの大規模変更（UI 側の部分更新対応など）
- エラー表示（failed / error）を Web UI に追加する UI 改修

## 受け入れ条件（Acceptance Criteria）
- `--web --parallel` 実行中に、ダッシュボードのステータスバッジが `pending` から `in_progress`/`complete` に遷移する。
- `ProgressUpdated` 相当の更新で、タスク数（`completed/total`）と進捗バーが更新される。
- WebSocket 接続中はポーリングに依存せず更新できる。
