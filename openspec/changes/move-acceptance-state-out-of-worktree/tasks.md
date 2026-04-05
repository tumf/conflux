## Implementation Tasks

- [ ] 1. 現行の acceptance state 読み書き経路を整理し、worktree 外 persistence へ移す設計を `src/parallel/acceptance_state.rs`, `src/parallel/executor.rs`, `src/parallel/dispatch.rs` に対応づける (verification: 設計が `design.md` に反映され、各利用箇所が列挙されている)
- [ ] 2. durable acceptance state を worktree 外の Conflux 管理領域へ保存・読取する仕組みを実装する (verification: 単体テストで `pending` / `running` / `passed` / `failed` の roundtrip と revision 対応を確認できる)
- [ ] 3. apply/acceptance/resume/archive guard の各導線を新 persistence へ切り替え、worktree 配下に `.cflx/acceptance-state.json` を生成しないようにする (verification: `src/parallel/executor.rs` と `src/parallel/dispatch.rs` の回帰テストで worktree 内ファイル不在を確認できる)
- [ ] 4. merge readiness / dirty worktree の回帰テストを追加し、Conflux 生成 acceptance state artifact が merge defer 要因にならないことを確認する (verification: `src/parallel/tests/executor.rs` などのテストで internal artifact なしの merge 判定を確認できる)
- [ ] 5. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- 外部 persistence の cleanup / GC ポリシーを長期運用ログに基づいて最適化する
