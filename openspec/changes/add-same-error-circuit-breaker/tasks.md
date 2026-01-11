# タスク一覧: 同一エラー検出サーキットブレーカー

## 実装タスク

1. [ ] `ErrorHistory`構造体を追加して、エラーメッセージ履歴を保持
2. [ ] エラーメッセージ正規化関数を実装（JSONフィールド除外）
3. [ ] `detect_same_error()`メソッドで5回連続同一エラーを検出
4. [ ] configに`error_circuit_breaker.enabled`と`error_circuit_breaker.threshold`を追加
5. [ ] orchestrator.rsのエラーハンドリング部分でエラー履歴を更新
6. [ ] 同一エラー検出時にerrorログを出力し、changeをスキップ

## テストタスク

7. [ ] 同一エラーが5回連続した場合のユニットテスト
8. [ ] 異なるエラーが混在する場合は検出されないことをテスト
9. [ ] JSONフィールド名が誤検知されないことをテスト
10. [ ] エラー正規化関数のユニットテスト

## ドキュメント

11. [ ] AGENTS.mdにエラーサーキットブレーカーの説明を追加
12. [ ] configサンプルにerror_circuit_breaker設定を追加
