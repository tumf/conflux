## Implementation Tasks

- [x] 1. `src/web/state.rs` の既存テスト（`cargo test --lib web::state`）が通ることを確認する（verification: `cargo test --lib web::state` が全パス）
- [x] 2. `src/web/state.rs` の `apply_execution_event()` 内で `change.queue_status = Some("applying".to_string())` 等のステータス直接代入をしている 14 箇所（L462, L476, L485, L509, L515, L526, L533, L572, L582, L589, L599, L684, L690、および L613 付近の `MergeDeferred` 内）を削除する。ログ追加（`log_entries.push(...)`）、progress 更新（`iteration_number`, `progress`, `status`）、worktree 更新などの非ステータス処理はそのまま残す（verification: `rg "queue_status\s*=\s*Some" src/web/state.rs` が test コード以外 0 件）
- [x] 3. `apply_execution_event()` 終了後に `from_changes_with_shared_state()` 相当の Reducer 導出で `queue_status` を更新するパスが呼ばれることを確認する。既存の WebSocket broadcast パス（`update_with_mode` / `broadcast`）が `from_changes_with_shared_state()` を使っているかを確認し、使っていない場合は `apply_execution_event()` の末尾で Reducer から `queue_status` を再導出する処理を追加する（verification: `cargo test --lib web::state` 全パス）
- [x] 4. `ChangesRefreshed` イベントハンドラ（L634 付近）内の `queue_status` 保存ロジック（L647-651）で、Reducer 導出パスと重複している書き換えがあれば削除する（verification: `rg "queue_status\s*=\s*Some" src/web/state.rs` で test コード以外の行数を確認）
- [x] 5. `src/web/state.rs` のテスト内で `queue_status` の期待値を直接設定しているテスト（L967-969 等）の期待値が Reducer の `display_status()` と一致することを確認し、必要ならテストを修正する（verification: `cargo test --lib web::state` 全パス）
- [x] 6. `cargo test` 全体が通ることを確認する（verification: `cargo test` が exit code 0）
- [x] 7. `cargo clippy -- -D warnings` がクリアであることを確認する（verification: clippy warning 0 件）
