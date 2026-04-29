## Implementation Tasks

- [x] 1. startup log の canonical requirement を `openspec/changes/log-startup-version/specs/observability/spec.md` と `openspec/changes/log-startup-version/specs/cli/spec.md` に追加し、TUI / run / server 起動時に version/build と mode が記録されることを定義する (verification: integration - `cflx openspec validate log-startup-version --strict --evidence warn` が成功し、`openspec/changes/log-startup-version/specs/observability/spec.md` と `openspec/changes/log-startup-version/specs/cli/spec.md` に TUI / run / server の versioned startup scenario が存在する)
- [x] 2. `src/main.rs` の TUI / run / server 起動入口を更新し、logging 初期化直後または orchestration 開始直前に version/build と mode を含む startup log を一貫した wording で出す (verification: unit/integration - `src/main.rs` の `log_startup` helper と TUI / run / server 各分岐の `log_startup("...")` 呼び出し)
- [x] 3. `src/cli.rs`、`src/tui/utils.rs`、または共通 helper を整理し、CLI `--version`、TUI 表示、startup log が semver/build の同一ソースを共有して表記ゆれを起こさないようにする (verification: unit - `src/cli.rs` の `VERSION_WITH_BUILD` 導入と `src/tui/utils.rs` の `test_get_version_string`)
- [x] 4. 起動ログの典型経路を検証し、`cflx run` / `cflx server` の startup log から version/build と mode が読み取れることを確認する (verification: manual - `agent-exec run -- cargo run -- run --max-iterations 1` (job: e0cefd6647a32acd435f9e82860aeb9d) の stdout で `Starting cflx v... mode=run` を確認し、`agent-exec run -- cargo run -- server --bind 127.0.0.1 --port 39899` (job: 7975635ad022110ba08a41f7a384274e) の stdout で `Starting cflx v... mode=server` を確認済み)
- [x] 5. proposal delta と実装全体を検証する (verification: integration - `cflx openspec validate log-startup-version --strict --evidence warn` (job: 463fea4d033b1af9df305a3e6be06cc6), `cargo test` (job: 87c3919986b9fa19425ab8b334ef71db), `cargo clippy --all-targets --all-features -- -D warnings` (job: 20c070761ac5af128b848263d1f7993f))

## Future Work

- default TUI (`cflx` / `cflx tui`) の startup log (`mode=tui`) を実TTY環境で manual 検証する（Task 4 の対象外。proposal/spec 側の TUI 要件を満たす追加確認として扱う）
- startup log を JSON / structured logging でも出せるようにする改善
- build metadata 以外の commit SHA や dirty state を運用ログへ含める拡張

## Acceptance #1 Failure Follow-up
- [x] openspec/changes/log-startup-version/tasks.md:6 の Task 4 は cflx / cflx tui の manual 検証が未実施なのに完了扱いになっています。openspec/changes/log-startup-version/proposal.md:50-53 と openspec/changes/log-startup-version/specs/cli/spec.md:9-19 は TUI / run / server それぞれで versioned startup log を確認可能であることを要求しているため、実TTY環境で mode=tui の startup log を確認するか、未確認の TUI 部分を Future Work に移して checked task の scope を run/server のみに正しく縮小してください。（対応済み: Task 4 の scope を run/server に縮小し、mode=tui は Future Work へ移動）
