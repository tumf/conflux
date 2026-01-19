## 1. 実装
- [ ] 1.1 Web UIの状態集計ロジックをTUIのQueueStatus基準に更新する（完了条件: `web/app.js` の集計関数がQueueStatusのみを参照し、pending/in_progress/completeにフォールバックしない）
- [ ] 1.2 Web UIのステータス表示ルールをTUIのQueueStatusと一致させる（完了条件: `web/app.js` の表示テキストがQueueStatus.display()と一致し、Acceptingを含む）
- [ ] 1.3 Web UIのステータスバッジ/色/アイコンをTUIのQueueStatusに合わせて更新する（完了条件: `web/style.css` にAcceptingのバッジ定義があり、legacy status用スタイルが削除/未使用になる）
- [ ] 1.4 Web UIのステータスフォールバック方針をテストで確認する（完了条件: 仕様と一致する表示が確認できるスクリーンショット or `web/app.js` の単体チェック手順を記述）

## 2. 検証
- [ ] 2.1 `npx @fission-ai/openspec@latest validate update-web-ui-change-status --strict` を実行し、エラーがないことを確認する
