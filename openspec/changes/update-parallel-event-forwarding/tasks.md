## 1. 実装
- [ ] 1.1 `src/tui/orchestrator.rs` のforward_handleで `forward_tx.send` を `shared_state.write()` より先に実行し、TUIイベントがロック待ちで遅延しないようにする（検証: `src/tui/orchestrator.rs` のイベント転送順序を確認）
- [ ] 1.2 `cargo test` を実行して既存テストが通ることを確認する（検証: `cargo test`）
