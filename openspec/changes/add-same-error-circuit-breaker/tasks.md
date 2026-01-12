# タスク一覧: 同一エラー検出サーキットブレーカー

## 実装タスク

- [ ] `ErrorHistory`構造体を追加して、エラーメッセージ履歴を保持
- [ ] エラーメッセージ正規化関数を実装（JSONフィールド除外）
- [ ] `detect_same_error()`メソッドで5回連続同一エラーを検出
- [ ] configに`error_circuit_breaker.enabled`と`error_circuit_breaker.threshold`を追加
- [ ] orchestrator.rsのエラーハンドリング部分でエラー履歴を更新
- [ ] 同一エラー検出時にerrorログを出力し、changeをスキップ

## テストタスク

- [ ] 同一エラーが5回連続した場合のユニットテスト
- [ ] 異なるエラーが混在する場合は検出されないことをテスト
- [ ] JSONフィールド名が誤検知されないことをテスト
- [ ] エラー正規化関数のユニットテスト

## ドキュメント

- [ ] AGENTS.mdにエラーサーキットブレーカーの説明を追加
- [ ] configサンプルにerror_circuit_breaker設定を追加
