# タスク一覧: 出力量減少検出

## 実装タスク

- [ ] `OutputHistory`構造体を追加して、各apply実行の出力バイト数を記録
- [ ] agent.rsでstdout/stderrの出力サイズを計測
- [ ] `detect_output_decline()`メソッドで前回比70%減少を検出
- [ ] configに`output_decline_detector.enabled`と`output_decline_detector.threshold_percent`を追加
- [ ] orchestrator.rsのapply後に出力履歴を更新
- [ ] 出力減少検出時にwarningログを出力

## テストタスク

- [ ] 出力が70%以上減少した場合のユニットテスト
- [ ] 正常な出力減少では検出されないことをテスト
- [ ] 初回実行（履歴なし）では検出されないことをテスト

## ドキュメント

- [ ] AGENTS.mdに出力減少検出の説明を追加
- [ ] configサンプルにoutput_decline_detector設定を追加
