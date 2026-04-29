## Implementation Tasks

- [x] 1. startup log の canonical requirement を `openspec/changes/log-startup-version/specs/observability/spec.md` と `openspec/changes/log-startup-version/specs/cli/spec.md` に追加し、TUI / run / server 起動時に version/build と mode が記録されることを定義する (verification: integration - `cflx openspec validate log-startup-version --strict --evidence warn` が成功し、`openspec/changes/log-startup-version/specs/observability/spec.md` と `openspec/changes/log-startup-version/specs/cli/spec.md` に TUI / run / server の versioned startup scenario が存在する)
- [x] 2. `src/main.rs` の TUI / run / server 起動入口を更新し、logging 初期化直後または orchestration 開始直前に version/build と mode を含む startup log を一貫した wording で出す (verification: unit/integration - `src/main.rs` の `log_startup` helper と TUI / run / server 各分岐の `log_startup("...")` 呼び出し)
- [x] 3. `src/cli.rs`、`src/tui/utils.rs`、または共通 helper を整理し、CLI `--version`、TUI 表示、startup log が semver/build の同一ソースを共有して表記ゆれを起こさないようにする (verification: unit - `src/cli.rs` の `VERSION_WITH_BUILD` 導入と `src/tui/utils.rs` の `test_get_version_string`)
- [ ] 4. 起動ログの典型経路を検証し、`cflx` / `cflx run` / `cflx server` の startup log から version/build と mode が読み取れることを確認する (verification: manual - `cflx`, `cflx run`, `cflx server` をそれぞれ起動して `~/.local/state/cflx/logs/<project_slug>/<YYYY-MM-DD>.log` または stdout を確認し、少なくとも 1 件の startup log に `cflx v... (...)` と `mode=tui|run|server` 相当の識別子が含まれる)
- [ ] 5. proposal delta と実装全体を検証する (verification: integration - `cflx openspec validate log-startup-version --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- startup log を JSON / structured logging でも出せるようにする改善
- build metadata 以外の commit SHA や dirty state を運用ログへ含める拡張
