# タスク一覧: APIレート制限ハンドリング

## 実装タスク

- [ ] error.rsに`RateLimitError`型を追加
- [ ] agent.rsでレート制限エラーメッセージをパース（"rate limit exceeded"等）
- [ ] retry-afterヘッダーまたはエラーメッセージから待機時間を抽出
- [ ] configに`rate_limit_handler.strategy`（"wait", "exit", "skip"）を追加
- [ ] orchestrator.rsでレート制限エラー検出時に待機または終了
- [ ] 待機中のカウントダウン表示（progressモジュール利用）

## テストタスク

- [ ] レート制限エラーパースのユニットテスト
- [ ] 待機戦略ごとの動作テスト
- [ ] 待機時間計算のテスト

## ドキュメント

- [ ] AGENTS.mdにレート制限ハンドリングの説明を追加
- [ ] configサンプルにrate_limit_handler設定を追加
