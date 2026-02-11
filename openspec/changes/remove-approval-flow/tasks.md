## 1. Implementation
- [x] 1.1 承認データモデルと approved ファイル処理を削除する
  - 対象: `src/approval.rs`, `src/openspec.rs`, `src/orchestrator.rs`, `src/web/state.rs`
  - 完了確認: `rg "approved|is_approved" src` で承認フローの参照が残っていないこと
- [x] 1.2 CLI/TUI の承認操作と初期選択の挙動を更新する
  - 対象: `src/cli.rs`, `src/main.rs`, `src/tui/key_handlers.rs`, `src/tui/state.rs`, `src/tui/render.rs`
  - 完了確認: `cflx --help` に approve が出ないこと、`cflx tui --help` に @ が出ないこと
- [x] 1.3 Web 承認 API と UI を削除する
  - 対象: `src/web/api.rs`, `src/web/mod.rs`, `src/web/state.rs`, `src/web/*`
  - 完了確認: `rg "approve|unapprove" src/web` で API/ボタンが残っていないこと
- [x] 1.4 Git の approved 除外自動追加を削除する
  - 対象: `src/vcs/git/mod.rs`, `src/vcs/git/commands/basic.rs` と関連テスト
  - 完了確認: `rg "openspec/changes/\*/approved" src` で参照が残っていないこと
- [x] 1.5 テストを更新し、承認前提のテストを置き換える
  - 対象: `src/approval.rs` のテスト群、TUI/CLI/Web の承認関連テスト
  - 完了確認: `cargo test` が成功すること

## Acceptance #1 Failure Follow-up
- [x] `git status --porcelain` が空になるように作業ツリーをクリーンにする（未コミット変更: `docs/openapi.yaml`, `openspec/changes/remove-approval-flow/tasks.md`, `src/analyzer.rs`, `src/approval.rs`, `src/bin/openapi_gen.rs`, `src/cli.rs`, `src/config/mod.rs`, `src/hooks.rs`, `src/lib.rs`, `src/main.rs`, `src/openspec.rs`, `src/orchestration/archive.rs`, `src/orchestration/hooks.rs`, `src/orchestration/selection.rs`, `src/orchestrator.rs`, `src/parallel/conflict.rs`, `src/parallel_run_service.rs`, `src/progress.rs`, `src/serial_run_service.rs`, `src/templates.rs`, `src/tui/command_handlers.rs`, `src/tui/events.rs`, `src/tui/key_handlers.rs`, `src/tui/log_deduplicator.rs`, `src/tui/render.rs`, `src/tui/state.rs`, `src/vcs/git/commands/basic.rs`, `src/vcs/git/commands/mod.rs`, `src/vcs/git/mod.rs`, `src/web/api.rs`, `src/web/mod.rs`, `src/web/state.rs`）。
- [x] Web ダッシュボードから承認 UI と承認 API 呼び出しを削除し、`is_approved` 前提の描画を廃止する（`web/app.js` の `handleApprovalClick` / `toggleApproval` / `updateChangeInUI` とカード描画内の approval セクション、`web/style.css` の `.badge-approved` / `.badge-unapproved` / `.approval-*` スタイル）。
- [x] 承認フロー廃止後に未使用となっている承認用フックヘルパーを削除する（`src/orchestration/hooks.rs` の `build_approve_context` と `test_build_approve_context`）。
- [x] 失敗している回帰テストを修正し、`cargo test` を全件成功させる（失敗: `analyzer::tests::test_build_prompt_all_selected`, `analyzer::tests::test_build_prompt_none_selected`, `analyzer::tests::test_build_prompt_with_inflight_changes`, `analyzer::tests::test_build_prompt_with_selected_markers`, `analyzer::tests::test_build_prompt_without_inflight_changes`, `tui::render::tests::test_render_select_mode_footer_message`）。

## Acceptance #2 Failure Follow-up
- [ ] 並列モードで未コミット change を選択中のとき、Changes パネルのキーヒントから選択操作（`Space: queue` / `Space: unqueue`）を表示しないように修正する（`src/tui/render.rs` の `render_changes_list_select` と `render_changes_list_running` のキー表示ロジックに `app.parallel_mode && !item.is_parallel_eligible` 条件を反映し、回帰テストを追加）。
- [ ] `cargo test` が成功するよう doctest 失敗を解消する（現状失敗: `src/web/api.rs` の `not_found_response` ドキュメント例、`src/acceptance.rs`、`src/orchestration/state.rs`、`src/task_parser.rs`。`cargo test --doc` で再確認）。
