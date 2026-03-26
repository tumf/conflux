## 1. キャラクタリゼーション
- [ ] 1.1 既存の `parse_acceptance_output` 挙動を固定するテストケースを追加する（確認: `cargo test acceptance::`）
- [ ] 1.2 `src/orchestration/acceptance.rs` の所見抽出と結果判定の現行フローを再現する統合テストを追加する（確認: `cargo test orchestration::acceptance`）

## 2. リファクタリング
- [ ] 2.1 受け入れ判定で使用する出力ソースを単一経路へ統合する（確認: 追加したテストが全て成功）
- [ ] 2.2 TODOで示されている出力経路の曖昧さを解消し、判定と所見の整合を保証する（確認: FAIL時の `findings` がテスト期待値と一致）

## 3. 回帰確認
- [ ] 3.1 `cargo test` を実行し、既存テストを含めてグリーンであることを確認する
- [ ] 3.2 `cargo run -- run --dry-run` を実行し、CLI挙動に変更がないことを確認する
