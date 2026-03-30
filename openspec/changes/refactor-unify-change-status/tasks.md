## 1. Reducer に display ヘルパーを追加する
- [ ] 1.1 `src/orchestration/state.rs` の `ChangeRuntimeState` に `display_color()` メソッドを追加する。`ratatui::style::Color` を返す。マッピングは `src/tui/type_impls.rs` L32-45 の `QueueStatus::color()` と同一にする。`display_status()` の返す文字列に対して match する実装でよい（例: `"applying" => Color::Cyan`）。`Cargo.toml` に `ratatui` が既に依存にあるので追加は不要。
- [ ] 1.2 `src/orchestration/state.rs` の `ChangeRuntimeState` に `is_active_display()` メソッドを追加する。`ActivityState::Idle` 以外なら true を返す。これは既存の `is_active()` と同じ動作だが public 名を明確化する。
- [ ] 1.3 `src/orchestration/state.rs` の `ChangeRuntimeState` に `error_message()` メソッドを追加する。`TerminalState::Error(msg)` のとき `Some(&msg)` を返し、それ以外は `None` を返す。
- [ ] 1.4 `src/orchestration/state.rs` の `OrchestratorState` に `display_status_for(&self, change_id: &str) -> Option<&'static str>` メソッドを追加する。内部で `change_runtime.get(change_id).map(|rt| rt.display_status())` を返す。
- [ ] 1.5 `src/orchestration/state.rs` の `OrchestratorState` に `display_color_for(&self, change_id: &str) -> Option<Color>` メソッドを追加する。
- [ ] 1.6 `src/orchestration/state.rs` の `OrchestratorState` に `error_message_for(&self, change_id: &str) -> Option<&str>` メソッドを追加する。
- [ ] 1.7 上記メソッドの単体テストを `src/orchestration/state.rs` の `#[cfg(test)]` モジュール内に追加する。`display_color()` が `display_status()` の各返り値に対し正しい Color を返すことを検証する。
- [ ] 1.8 `cargo test` と `cargo clippy -- -D warnings` が通ることを確認する。

## 2. TUI `ChangeState` に Reducer 参照用フィールドを追加する（並行稼働期間）
- [ ] 2.1 `src/tui/state.rs` L123 の `ChangeState` struct に `display_status_cache: String` フィールドを追加する。初期値は `"not queued".to_string()` とする（L248 の `from_change` メソッド内）。
- [ ] 2.2 `src/tui/state.rs` L123 の `ChangeState` struct に `display_color_cache: Color` フィールドを追加する。初期値は `Color::DarkGray` とする。
- [ ] 2.3 `src/tui/state.rs` L123 の `ChangeState` struct に `error_message_cache: Option<String>` フィールドを追加する。初期値は `None` とする。
- [ ] 2.4 `src/tui/state.rs` の `apply_display_statuses_from_reducer()` メソッド（L970付近）で、`change.queue_status = new_status;` の直後に `change.display_status_cache = status_str.to_string();` と `change.display_color_cache = ...` を追加して cache を同時更新する。
- [ ] 2.5 `cargo test` が通ることを確認する。

## 3. TUI render.rs を cache フィールド参照に切り替える
- [ ] 3.1 `src/tui/render.rs` L129 の `get_checkbox_display` 関数のシグネチャを `(queue_status: &QueueStatus, is_selected: bool)` から `(display_status: &str, is_selected: bool)` に変更する。内部の match を文字列比較に変更する（`"archived" | "merged" => ...`）。
- [ ] 3.2 `src/tui/render.rs` 内の `get_checkbox_display` 呼び出し箇所（L420, L708 付近）を `change.display_status_cache.as_str()` に変更する。
- [ ] 3.3 `src/tui/render.rs` 内で `change.queue_status.display()` を参照している箇所を `change.display_status_cache.as_str()` に変更する。
- [ ] 3.4 `src/tui/render.rs` 内で `change.queue_status.color()` を参照している箇所を `change.display_color_cache` に変更する。
- [ ] 3.5 `src/tui/render.rs` 内で `matches!(change.queue_status, QueueStatus::XYZ)` パターンマッチをしている箇所（L420, L427, L606, L607, L616, L708, L715, L774, L782, L790, L860, L964, L965, L974, L1413）を `change.display_status_cache == "xyz"` の文字列比較に変更する。
- [ ] 3.6 `src/tui/render.rs` のテスト（L1818〜）で `QueueStatus::` を使っている箇所を新しいインターフェースに合わせて更新する。
- [ ] 3.7 `cargo test` と `cargo clippy -- -D warnings` が通ることを確認する。

## 4. TUI key_handlers.rs を cache 参照に切り替える
- [ ] 4.1 `src/tui/key_handlers.rs` L220 の `matches!(change.queue_status, QueueStatus::MergeWait)` を `change.display_status_cache == "merge wait"` に変更する。
- [ ] 4.2 `src/tui/key_handlers.rs` から `use super::types::QueueStatus;` を削除する（L191）。
- [ ] 4.3 `cargo test` が通ることを確認する。

