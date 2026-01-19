## 1. 実装
- [x] 1.1 Web UIの状態集計ロジックをTUIのQueueStatus基準に更新する（完了条件: `web/app.js` の集計関数がQueueStatusのみを参照し、pending/in_progress/completeにフォールバックしない）
- [x] 1.2 Web UIのステータス表示ルールをTUIのQueueStatusと一致させる（完了条件: `web/app.js` の表示テキストがQueueStatus.display()と一致し、Acceptingを含む）
- [x] 1.3 Web UIのステータスバッジ/色/アイコンをTUIのQueueStatusに合わせて更新する（完了条件: `web/style.css` にAcceptingのバッジ定義があり、legacy status用スタイルが削除/未使用になる）
- [x] 1.4 Web UIのステータスフォールバック方針をテストで確認する（完了条件: 仕様と一致する表示が確認できるスクリーンショット or `web/app.js` の単体チェック手順を記述）

## 2. 検証
- [x] 2.1 `npx @fission-ai/openspec@latest validate update-web-ui-change-status --strict` を実行し、エラーがないことを確認する


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ✅ Backend aggregation now uses queue_status: Updated `refresh_summary` function in `src/web/state.rs` to filter by `queue_status` ("queued", "processing", "completed") instead of legacy `status` field.
  2) ✅ Initial state calculation now uses queue_status: Updated `OrchestratorState::from_changes` method to calculate statistics using queue_status filters.
  3) ✅ Consistent aggregation between frontend and backend: Both Web UI JavaScript and backend Rust code now use queue_status exclusively for aggregation.
  4) ✅ Task 1.1 fully implemented: Backend state aggregation now aligns with frontend, both using queue_status exclusively.
  5) ✅ Integration complete: REST API endpoint `/api/state` now returns correctly calculated statistics based on queue_status.
