# タスク一覧: 連続完了シグナル検出

## 実装タスク

1. [ ] `DoneSignalHistory`構造体を追加して、完了シグナルを追跡
2. [ ] agent.rsでエージェント出力から完了シグナルをパース（"done", "completed"等）
3. [ ] `detect_consecutive_done()`メソッドで2回連続を検出
4. [ ] configに`done_signal_detector.enabled`と`done_signal_detector.threshold`を追加
5. [ ] orchestrator.rsのapply後に完了シグナル履歴を更新
6. [ ] 連続完了検出時にchangeを強制的に完了状態にマーク

## テストタスク

7. [ ] 2回連続で完了シグナルが出た場合のユニットテスト
8. [ ] 1回だけでは検出されないことをテスト
9. [ ] 完了シグナルパース関数のユニットテスト

## ドキュメント

10. [ ] AGENTS.mdに連続完了シグナル検出の説明を追加
11. [ ] configサンプルにdone_signal_detector設定を追加
