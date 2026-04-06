## Implementation Tasks

- [ ] 1. `ResumeAction::Acceptance` の handoff 条件を明文化どおりに固定する: `src/parallel/dispatch.rs` の apply/acceptance loop で、acceptance `Pass` 以外では archive handoff しないことを確認し、必要なら cycle state を追加する (verification: `Applied` resume で durable pass がない場合に archive 実行へ進まないテストを追加)
- [ ] 2. `Applied + failed durable state` の resume 回帰テストを追加する: `src/parallel/tests/executor.rs` または resume routing テストで failed durable acceptance state を持つ revision が archive ではなく acceptance に進むことを検証する (verification: 新規テストが pass)
- [ ] 3. `Applied + missing durable pass` の dispatch/phase 整合テストを追加する: `state=Applied -> Acceptance` のケースで archive guard failure が先行せず acceptance 実行が開始されることを検証する (verification: 新規テストが pass)
- [ ] 4. `Applied + passed durable pass` の archive continuation テストを維持・補強する: durable pass がある revision のみ `ResumeAction::Archive` から archive 継続できることを確認する (verification: 既存/追加テストが pass)
- [ ] 5. phase ログ整合性を改善する: `src/parallel/dispatch.rs` / `src/parallel/executor.rs` で resume routing と実行開始 phase が一致するログを出す、または不整合を警告として観測可能にする (verification: ログ系テストまたは event assertion が pass)
- [ ] 6. spec delta を canonical spec に追加する: `parallel-execution` に Applied resume handoff 条件を追記し、acceptance missing/failed revision で archive に進まないことを明記する (verification: `cflx.py validate --strict` が pass)

## Future Work

- acceptance state を UI 上で直接可視化する改善
- archive guard と resume routing の共有 validator 化
