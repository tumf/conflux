## Implementation Tasks

- [x] 1. selection / queue / retry / resume / log / worktree の現在挙動を固定する characterization test を先に追加または更新する (verification: unit - `src/tui/state.rs` 内の `test_retry_error_changes_returns_error_rows_to_queued_status` / `test_resume_processing_syncs_reducer_queue_intent` / `test_resolve_wait_queue_operations` / `test_resolve_queue_fifo_order` で selection/queue/retry/resume の既存意味論を固定、`src/tui/render.rs` 内の `test_log_panel_toggle_hides_logs` / `test_log_panel_toggle_shows_logs_when_enabled` / `test_render_shows_worktree_badge` で log/worktree 表示挙動を固定済み)
- [x] 2. `src/tui/state.rs` に残る選択・キュー・resume / retry 系の AppState 実装を責務別モジュールへ移し、shared reducer 同期と TuiCommand 生成を維持する (verification: unit - `src/tui/state/selection_logic.rs` と `state.rs` の委譲、`test_start_processing_syncs_reducer_queue_intent` / `test_resume_processing_syncs_reducer_queue_intent` / `test_retry_error_changes_returns_error_rows_to_queued_status` で reducer-facing 挙動維持を確認)
- [x] 3. ログ管理と worktree 操作に関する AppState 実装を既存サブモジュールまたは新規責務モジュールへ整理し、`state.rs` を入口中心にする (verification: unit - `src/tui/state/log_logic.rs` / `src/tui/state/worktree_logic.rs` への責務分離と `state.rs` の委譲、および `src/tui/render.rs` の `test_log_panel_toggle_hides_logs` / `test_log_panel_toggle_shows_logs_when_enabled` / `test_render_shows_worktree_badge` で log/worktree 関連挙動維持を確認)
- [x] 4. display status / queue intent / reducer sync の回帰がないことを TUI 関連テストで確認する (verification: unit/integration - `cargo test` 実行成功、および `test_tui_uses_reducer_display_status` / `test_display_status_consistency_between_tui_and_web` / `test_parallel_start_refresh_preserves_queued_rows` で queued/not queued/error/retry/resolve 系を確認)
- [x] 5. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn` 成功、`cargo test` 成功、`cargo clippy --all-targets --all-features -- -D warnings` 成功)

## Future Work

- `src/tui/render.rs` や `src/tui/orchestrator.rs` も同じ責務単位で段階的に整理する
- TUI 状態変更のテストサポート DSL を追加して、今後のリファクタリングをさらに安全にする

## Acceptance #1 Failure Follow-up
- [x] openspec/changes/refactor-split-tui-state-appstate/tasks.md:3 の verification は selection/queue/retry/resume/log/worktree を固定済みと主張しているが、列挙された証拠テスト（src/tui/state.rs:2477, 2714, 3537, 3616）は retry/resume/queue 寄りで、log や worktree の固定を直接示していない。Task 1 の verification 記述を実際の根拠テストに合わせて修正するか、log/worktree を固定する具体テストを明示する必要がある。
- [x] openspec/changes/refactor-split-tui-state-appstate/tasks.md:5 の verification はログ管理と worktree 操作の挙動維持を主張しているが、根拠として挙げている src/tui/state.rs:3515 と 3537 は merge/resolve wait の queue 操作テストであり、log/worktree の証拠として不適切。Task 3 の verification を log/worktree 関連の実テストへ差し替えるか、記述を実際の検証範囲に縮小する必要がある。
