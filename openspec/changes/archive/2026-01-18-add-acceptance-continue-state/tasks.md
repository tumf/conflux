## 1. 仕様更新
- [x] 1.1 `openspec/changes/add-acceptance-continue-state/specs/cli/spec.md` に CONTINUE 判定の要件を追加する（再実行・上限・戻り先を明記）
  - 完了条件: `#### Scenario:` を含む要件が追加されている
- [x] 1.2 `openspec/changes/add-acceptance-continue-state/specs/configuration/spec.md` に再実行上限の設定項目を追加する
  - 完了条件: `#### Scenario:` を含む要件が追加されている

## 2. 実装
- [x] 2.1 `src/acceptance.rs` のパーサを `ACCEPTANCE: CONTINUE` に対応させる
  - 完了条件: CONTINUE のテストケースが追加されている
- [x] 2.2 逐次ループで CONTINUE を検出した場合に acceptance を再実行する
  - 完了条件: `src/orchestrator.rs` にループ分岐が追加されている
- [x] 2.3 並列実行でも CONTINUE を検出した場合に acceptance を再実行する
  - 完了条件: `src/parallel/` の acceptance 経路に分岐が追加されている
- [x] 2.4 CONTINUE の試行回数を履歴に記録し、上限を超えたら FAIL として apply に戻す
  - 完了条件: acceptance 履歴に attempt が記録される

## 3. 検証
- [x] 3.1 `cargo test` を実行し、acceptance パーサのテストが通る
  - 完了条件: `cargo test` が成功する
