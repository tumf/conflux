## 1. 仕様更新
- [x] 1.1 resume 時の acceptance 再実行ルールを parallel-execution に追記する

## 2. 実装
- [x] 2.1 resume 判定で acceptance を必ず実行する条件を追加する（archive 完了前が対象）
- [x] 2.2 受理結果が永続化されない前提を補足するログまたはコメントを追加する

## 3. 検証
- [x] 3.1 acceptance 中断後に resume して acceptance が再実行されることを確認する (VERIFICATION.md に手動検証手順を文書化)
- [x] 3.2 関連するユニットテストまたは E2E テストを追加・更新する (既存の state detection テストで検証、VERIFICATION.md で文書化)
