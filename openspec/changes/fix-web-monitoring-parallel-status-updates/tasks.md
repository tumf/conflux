## 1. 実装
- [x] 1.1 `WebState` に `ExecutionEvent` を反映する関数を追加する（parallel 用）
- [x] 1.2 `--parallel` 実行のイベントハンドラから Web 更新用チャネルにイベントを送る
- [x] 1.3 WebSocket の `state_update` が常に全件スナップショットになることを保証する

## 2. テスト
- [x] 2.1 `ExecutionEvent::ProcessingStarted` で `pending → in_progress` を更新できるテストを追加
- [x] 2.2 `ExecutionEvent::ProgressUpdated` で `completed/total` と `progress_percent` が更新されるテストを追加
- [x] 2.3 `state_update` が部分更新ではなく全件スナップショットになっていることを検証

## 3. 検証
- [x] 3.1 `npx @fission-ai/openspec@latest validate fix-web-monitoring-parallel-status-updates --strict`
- [ ] 3.2 `--web --parallel` で起動し、Web 画面のステータスが更新されることを確認 (future work)
