## Implementation Tasks

- [x] 1. `openspec/specs/cli/spec.md` と必要なら proposal-session 系 spec delta を追加し、native validator が free-text keyword inference ではなく explicit structure だけを検証する契約を記述する (verification: integration - `cflx openspec validate remove-heuristic-validator-and-promotion-fallback --strict --evidence warn` が成功し、spec delta が non-heuristic validator contract を明示する)
- [x] 2. `src/openspec_cmd.rs` から `BEHAVIOR_TASK_KEYWORDS`, `ARTIFACT_HEAVY_TASK_KEYWORDS`, `EXECUTABLE_SURFACE_HINTS` 等に依存する quality inference を削除し、validator の判定根拠を explicit marker / parseable field に限定する (verification: unit - validator tests が wording だけを変えた proposal/task で warning の有無が変わらないことを確認する)
- [x] 3. verification ownership や明示 metadata が必要な場合、その要求は structured field / explicit marker 不足として deterministic に報告し、文章の意味推測に依存しないようにする (verification: unit - validator tests が explicit marker の有無だけで pass/fail or warn を決め、同義語や言い換えでは結果が変わらないことを確認する)
- [x] 4. `delta_to_canonical(...)` の fallback rewrite を削除し、requirement block を parse できない malformed delta は canonical promotion error を返すようにする (verification: unit - malformed delta test が fallback canonical text を返さず parse error になることを確認する)
- [x] 5. archive/promotion path の diagnostics を更新し、malformed delta failure を `parse error` / `promotion error` として明示し、automatic rewrite を示唆しないことを固定する (verification: integration - archive/promotion tests が malformed delta を deterministic parse failure として観測する)
- [x] 6. proposal/session guidance と native validator responsibility を整合させ、author が明示すべき構造を docs/spec で説明する (verification: manual - related spec/doc diff review で validator responsibility が free-text inference から explicit structure validation へ揃っていることを確認する)
- [x] 7. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate remove-heuristic-validator-and-promotion-fallback --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- proposal authoring UI / CLI に explicit metadata を入力しやすくする補助機能
- acceptance / proposal workflow 全体の structured-schema 化
