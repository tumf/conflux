# タスク一覧: APIレート制限ハンドリング

## 実装タスク

1. [ ] error.rsに`RateLimitError`型を追加
2. [ ] agent.rsでレート制限エラーメッセージをパース（"rate limit exceeded"等）
3. [ ] retry-afterヘッダーまたはエラーメッセージから待機時間を抽出
4. [ ] configに`rate_limit_handler.strategy`（"wait", "exit", "skip"）を追加
5. [ ] orchestrator.rsでレート制限エラー検出時に待機または終了
6. [ ] 待機中のカウントダウン表示（progressモジュール利用）

## テストタスク

7. [ ] レート制限エラーパースのユニットテスト
8. [ ] 待機戦略ごとの動作テスト
9. [ ] 待機時間計算のテスト

## ドキュメント

10. [ ] AGENTS.mdにレート制限ハンドリングの説明を追加
11. [ ] configサンプルにrate_limit_handler設定を追加
