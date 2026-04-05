## Implementation Tasks

- [ ] 1. `src/parallel/acceptance_state.rs` を worktree 内 `.cflx/acceptance-state.json` 前提から切り離し、`~/.local/state/cflx/acceptance-state/` 相当の外部 storage module へ置き換える (verification: 単体テストで `pending` / `running` / `passed` / `failed` の roundtrip、workspace path 主キー、revision 対応を確認できる)
- [ ] 2. `src/parallel/executor.rs` と `src/parallel/dispatch.rs` を新 persistence へ切り替え、apply/acceptance/resume/archive guard が stale revision を `passed` 扱いしないことを回帰テストで確認する (verification: interrupted acceptance と revision mismatch のテストが追加される)
- [ ] 3. archive 完了および workspace cleanup 完了時に外部 acceptance state を削除または無効化する (verification: cleanup 後に stale state が残っても次回 archive を解放しない、または state が削除されることをテストで確認できる)
- [ ] 4. worktree 配下に `.cflx/acceptance-state.json` を生成しない回帰テストと、merge readiness / dirty worktree の回帰テストを追加する (verification: `src/parallel/tests/executor.rs` などのテストで internal artifact が merge defer 要因にならないことを確認できる)
- [ ] 5. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- 外部 persistence の cleanup / GC ポリシーを長期運用ログに基づいて最適化する
