## 1. 実装
- [x] 1.1 `ProcessingError` 受信時に AppMode を Error にしないよう調整し、失敗は change 単位の `QueueStatus::Error` とログに反映する（検証: `src/tui/state/events/processing.rs` にユニットテスト `test_processing_error_keeps_app_mode` を追加し、`cargo test` で通過）
- [x] 1.2 致命的な `OrchestratorEvent::Error` では従来通り Error モードに遷移することを明文化し、関連テストを更新する（検証: `src/tui/state/events/messages.rs` のテスト追加、`cargo test` で通過）
- [x] 1.3 `resolve` 実行可否判定を `AppState` に集約し、Changes パネルの `M: resolve` 表示条件と `resolve_merge()` の条件を一致させる（検証: `src/tui/render.rs` と `src/tui/state/mod.rs` のテストで「表示される時は必ず実行可能」を確認）
- [x] 1.4 既存の TUI 関連テストが通ることを確認する（検証: `cargo test tui` または `cargo test`）

## Acceptance #1 Failure Follow-up
- [ ] Git の作業ツリーをクリーンにする（未コミット変更: `openspec/changes/update-tui-error-mode-continuation/tasks.md`, `src/tui/render.rs`, `src/tui/state/events/messages.rs`, `src/tui/state/events/processing.rs`, `src/tui/state/mod.rs`）
