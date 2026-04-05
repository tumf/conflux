## Implementation Tasks

- [ ] 1. workspace ごとの実効 Git dir から `info/exclude` を解決し、`.cflx/acceptance-state.json` を idempotent に登録するヘルパーを追加する (verification: worktree と通常 repo の両方で exclude path 解決と重複防止を単体テストで確認できる)
- [ ] 2. acceptance state 保存フローまたは workspace 初期化フローから exclude 登録を呼び出し、state 書き込み前に ignore が保証されるようにする (verification: `src/parallel/acceptance_state.rs` または worktree 作成導線のテストで `.git/info/exclude` への登録を確認できる)
- [ ] 3. `.gitignore` から `.cflx/acceptance-state.json` を削除し、workspace local exclude が dirty worktree 判定を吸収することを回帰テストで確認する (verification: `git status --porcelain` ベースのテストで acceptance state が未追跡表示されないことを確認できる)
- [ ] 4. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- `.cflx/` 配下の将来追加される内部生成物を file-by-file で local exclude 管理するか、専用管理ディレクトリ方針へ拡張するかを再評価する