## 5. TUI state.rs のイベントハンドラ内の queue_status 書き込みを Reducer 委譲に切り替える
- [ ] 5.1 `src/tui/state.rs` の `handle_processing_started()` (L1601) で `change.queue_status = QueueStatus::Applying` を削除する。代わりに `change.display_status_cache = "applying".to_string(); change.display_color_cache = Color::Cyan;` を設定する。
- [ ] 5.2 同様に `handle_processing_completed()` (L1611): `QueueStatus::Archiving` → `display_status_cache = "archiving"`, `display_color_cache = Color::Magenta` に変更する。
- [ ] 5.3 同様に `handle_processing_error()` (L1623): `QueueStatus::Error(...)` → `display_status_cache = "error"`, `display_color_cache = Color::Red`, `error_message_cache = Some(error.clone())` に変更する。
- [ ] 5.4 同様に以下のハンドラ内の `queue_status = QueueStatus::XYZ` 代入をすべて `display_status_cache`/`display_color_cache` 代入に変更する（対象行: L1604, L1613, L1625, L1655, L1730, L1758, L1780, L1814, L1845, L1876, L1913, L1925, L1978, L1992, L2015, L2053, L2073, L2084, L2099, L2117, L2127, L2154, L2174, L2186, L2335）。
- [ ] 5.5 `cargo test` が通ることを確認する。

## 6. TUI state.rs の読み取り参照を cache に切り替える
- [ ] 6.1 `src/tui/state.rs` 内の `matches!(change.queue_status, QueueStatus::XYZ)` パターン（L478, L649, L717, L724, L785, L837, L950, L1042, L1087, L1098, L1118, L1163, L1219, L1229, L1652, L1683, L1717, L2007, L2042, L2185, L2414, L2415）を `change.display_status_cache == "xyz"` 文字列比較に変更する。
- [ ] 6.2 `src/tui/state.rs` の `apply_remote_status()` 関数（L28-99）を削除する。remote status 処理は `display_status_cache` 直接更新に置き換える（L1571-1581 の呼び出し元を修正）。
- [ ] 6.3 `src/tui/state.rs` の `apply_display_statuses_from_reducer()` メソッド（L970-999）から `QueueStatus` 変換ロジックを削除し、`display_status_cache` と `display_color_cache` のみを更新するよう簡素化する。
- [ ] 6.4 `cargo test` が通ることを確認する。

## 7. ChangeState から queue_status フィールドを削除する
- [ ] 7.1 `src/tui/state.rs` L131 の `pub queue_status: QueueStatus` フィールドを削除する。
- [ ] 7.2 `src/tui/state.rs` L248 の `from_change` メソッドから `queue_status: QueueStatus::NotQueued` 初期化を削除する。
- [ ] 7.3 コンパイルエラーが出る箇所をすべて修正する（残っている `change.queue_status` 参照を `change.display_status_cache` に変更する）。
- [ ] 7.4 `cargo test` が通ることを確認する。

## 8. QueueStatus enum を削除する
- [ ] 8.1 `src/tui/types.rs` L54-80 の `QueueStatus` enum 定義を削除する。
- [ ] 8.2 `src/tui/type_impls.rs` L12-65 の `impl QueueStatus` ブロックを削除する。
- [ ] 8.3 `src/tui/type_impls.rs` のテスト（L124-167）で `QueueStatus` を使っている箇所を削除または `ChangeRuntimeState::display_status()` / `display_color()` のテストに置き換える。
- [ ] 8.4 ファイル全体から `use.*QueueStatus` の残留 import を削除する（`rg "QueueStatus" src/` で確認）。
- [ ] 8.5 `cargo test` と `cargo clippy -- -D warnings` と `cargo fmt --check` が通ることを確認する。

## 9. Web state のステータス導出を Reducer に統一する
- [ ] 9.1 `src/web/state.rs` の `apply_execution_event()` メソッド（L447〜）内で `change.queue_status = Some("...".to_string())` を設定している全 match arm（L462, L476, L485, L509, L515, L526, L533, L572, L582, L589, L599, L613, L684, L690）からステータス書き込みを削除する。ログ・progress・worktree 等の更新処理は残す。
- [ ] 9.2 `src/web/state.rs` の `from_changes_with_shared_state()` メソッド（L130付近）で、Reducer の `display_status()` から `queue_status` を導出するロジックが既にある（L144-148）ことを確認する。9.1 で削除した書き込みが不要であることを検証する。
- [ ] 9.3 `src/web/state.rs` の `ChangesRefreshed` イベントハンドラ（L634付近）内の `queue_status` 保存ロジック（L647-651）を簡素化し、Reducer からの導出に統一する。
- [ ] 9.4 `cargo test` と `cargo clippy -- -D warnings` が通ることを確認する。
