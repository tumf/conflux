# タスク一覧: 同一エラー検出サーキットブレーカー

## 実装タスク

- [x] `ErrorHistory`構造体を追加して、エラーメッセージ履歴を保持
- [x] エラーメッセージ正規化関数を実装（JSONフィールド除外）
- [x] `detect_same_error()`メソッドで5回連続同一エラーを検出
- [x] configに`error_circuit_breaker.enabled`と`error_circuit_breaker.threshold`を追加
- [x] orchestrator.rsのエラーハンドリング部分でエラー履歴を更新
- [x] 同一エラー検出時にerrorログを出力し、changeをスキップ

## テストタスク

- [x] 同一エラーが5回連続した場合のユニットテスト
- [x] 異なるエラーが混在する場合は検出されないことをテスト
- [x] JSONフィールド名が誤検知されないことをテスト
- [x] エラー正規化関数のユニットテスト

## ドキュメント

- [x] AGENTS.mdにエラーサーキットブレーカーの説明を追加
- [x] configサンプルにerror_circuit_breaker設定を追加
