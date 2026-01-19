## 1. 実装
- [ ] 1.1 Accepting状態の表示仕様を整理し、既存のProcessingスピナー表示ロジックとの差分を確認する（確認方法: `src/tui/render.rs` の行表示処理を参照してAcceptingの扱いを把握する）
- [ ] 1.2 Accepting状態でもスピナー表示を行うよう描画ロジックを更新する（確認方法: `src/tui/render.rs` でAccepting時にスピナー文字が描画されることを確認する）
- [ ] 1.3 Acceptingへの状態遷移とログ表示が期待通りに維持されていることを確認する（確認方法: `src/tui/state/events/completion.rs` 等でAccepting遷移が維持されていることを確認する）

## 2. テスト
- [ ] 2.1 既存テスト/スナップショットがある場合は更新し、最低限 `cargo test` で変更が問題ないことを確認する（確認方法: `cargo test`）
