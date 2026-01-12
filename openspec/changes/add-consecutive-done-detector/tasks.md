# タスク一覧: 連続完了シグナル検出

## 実装タスク

- [ ] `DoneSignalHistory`構造体を追加して、完了シグナルを追跡
- [ ] agent.rsでエージェント出力から完了シグナルをパース（"done", "completed"等）
- [ ] `detect_consecutive_done()`メソッドで2回連続を検出
- [ ] configに`done_signal_detector.enabled`と`done_signal_detector.threshold`を追加
- [ ] orchestrator.rsのapply後に完了シグナル履歴を更新
- [ ] 連続完了検出時にchangeを強制的に完了状態にマーク

## テストタスク

- [ ] 2回連続で完了シグナルが出た場合のユニットテスト
- [ ] 1回だけでは検出されないことをテスト
- [ ] 完了シグナルパース関数のユニットテスト

## ドキュメント

- [ ] AGENTS.mdに連続完了シグナル検出の説明を追加
- [ ] configサンプルにdone_signal_detector設定を追加
