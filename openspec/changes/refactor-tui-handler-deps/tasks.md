## 1. 実装
- [x] 1.1 runner 由来の worktree/terminal ヘルパーを専用モジュールに移す（検証: src/tui/runner.rs のヘルパーが新モジュールに移動していることを確認）
- [x] 1.2 command_handlers/key_handlers が新モジュール経由でヘルパーを参照する（検証: runner への直接参照が消えていることを確認）
- [x] 1.3 既存の公開 API と挙動を維持する（検証: 既存の公開関数・型のエクスポートが保持されていることを確認）
- [x] 1.4 既存の挙動維持を確認するため `cargo test` を実行する（検証: `cargo test` が成功）

## Acceptance #1 Failure Follow-up
- [x] 作業ツリーをクリーンにする（未コミット: Cargo.lock, openspec/changes/refactor-tui-handler-deps/tasks.md, src/tui/command_handlers.rs, src/tui/key_handlers.rs, src/tui/mod.rs, src/tui/runner.rs, src/tui/terminal_helpers.rs, src/tui/worktree_helpers.rs）
- [x] `openspec/changes/refactor-tui-handler-deps/specs/tui-architecture/spec.md` の要求に合わせて TUI モジュール構成を修正する（`src/tui/mod.rs` が `terminal_helpers`/`worktree_helpers` と `state` モジュールを参照しており、`terminal.rs`/`worktrees.rs`/`state.rs` の要件に一致していない）
- [x] `src/tui/orchestrator.rs` のレガシー未使用コード（ArchiveContext/ArchiveResult/archive_single_change/archive_all_complete_changes）を削除または実際のフローに統合する

## Acceptance #2 Failure Follow-up
- [x] `src/tui/state/mod.rs` と `src/tui/state/*` を `src/tui/state.rs` に整理し、`src/tui/*.rs` 配下のモジュール構成に統一する（spec: `openspec/changes/refactor-tui-handler-deps/specs/tui-architecture/spec.md:9-15`）
- [x] `src/tui/types.rs` の `impl QueueStatus`/`impl WorktreeInfo` と `#[cfg(test)]` を別モジュールへ移し、型定義のみ残す（spec: `openspec/changes/refactor-tui-handler-deps/specs/tui-architecture/spec.md:13`）
- [ ] `src/tui/state.rs` の `update_changes` と `from_change` が `Change` から進捗を取得しているため、shared state（`OrchestratorState`）を進捗/実行メタデータのソースにする（spec: `openspec/changes/refactor-tui-handler-deps/specs/tui-architecture/spec.md:24-28`）
- [x] `src/tui/state.rs` の `request_worktree_delete`/`confirm_worktree_delete`/`cancel_worktree_delete`/`should_refresh` が本フローで未使用のため、呼び出し経路へ統合するか削除する

## 2. テストと検証
- [x] `cargo build` でコンパイルが通ることを確認
- [x] `cargo test` で既存のテストが通ることを確認
- [x] TUI モジュール構成が仕様（`src/tui/*.rs`形式）に準拠していることを確認
