## Implementation Tasks

- [x] 1. 特性化テスト: `cargo test --lib tui::state` を実行し全テスト通過を記録する（verification: テスト結果ログ）
- [x] 2. `src/tui/state/event_handlers/mod.rs` を作成し、`handle_orchestrator_event` ディスパッチャを移動する（verification: `cargo build` 成功）
- [x] 3. 実行開始系ハンドラ (`handle_processing_started`, `handle_apply_started` 等) を `event_handlers/processing.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 4. 完了系ハンドラ (`handle_processing_completed`, `handle_all_completed` 等) を `event_handlers/completion.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 5. エラー系ハンドラ (`handle_processing_error`, `handle_apply_failed` 等) を `event_handlers/errors.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 6. 出力系ハンドラ (`handle_apply_output`, `handle_archive_output` 等) を `event_handlers/output.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 7. リフレッシュ系ハンドラ (`handle_changes_refreshed`, `handle_worktrees_refreshed`) を `event_handlers/refresh.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 8. テストを適切なサブモジュール内 `#[cfg(test)]` に配置する（verification: `cargo test --lib tui::state` 全通過）
- [x] 9. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- ガード関数群 (`validate_*`) の分離は別 proposal で扱う
- `ChangeState` のサブモジュール化も別 proposal とする

## Acceptance #1 Failure Follow-up

- [x] `handle_apply_started` / `handle_archive_started` / `handle_acceptance_started` / `handle_resolve_started` / `handle_analysis_started` を `src/tui/state/event_handlers/processing.rs` へ移動し、開始系ハンドラの責務を spec と tasks に一致させる
- [x] 完了系ハンドラのみを `src/tui/state/event_handlers/completion.rs` に残し、完了系と開始系の分類に対するテストを更新して `cargo fmt --check && cargo clippy -- -D warnings && cargo test` を再実行する

## Acceptance #2 Failure Follow-up

- [x] `handle_processing_completed` / `handle_all_completed` を `src/tui/state/event_handlers/completion.rs` へ移動し、`processing.rs` には開始系ハンドラのみを残す
- [x] `handle_processing_error` / `handle_change_stop_failed` を `src/tui/state/event_handlers/errors.rs` へ移動し、`completion.rs` からエラー系責務を除去する
- [x] `handle_dependency_blocked` / `handle_dependency_resolved` の配置先を spec に沿って整理し、必要なら proposal/spec/tasks を実装実態に合わせて更新したうえで、各サブモジュールの責務に対するテストと `cargo fmt --check && cargo clippy -- -D warnings && cargo test` を再実行する
