# タスク一覧: 進捗停滞検出

## 実装タスク

- [ ] `Progress History`構造体を追加して、各changeのタスク進捗を追跡
- [ ] `detect_stall()`メソッドを追加して、3回連続で進捗なしを検出
- [ ] configに`stall_detection.enabled`と`stall_detection.threshold`を追加
- [ ] orchestrator.rsのループ内で進捗履歴を更新
- [ ] 停滞検出時にwarningログを出力し、次のchangeへスキップ

## テストタスク

- [ ] 進捗が停滞した場合のユニットテストを追加
- [ ] 正常に進捗する場合は検出されないことをテスト
- [ ] config設定値の読み込みテスト

## ドキュメント

- [ ] AGENTS.mdに進捗停滞検出の説明を追加
- [ ] configサンプルにstall_detection設定を追加
