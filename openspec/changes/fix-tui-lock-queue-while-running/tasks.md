## 1. 仕様
- [x] 1.1 `cli` spec の Dynamic Execution Queue 要件を更新する（未着手の queued change は実行前に外せる）

## 2. 実装（TUI / serial）
- [x] 2.1 キュー削除イベントをオーケストレータの pending から除外し、該当 change が実行対象にならないことを確認する
- [x] 2.2 Processing/Archiving の change への操作無効化を維持し、対象状態では削除できないことを確認する

## 3. テスト
- [x] 3.1 Running 中に queued change を外すと未着手なら実行されないことを検証するユニットテストを追加する
- [x] 3.2 Processing/Archiving の change を外せないことを確認する既存テストを維持・更新する

## Future work
- 手動: Running 中に queued change を外すと実行されない
- 手動: Processing/Archiving の change は操作できない
