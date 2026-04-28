## Implementation Tasks

- [x] 1. apply / acceptance / archive / analyze / resolve の現役実行経路を特定し、prompt 展開順・履歴注入・出力変換を固定する characterization test を先に追加または更新する (verification: unit/integration - add or update Rust tests proving current `*_with_runner` paths preserve prompt/history/output behavior before structural refactor)
- [ ] 2. `src/agent/runner.rs` に混在している未使用または移行済み entrypoint を削除するか legacy 境界へ隔離し、現役 execution surface を明示する (verification: unit - inspect module boundaries and run targeted tests confirming supported execution flows still compile and pass)
- [ ] 3. `*_with_runner` 系で重複している command 展開・prompt 注入・出力変換の骨格を共通ヘルパーへ抽出し、各 operation の挙動を変えずに保守点を減らす (verification: unit - run characterization tests and confirm all operations still produce the same observable outputs and error handling)
- [ ] 4. `#[allow(dead_code)]` の適用範囲を必要最小限へ整理し、不要な suppressions を減らす (verification: lint - run `cargo clippy --all-targets --all-features -- -D warnings` and confirm dead-code suppression remains only on intentional compatibility boundaries)
- [ ] 5. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - run `cflx openspec validate refactor-prune-agent-runner-legacy-paths --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Implementation Blocker #1
- category: spec_contradiction
- summary: レガシーentrypoint削除/隔離要件と、同ファイル内でレガシーentrypointを直接検証している既存テスト群の同時維持が衝突している
- evidence:
  - src/agent/tests.rs:68 (`runner.run_apply("test-change")`)
  - src/agent/tests.rs:79 (`runner.run_archive("test-change")`)
  - src/agent/tests.rs:112,151,197 (`runner.run_apply_streaming(...)`)
  - src/agent/runner.rs:160-200,368-403,546-613,860-871,1006-1037 (legacy entrypoint definitions)
- impact: tasks 2-5 を現状の仕様解釈のまま完了すると、互換要件または既存テスト整合性のいずれかを破る
- unblock_actions:
  - レガシーentrypointを公開APIから外す対象範囲（テスト専用許容含む）を仕様に明記する
  - `src/agent/tests.rs` のレガシー依存テストを `*_with_runner` ベースへ置換するか、legacy境界検証として別目的に再定義する
- owner: conflux-maintainers
- decision_due: 2026-05-06

## Future Work

- AgentRunner と AiCommandRunner の責務境界をより小さな operation 別モジュールへ分割する検討
- CommandQueue 側の retry / streaming API 整理と合わせた第2段階の簡素化
