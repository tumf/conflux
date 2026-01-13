## 1. 仕様
- [ ] 1.1 `cli` spec の Dynamic Execution Queue 要件を更新する（未着手の queued change は実行前に外せる）

## 2. 実装（TUI / serial）
- [ ] 2.1 キュー削除イベントをオーケストレータの pending から除外する
- [ ] 2.2 Processing/Archiving の change への操作は従来通り無効化を維持する

## 3. テスト
- [ ] 3.1 Running 中に queued change を外すと未着手なら実行されないことのユニットテスト
- [ ] 3.2 Processing/Archiving の change を外せないことの既存テスト維持

## 4. 検証
- [ ] 4.1 手動: Running 中に queued change を外すと実行されない
- [ ] 4.2 手動: Processing/Archiving の change は操作できない
