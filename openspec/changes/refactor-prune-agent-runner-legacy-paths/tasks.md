## Implementation Tasks

- [x] 1. apply / acceptance / archive / analyze / resolve の現役実行経路を特定し、prompt 展開順・履歴注入・出力変換を固定する characterization test を先に追加または更新する (verification: integration - `src/agent/tests.rs` の `test_with_runner_paths_preserve_prompt_and_output` を含む経路検証テスト更新と `cargo test` 成功)
- [x] 2. `src/agent/runner.rs` に混在している未使用または移行済み entrypoint を削除するか legacy 境界へ隔離し、現役 execution surface を明示する (verification: unit - `src/agent/tests.rs` のレガシーAPI直接呼び出しを `*_with_runner` 経路へ置換し、`src/orchestration/archive.rs` で archive 実行を `run_archive_streaming_with_runner` に寄せた)
- [x] 3. `*_with_runner` 系で重複している command 展開・prompt 注入・出力変換の骨格を共通ヘルパーへ抽出し、各 operation の挙動を変えずに保守点を減らす (verification: unit - 既存の `expand_command_with_prompt` / `bridge_ai_output_channel` を apply/archive 双方で使用し、`test_with_runner_paths_preserve_prompt_and_output` を含む `cargo test` が成功)
- [x] 4. `#[allow(dead_code)]` の適用範囲を必要最小限へ整理し、不要な suppressions を減らす (verification: lint - `run_archive_streaming` のみ互換境界として明示コメント付き `#[allow(dead_code)]` を残し、`cargo clippy --all-targets --all-features -- -D warnings` 成功)
- [ ] 5. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - `cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict --evidence warn` は tasks.md 形式違反で失敗、`cargo test` と `cargo clippy --all-targets --all-features -- -D warnings` は成功)

## Implementation Blocker #1
category: spec_contradiction
summary: レガシーentrypoint削除/隔離要件と、同ファイル内でレガシーentrypointを直接検証している既存テスト群の同時維持が衝突している
evidence:
  src/agent/tests.rs:68 (`runner.run_apply("test-change")`)
  src/agent/tests.rs:79 (`runner.run_archive("test-change")`)
  src/agent/tests.rs:112,151,197 (`runner.run_apply_streaming(...)`)
  src/agent/runner.rs:160-200,368-403,546-613,860-871,1006-1037 (legacy entrypoint definitions)
impact: tasks 2-5 を現状の仕様解釈のまま完了すると、互換要件または既存テスト整合性のいずれかを破る
unblock_actions:
  レガシーentrypointを公開APIから外す対象範囲（テスト専用許容含む）を仕様に明記する
  `src/agent/tests.rs` のレガシー依存テストを `*_with_runner` ベースへ置換するか、legacy境界検証として別目的に再定義する
owner: conflux-maintainers
decision_due: 2026-05-06

## Future Work

- AgentRunner と AiCommandRunner の責務境界をより小さな operation 別モジュールへ分割する検討
- CommandQueue 側の retry / streaming API 整理と合わせた第2段階の簡素化

## Rejecting Recovery Tasks

- [ ] Investigate blocker in openspec/changes/refactor-prune-agent-runner-legacy-paths/REJECTED.md and implement a non-rejection recovery path before rerunning apply (verification: integration - `openspec/changes/refactor-prune-agent-runner-legacy-paths/REJECTED.md` の blocker 根拠を解消するコード変更を加え、`cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict --evidence warn` が成功すること)
