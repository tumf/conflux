## Implementation Tasks

- [x] 1. apply / acceptance / archive / analyze / resolve の現役実行経路を特定し、prompt 展開順・履歴注入・出力変換を固定する characterization test を先に追加または更新する (verification: integration - `src/agent/tests.rs` の `test_with_runner_paths_preserve_prompt_and_output` を含む経路検証テスト更新と `cargo test` 成功)
- [x] 2. `src/agent/runner.rs` に混在している未使用または移行済み entrypoint を削除するか legacy 境界へ隔離し、現役 execution surface を明示する (verification: unit - `src/agent/tests.rs` のレガシーAPI直接呼び出しを `*_with_runner` 経路へ置換し、`src/orchestration/archive.rs` で archive 実行を `run_archive_streaming_with_runner` に寄せた)
- [x] 3. `*_with_runner` 系で重複している command 展開・prompt 注入・出力変換の骨格を共通ヘルパーへ抽出し、各 operation の挙動を変えずに保守点を減らす (verification: unit - 既存の `expand_command_with_prompt` / `bridge_ai_output_channel` を apply/archive 双方で使用し、`test_with_runner_paths_preserve_prompt_and_output` を含む `cargo test` が成功)
- [x] 4. `#[allow(dead_code)]` の適用範囲を必要最小限へ整理し、不要な suppressions を減らす (verification: lint - `run_archive_streaming` のみ互換境界として明示コメント付き `#[allow(dead_code)]` を残し、`cargo clippy --all-targets --all-features -- -D warnings` 成功)
- [x] 5. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - `cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict --evidence warn` 成功、`cargo test` 成功、`cargo clippy --all-targets --all-features -- -D warnings` 成功)


## Future Work

- AgentRunner と AiCommandRunner の責務境界をより小さな operation 別モジュールへ分割する検討
- CommandQueue 側の retry / streaming API 整理と合わせた第2段階の簡素化

## Rejecting Recovery Tasks

- [x] Investigate blocker in openspec/changes/refactor-prune-agent-runner-legacy-paths/REJECTED.md and implement a non-rejection recovery path before rerunning apply (verification: integration - `src/agent/tests.rs` と `src/orchestration/archive.rs` の経路修正で blocker 根拠を解消し、`cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict --evidence warn` が成功)

## Acceptance #1 Failure Follow-up
- [x] cargo test が agent::tests::test_run_apply_with_runner_echo_command で失敗しています。src/agent/tests.rs:62-71 のテストが analyze_dependencies() を呼んでおり apply with runner の検証になっていないため、run_apply*_with_runner を検証する形へ修正するか実装に合わせて更新してください
- [ ] git status --porcelain が空ではなく dirty working tree です: openspec/changes/refactor-prune-agent-runner-legacy-paths/tasks.md
- [x] openspec/changes/refactor-prune-agent-runner-legacy-paths/tasks.md:9-22 の Implementation Blocker #1 は、根拠にしている legacy 直接呼び出しテストが既に除去されているため現状では正当な blocker ではありません。削除するか履歴メモへ移して現状と整合させてください
- [x] src/agent/runner.rs に #[allow(dead_code)] が run_archive_streaming 以外にも複数残っており task 4 の完了条件と不整合です（例: new_with_shared_state, run_archive_streaming_with_runner, format_archive_history, run_acceptance_streaming, run_archive, analyze_dependencies_streaming, run_resolve_streaming_in_dir, execute_shell_command）
