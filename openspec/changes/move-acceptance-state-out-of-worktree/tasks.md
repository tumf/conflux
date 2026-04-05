## Implementation Tasks

- [x] 1. `src/parallel/acceptance_state.rs` を worktree 内 `.cflx/acceptance-state.json` 前提から切り離し、`~/.local/state/cflx/acceptance-state/` 相当の外部 storage module へ置き換える (verification: 単体テストで `pending` / `running` / `passed` / `failed` の roundtrip、workspace path 主キー、revision 対応を確認できる)
- [x] 2. `src/parallel/executor.rs` と `src/parallel/dispatch.rs` を新 persistence へ切り替え、apply/acceptance/resume/archive guard が stale revision を `passed` 扱いしないことを回帰テストで確認する (verification: interrupted acceptance と revision mismatch のテストが追加される)
- [x] 3. archive 完了および workspace cleanup 完了時に外部 acceptance state を削除または無効化する (verification: cleanup 後に stale state が残っても次回 archive を解放しない、または state が削除されることをテストで確認できる)
- [x] 4. worktree 配下に `.cflx/acceptance-state.json` を生成しない回帰テストと、merge readiness / dirty worktree の回帰テストを追加する (verification: `src/parallel/tests/executor.rs` などのテストで internal artifact が merge defer 要因にならないことを確認できる)
- [x] 5. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Acceptance #3 Failure Follow-up

- [x] `resolve_merge_for_change()` の merge 成功後 cleanup 経路でも外部 acceptance state を削除する
- [x] `retry_deferred_merges()` の deferred merge retry 成功後 cleanup 経路でも外部 acceptance state を削除する
- [x] worktree 内 `.cflx/acceptance-state.json` が生成・更新されないようにし、`git status --porcelain` が clean になることを確認する
- [x] quality gate タスクの完了表記を実際の gate 成否と一致させる

## Future Work

- pre-commit の `end-of-file-fixer` 実ゲート確認（本環境では `pre-commit` コマンド未導入のため、別環境で hook 実行確認が必要）
- 外部 persistence の cleanup / GC ポリシーを長期運用ログに基づいて最適化する
