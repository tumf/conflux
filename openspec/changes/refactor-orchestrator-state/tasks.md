## 1. 共有ステートの統合
- [ ] 1.1 `src/orchestration/state.rs` にExecutionEventの更新関数を追加し、状態更新の単一経路を実装する（更新関数が追加されたことを確認）。
- [ ] 1.2 `src/web/state.rs` のDTOを `OrchestratorStateSnapshot` にリネームし、共有ステートからスナップショット生成する（型名変更と生成関数の参照箇所を確認）。
- [ ] 1.3 TUIのChangeState生成を共有ステート由来に切り替える（`src/tui/state/mod.rs` で参照元が変更されていることを確認）。

## 2. 動作確認
- [ ] 2.1 `cargo test` を実行して既存テストが通ることを確認する（`cargo test` の成功ログを確認）。
