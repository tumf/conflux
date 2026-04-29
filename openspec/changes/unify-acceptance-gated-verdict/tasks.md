## Implementation Tasks

- [x] 1. `openspec/specs/parallel-execution/spec.md`、`openspec/specs/agent-prompts/spec.md`、`openspec/specs/cli/spec.md` の delta を追加し、acceptance verdict の canonical vocabulary を `gated` に統一しつつ dependency `blocked` との責務境界を明記する (verification: integration - `cflx openspec validate unify-acceptance-gated-verdict --strict --evidence warn` が成功し、各 delta が `gated` canonical / `blocked` dependency-only を明示する)
- [x] 2. `.opencode/commands/cflx-accept.md` と `skills/cflx-accept/SKILL.md`、必要な workflow references を更新し、acceptance prompt の正規出力契約を `{"acceptance":"gated"}` / `ACCEPTANCE: GATED` に切り替えつつ、`FAIL=repo 編集で自律修正可能` と `GATED=repo 外前提または apply 再実行だけでは解決不能` の使い分け基準を明文化する (verification: manual - prompt/source diff で `BLOCKED` canonical output が `GATED` に置き換わり、fail/gated rubric と legacy compatibility の扱いが明記されていることを確認する)
- [x] 3. `src/acceptance.rs` の parser / verdict detection を更新し、canonical `gated` output を受理しつつ旧 `blocked` input は backward-compatible fallback としてのみ扱う (verification: unit - acceptance parser tests が `gated` JSON/text を canonical success path として通し、旧 `blocked` JSON/text も compatibility path として受理することを確認する)
- [x] 4. `src/orchestration/acceptance.rs` と acceptance outcome wording を更新し、ログ・履歴・rejection handoff explanation が `blocked verdict` ではなく `gated verdict` / `acceptance-gated` を使うようにする (verification: unit - acceptance/orchestration tests が user-visible wording を `gated` で確認する)
- [x] 5. rejection / orchestration-state / frontend-facing tests と comments を見直し、acceptance outcome と dependency wait の terminology collision が再発しないことを固定する (verification: unit - reducer/frontend/spec-related tests が acceptance gate を `gated`、dependency wait を `blocked` として区別する)
- [x] 6. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate unify-acceptance-gated-verdict --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- legacy `blocked` acceptance verdict 受理の削除時期を別 proposal で決める
- external integrations が `ACCEPTANCE: BLOCKED` を前提にしている場合の deprecation notice を整備する

## Acceptance #1 Failure Follow-up
- [x] openspec/specs/agent-prompts/spec.md:216-223 が canonical spec 側では依然として `ACCEPTANCE: BLOCKED` / blocked verdict を要求しており、change delta `openspec/changes/unify-acceptance-gated-verdict/specs/agent-prompts/spec.md:3-15` の `gated` 契約と矛盾しています。acceptance prompt の正規語彙を canonical spec でも `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` に更新し、legacy `blocked` は移行期入力互換のみであることを反映してください。
- [x] openspec/specs/cli/spec.md:121-126 と openspec/specs/orchestration-state/spec.md:415-446 が still `Acceptance blocked stops apply loop` / `Blocked verdict` を canonical wording として残しており、proposal の acceptance criteria (openspec/changes/unify-acceptance-gated-verdict/proposal.md:52-57) にある「acceptance outcome を gated verdict へ統一」と一致していません。canonical spec の user-facing / orchestration wording を `gated verdict` / `acceptance-gated` に改め、dependency wait の `blocked` と混同しない形へ揃えてください。
