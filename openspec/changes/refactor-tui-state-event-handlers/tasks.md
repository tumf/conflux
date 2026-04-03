## Implementation Tasks

- [ ] 1. 特性化テスト: `cargo test --lib tui::state` を実行し全テスト通過を記録する（verification: テスト結果ログ）
- [ ] 2. `src/tui/state/event_handlers/mod.rs` を作成し、`handle_orchestrator_event` ディスパッチャを移動する（verification: `cargo build` 成功）
- [ ] 3. 実行開始系ハンドラ (`handle_processing_started`, `handle_apply_started` 等) を `event_handlers/processing.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 4. 完了系ハンドラ (`handle_processing_completed`, `handle_all_completed` 等) を `event_handlers/completion.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 5. エラー系ハンドラ (`handle_processing_error`, `handle_apply_failed` 等) を `event_handlers/errors.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 6. 出力系ハンドラ (`handle_apply_output`, `handle_archive_output` 等) を `event_handlers/output.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 7. リフレッシュ系ハンドラ (`handle_changes_refreshed`, `handle_worktrees_refreshed`) を `event_handlers/refresh.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 8. テストを適切なサブモジュール内 `#[cfg(test)]` に配置する（verification: `cargo test` 全通過）
- [ ] 9. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- ガード関数群 (`validate_*`) の分離は別 proposal で扱う
- `ChangeState` のサブモジュール化も別 proposal とする
