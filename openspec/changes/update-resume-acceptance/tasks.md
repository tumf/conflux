## 1. 仕様更新
- [ ] 1.1 resume 時の acceptance 再実行ルールを parallel-execution に追記する

## 2. 実装
- [ ] 2.1 resume 判定で acceptance を必ず実行する条件を追加する（archive 完了前が対象）
- [ ] 2.2 受理結果が永続化されない前提を補足するログまたはコメントを追加する

## 3. 検証
- [ ] 3.1 acceptance 中断後に resume して acceptance が再実行されることを確認する
- [ ] 3.2 関連するユニットテストまたは E2E テストを追加・更新する
