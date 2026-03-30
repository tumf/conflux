## 1. 現状確認（ローカル重複フィールドの洗い出し）
- [ ] 1.1 `src/orchestrator.rs` の `Orchestrator` struct に `apply_counts`, `changes_processed`, `iteration`, `current_change_id`, `completed_change_ids`, `stalled_change_ids`, `skipped_change_ids` フィールドが**存在しない**ことを確認する。もし存在すれば 2. で削除、存在しなければ 2. をスキップする。
- [ ] 1.2 `src/tui/orchestrator.rs` の `run_orchestrator` 関数内に `apply_counts`, `pending_changes`, `changes_processed`, `total_changes` のローカル変数が**存在しない**ことを確認する。もし存在すれば 3. で削除、存在しなければ 3. をスキップする。
- [ ] 1.3 `src/orchestrator.rs` L54 の `max_iterations: u32` フィールドが `OrchestratorState` と重複していないか確認する。`OrchestratorState::max_iterations()` メソッドがあるなら、`Orchestrator.max_iterations` フィールドは `OrchestratorState` 初期化時にのみ使用され、ループ中は `shared_state` 経由で参照すべきである。

## 2. Orchestrator struct からの残留フィールド削除（該当する場合）
- [ ] 2.1 上記 1.1 で残留フィールドがあった場合、`shared_state.read().await` 経由のアクセスに置き換えてフィールドを削除する。
- [ ] 2.2 `cargo test` が通ることを確認する。

## 3. TUI orchestrator からのローカル変数削除（該当する場合）
- [ ] 3.1 上記 1.2 でローカル変数があった場合、`shared_state.read().await` 経由のアクセスに置き換えて変数を削除する。
- [ ] 3.2 `cargo test` が通ることを確認する。

## 4. max_iterations フィールドの統一（該当する場合）
- [ ] 4.1 上記 1.3 で重複が確認された場合、`Orchestrator` 内の `max_iterations` フィールドを削除し、`shared_state.read().await.max_iterations()` から取得するよう変更する。
- [ ] 4.2 `cargo test` と `cargo clippy -- -D warnings` が通ることを確認する。
