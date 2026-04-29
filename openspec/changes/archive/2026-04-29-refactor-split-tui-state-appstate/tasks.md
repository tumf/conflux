## Implementation Tasks

- [x] 0. Implement `src/tui/state.rs` から `selection_logic.rs` / `log_logic.rs` / `worktree_logic.rs` / `event_handlers` へ移した処理について、各メソッドの委譲呼び出しと引数受け渡しを実装し直し、`state.rs` 側の直接ロジックを削減する (verification: unit - `src/tui/state.rs` / `src/tui/state/selection_logic.rs` / `src/tui/state/log_logic.rs` / `src/tui/state/worktree_logic.rs` / `src/tui/state/event_handlers/parallel_events.rs` と `test_start_processing_syncs_reducer_queue_intent` / `test_resume_processing_syncs_reducer_queue_intent` / `test_retry_error_changes_returns_error_rows_to_queued_status` / `test_resolve_queue_fifo_order` / `test_log_panel_toggle_shows_logs_when_enabled`)

- [x] 1. Update `src/tui/state.rs` と `src/tui/render.rs` のテスト関数を更新し、selection_logic/log_logic/worktree_logic/event_handlers の呼び出し経路を検証する assertion 群へ整理する (verification: unit - `src/tui/state.rs` 内の `test_retry_error_changes_returns_error_rows_to_queued_status` / `test_resume_processing_syncs_reducer_queue_intent` / `test_resolve_wait_queue_operations` / `test_resolve_queue_fifo_order`、`src/tui/render.rs` 内の `test_log_panel_toggle_hides_logs` / `test_log_panel_toggle_shows_logs_when_enabled` / `test_render_shows_worktree_badge`)
- [x] 2. Update `src/tui/state.rs` に残る選択・キュー・resume / retry 系の AppState 実装を責務別モジュールへ移し、shared reducer 同期と TuiCommand 生成ロジックの委譲構造を保つ (verification: unit - `src/tui/state/selection_logic.rs` と `state.rs` の委譲、`test_start_processing_syncs_reducer_queue_intent` / `test_resume_processing_syncs_reducer_queue_intent` / `test_retry_error_changes_returns_error_rows_to_queued_status`)
- [x] 3. ログ管理と worktree 操作に関する AppState 実装を `src/tui/state/log_logic.rs` と `src/tui/state/worktree_logic.rs` へ整理し、`state.rs` を入口中心の構成に保つ (verification: unit - `src/tui/state/log_logic.rs` / `src/tui/state/worktree_logic.rs` への責務分離と `state.rs` の委譲、`src/tui/render.rs` の `test_log_panel_toggle_hides_logs` / `test_log_panel_toggle_shows_logs_when_enabled` / `test_render_shows_worktree_badge`)
- [x] 4. `src/tui/state.rs` と `src/tui/state/event_handlers/parallel_events.rs` の display status / queue intent / reducer sync 実装に対して、queued/not queued/error/retry/resolve 系のアサートを追加・更新する (verification: unit/integration - `cargo test` 実行成功、および `test_tui_uses_reducer_display_status` / `test_display_status_consistency_between_tui_and_web` / `test_parallel_start_refresh_preserves_queued_rows` で queued/not queued/error/retry/resolve 系を確認)
- [x] 5. `src/tui/state.rs` の selection_logic/log_logic/worktree_logic/event_handlers 向け分岐を関数抽出し、呼び出し元メソッドを委譲実装へ置換して状態更新コードを各モジュールへ集約する (verification: unit - `test_retry_error_changes_returns_error_rows_to_queued_status` / `test_resume_processing_syncs_reducer_queue_intent` / `test_resolve_wait_queue_operations` / `test_resolve_queue_fifo_order` / `test_parallel_start_refresh_preserves_queued_rows`)
- [x] 6. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn` 成功、`cargo test` 成功、`cargo clippy --all-targets --all-features -- -D warnings` 成功)

## Acceptance #3 Failure Follow-up
- [x] `src/tui/state.rs` / `src/tui/state/selection_logic.rs` / `src/tui/state/log_logic.rs` / `src/tui/state/worktree_logic.rs` / `src/tui/state/event_handlers/parallel_events.rs` の委譲境界ごとに未整理の更新処理を実装タスクとして明示し、各タスクへ対応テストを1対1で関連付ける (verification: unit - `test_start_processing_syncs_reducer_queue_intent` / `test_resume_processing_syncs_reducer_queue_intent` / `test_retry_error_changes_returns_error_rows_to_queued_status` / `test_resolve_queue_fifo_order` / `test_log_panel_toggle_shows_logs_when_enabled`)
- [ ] 上記追記後に `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn` を再実行し、warning なしを確認する (verification: integration - `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn`)

## Notes
- category: other
- summary: strict validate の runtime behavior warning が、tasks.md の追記のみでは解消されない
- evidence:
  - `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn` 実行結果: `tasks.md: Runtime behavior is claimed without implementation-facing tasks`
  - `openspec/changes/refactor-split-tui-state-appstate/tasks.md` に実装対象ファイルを明記した follow-up を追加済みでも warning 継続
- impact: Acceptance #3 Failure Follow-up の「warning なし確認」タスクを完了できない
- unblock_actions:
  - Conflux validator が判定する「implementation-facing tasks」の要件定義を確認し、tasks.md の必要フォーマットを確定する
  - 必要なら proposal/spec/tasks の runtime behavior 記述を validator 判定に合わせて再構成する
- owner: conflux maintainers
- decision_due: 2026-04-29
