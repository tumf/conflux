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
- [x] `tasks.md:20` の Acceptance #1 Failure Follow-up 追記タスクが、完了チェック付きの挙動変更/検証タスクとして扱われる文面なのに `(verification: ...)` を含んでいなかったため、OpenSpec evidence 注記チェックに失敗していました。既に `cargo test --lib` は agent-exec job `866fd18c95005a9db5ec3c1e4dce0f89` で成功し、`cargo fmt --check` も成功しているため、前回の `CONTROL_CALLS` 並列混入問題は `src/server/api/control.rs:257-270` の `lock_control_calls_for_test()` と各該当テストの利用により解消されています。Acceptance #1 Failure Follow-up に検証 ownership と repository-verifiable evidence を含む注記を追加し、tasks.md の evidence 注記不足を解消しました。(verification: manual - repository-verifiable evidence: runnable commands `cargo test --lib` and `cargo fmt --check`, plus source path `src/server/api/control.rs:257-270`)

## Acceptance #3 Failure Follow-up
- [x] `tasks.md:23` の Acceptance #2 Failure Follow-up チェックボックスが、最終 OpenSpec 検証そのものを完了条件として含んでいたため、自己参照的な final validation task と判定されていました。完了チェック付きタスクから最終ゲート成功確認の文言を外し、最終検証は既存の非チェックボックス `## Final Validation` セクションに残す形へ整理しました。前回指摘の `(verification: ...)` 注記不足は `tasks.md:20` で解消済みです。(verification: manual - repository-verifiable evidence: source path `openspec/changes/fix-local-tui-live-change-visibility/tasks.md:23` no longer contains a final-validation completion claim)

## Acceptance #4 Failure Follow-up
- [x] archive-gate/strict 検証が `tasks.md:23` の evidence/ownership 不足と `tasks.md:26` の自己参照 final validation checkbox により失敗していました。Acceptance #2 の verification note を、検証 ownership と repository-verifiable evidence を含む `manual - repository-verifiable evidence: ...` 形式へ更新し、Acceptance #3 から final OpenSpec validation 成功を完了条件にする自己参照文言を除去しました。自己参照的な最終 OpenSpec 検証確認は非チェックボックスの `## Final Validation` セクションに残し、チェック付きタスクは tasks.md 文面修正そのものだけを完了条件にしました。(verification: manual - repository-verifiable evidence: source paths `openspec/changes/fix-local-tui-live-change-visibility/tasks.md:23` and `openspec/changes/fix-local-tui-live-change-visibility/tasks.md:26`; runnable commands `cargo fmt --check`, `cflx openspec validate fix-local-tui-live-change-visibility --strict`, and `cflx openspec validate fix-local-tui-live-change-visibility --archive-gate`)
