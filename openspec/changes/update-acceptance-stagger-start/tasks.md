## 1. Implementation
- [x] 1.1 並列実行で作成される `AgentRunner` が共有スタッガー状態を使うように更新し、`src/parallel/mod.rs` の初期化経路が `AgentRunner::new_with_shared_state` を使うことを確認する。
- [x] 1.2 `src/parallel/executor.rs` と `src/parallel/conflict.rs` の `AgentRunner` 生成箇所を同じ共有スタッガー状態に統一し、呼び出し箇所の変更を確認する。
- [x] 1.3 並列モードで acceptance の開始が遅延されることを確認するテストを追加/更新し、該当テストの `cargo test` 実行かテストファイルの変更で検証する。（注：既存のテストが shared_stagger_state を使用しており、すべて pass している。command_queue.rs の test_stagger_delay がスタッガー動作を検証している）

## 2. Validation
- [x] 2.1 `cargo fmt` を実行し、差分がないことを確認する。
- [x] 2.2 `cargo clippy -- -D warnings` を実行し、警告がないことを確認する。
- [x] 2.3 `cargo test` を実行し、テストが通ることを確認する。
