## Implementation Tasks

- [ ] 1. `src/orchestration/state.rs` の `ChangeRuntimeState` に `display_color() -> ratatui::style::Color` メソッドを追加する。`display_status()` の返す文字列に対し `src/tui/type_impls.rs` L32-45 と同一の Color マッピングを実装する（verification: `cargo test --lib orchestration::state` でメソッドの単体テスト通過）
- [ ] 2. `src/orchestration/state.rs` の `ChangeRuntimeState` に `error_message() -> Option<&str>` メソッドを追加する。`TerminalState::Error(msg)` のとき `Some(msg.as_str())` を返す（verification: テスト追加・通過）
- [ ] 3. `src/tui/state.rs` の `ChangeState` struct (L123付近) に `display_status_cache: String`（初期値 `"not queued".to_string()`）と `display_color_cache: Color`（初期値 `Color::DarkGray`）と `error_message_cache: Option<String>`（初期値 `None`）を追加する（verification: `cargo test` 通過）
- [ ] 4. `src/tui/state.rs` の `apply_display_statuses_from_reducer()` (L970付近) で cache フィールドを同時更新するよう修正する（verification: `cargo test` 通過）
- [ ] 5. `src/tui/render.rs` 内の `QueueStatus` 参照（59箇所）を `display_status_cache` / `display_color_cache` の文字列比較に置き換える。`get_checkbox_display()` のシグネチャを `(display_status: &str, is_selected: bool)` に変更する（verification: `rg "QueueStatus" src/tui/render.rs` が test 以外ゼロ、`cargo test` 通過）
- [ ] 6. `src/tui/key_handlers.rs` の `QueueStatus` 参照（2箇所: L191, L220）を cache 参照に置き換え import を削除する（verification: `rg "QueueStatus" src/tui/key_handlers.rs` がゼロ）
- [ ] 7. `src/tui/state.rs` のイベントハンドラ内の `queue_status = QueueStatus::XYZ` 直接代入（33箇所: L846, L881, L951, L997, L1043, L1119, L1175, L1230, L1604, L1613, L1625, L1655, L1730, L1758, L1780, L1814, L1845, L1876, L1913, L1925, L1978, L1992, L2015, L2053, L2073, L2084, L2099, L2117, L2127, L2154, L2174, L2186, L2335）を `display_status_cache`/`display_color_cache`/`error_message_cache` 更新に置き換える（verification: `rg "queue_status\s*=" src/tui/state.rs` がゼロ、`cargo test` 通過）
- [ ] 8. `src/tui/state.rs` の読み取り参照（`matches!(change.queue_status, ...)` パターン）を `display_status_cache` 文字列比較に置き換える（verification: `rg "queue_status" src/tui/state.rs` がゼロ）
- [ ] 9. `src/tui/state.rs` の `ChangeState` struct から `pub queue_status: QueueStatus` フィールドを削除する。コンパイルエラーがあれば修正する（verification: `cargo build` 成功）
- [ ] 10. `src/tui/types.rs` の `QueueStatus` enum 定義を削除する。`src/tui/type_impls.rs` の `impl QueueStatus` ブロックとテストを削除する（verification: `rg "QueueStatus" src/` がゼロ、`cargo test` 通過）
- [ ] 11. `cargo clippy -- -D warnings` と `cargo fmt --check` がクリアであることを確認する（verification: warning 0）
