## Implementation Tasks

- [ ] `OrchestratorConfig` の全queue関連フィールドに非既定値を設定し、現在のfallbackを含む変換contractをcharacterization testで固定する (verification: unit - `src/command_queue.rs` のtestで全 `CommandQueueConfig` フィールドをassertし、`cargo test command_queue` が成功する)
- [ ] `OrchestratorConfig` から `CommandQueueConfig` を生成する単一の共通変換を既存の設定境界へ追加する (verification: unit - `src/command_queue.rs` の変換testが全フィールドの一致を証明する)
- [ ] stream JSON textification、strict cleanup、command environmentsを反映する設定済み `AiCommandRunner` の最小共通生成処理を追加する (verification: unit - `src/ai_command_runner.rs` のtestが各設定の保持を確認し、`cargo test ai_command_runner` が成功する)
- [ ] `src/agent/runner.rs` の通常constructorとshared-state constructorを共通queue変換へ移し、history初期化とshared state注入を維持する (verification: unit - `src/agent/tests.rs` と `cargo test agent::` が成功する)
- [ ] `src/orchestrator.rs` と `src/parallel_run_service.rs` の完全一致するproduction初期化を共通処理へ置換し、意図的なtest fixture overrideは残す (verification: integration - 対象source pathの重複初期化が除去され、`cargo test parallel_run_service orchestrator` と `cargo check --all-features` が成功する)
- [ ] formattingとdefault lintを実行して、共通化が未使用APIや警告を導入していないことを確認する (verification: integration - `src/command_queue.rs`、`src/ai_command_runner.rs`、各call siteを対象に `cargo fmt --all -- --check` と `cargo clippy -- -D warnings` が成功する)

## Future Work

完全一致しないrunner初期化の統合は、各overrideの意図を個別に確認できる別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate centralize-command-queue-config --archive-gate`
