## Implementation Tasks

- [x] Add repo-root-based active and rejected change listing for local TUI refresh, replacing cwd-relative listing in the refresh path (verification: unit - tests in `src/openspec.rs` prove listing from an explicit base path still finds active and rejected rows when process cwd points elsewhere)
- [x] Wire local TUI auto-refresh to pass the captured `repo_root` through active and rejected change update logic without changing remote-mode refresh bypass behavior (verification: unit/integration - tests around `src/tui/runner.rs` or refresh helpers verify local refresh uses explicit repo root and remote mode still bypasses local refresh)
- [x] Preserve new active change state as unselected `not queued` with `is_new = true` and rejected rows as read-only non-new rows (verification: unit - `src/tui/state/processing_logic.rs` or refresh handler tests cover active new row, rejected new row, marker removal reactivation, and unchanged cursor index)
- [x] Add a Running-mode visible new-change indicator when `new_change_count > 0`, independent of whether the appended row is inside the visible list viewport (verification: unit - `src/tui/render.rs` TestBackend render test shows `New: 1` or equivalent in Running mode with logs panel enabled and many changes)
- [x] Emit a TUI log entry when active changes are newly detected, while keeping that log observability-only and not feeding queue/scheduler state (verification: unit - `src/tui/state/processing_logic.rs` or `src/tui/state/event_handlers/refresh.rs` test confirms the log entry appears once per newly detected active change and no `selected`/queue status changes are introduced by logging)
- [x] Run affected Rust verification and formatting checks (verification: manual - run `cargo test` for affected modules/tests and `cargo fmt --check`; if full `cargo test` is too slow, document the targeted commands and why broader coverage was not run)

## Future Work

- Manual dogfood: run local TUI, add a valid `openspec/changes/<id>` with `proposal.md` during Running mode, and confirm the row or new-change indicator is visible without moving the cursor.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] `cargo test --lib` が失敗しており、実リポジトリの通常 Rust ライブラリ検証が通りません。agent-exec job `6b6c3f554ac56e65116ff0315db0e74b` / command `cargo test --lib` の結果: `test result: FAILED. 1873 passed; 1 failed; 6 ignored`。失敗箇所は `src/server/api/control.rs:1139` の `server::api::control::tests::test_global_control_run_skips_rejected_changes` で、期待値 `[("_global_", "run")]` に対し実値が `[("_global_", "run"), ("_global_", "run"), ("_global_", "run")]` でした。これはグローバルな `CONTROL_CALLS` が他テストからの呼び出しを混入しており、デフォルトの `cargo test --lib` 並列実行で不安定/失敗する状態です。`src/server/api/control.rs` にテスト専用の `lock_control_calls_for_test()` を追加し、`CONTROL_CALLS` を使う control/projects のテストを同じ async mutex で直列化・クリアすることで、並列実行中の呼び出し混入を隔離しました。検証: `cargo test --lib`、`cargo fmt --check`、`cflx openspec validate fix-local-tui-live-change-visibility --strict`、`cflx openspec validate fix-local-tui-live-change-visibility --archive-gate` を実行予定。(verification: manual - `cargo test --lib`、`cargo fmt --check`、`cflx openspec validate fix-local-tui-live-change-visibility --strict`、`cflx openspec validate fix-local-tui-live-change-visibility --archive-gate` で確認)

## Acceptance #2 Failure Follow-up
- [x] `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate` が失敗しており、archive commit 前の実 archive gate を通過できません。実行結果: `✗ Validation failed: fix-local-tui-live-change-visibility: tasks.md:20: Behavior-bearing task missing '(verification: ...)' note`。該当箇所は `openspec/changes/fix-local-tui-live-change-visibility/tasks.md:19-20` の Acceptance #1 Failure Follow-up 追記タスクで、完了チェック付きの挙動変更/検証タスクとして扱われる文面なのに `(verification: ...)` が含まれていません。既に `cargo test --lib` は agent-exec job `866fd18c95005a9db5ec3c1e4dce0f89` で成功し、`cargo fmt --check` と `cflx openspec validate fix-local-tui-live-change-visibility --strict` も成功しているため、前回の `CONTROL_CALLS` 並列混入問題は `src/server/api/control.rs:257-270` の `lock_control_calls_for_test()` と各該当テストの利用により解消されています。Acceptance #1 Failure Follow-up に `(verification: manual - ...)` 注記を追加し、archive gate の tasks.md 検証注記不足を解消しました。(verification: manual - `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate` がこの follow-up 完了後に成功することを確認する)
