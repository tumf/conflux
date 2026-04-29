---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/acceptance.rs
  - src/orchestration/acceptance.rs
  - .opencode/commands/cflx-accept.md
  - skills/cflx-accept/SKILL.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/cli/spec.md
---

# Change: acceptance verdict terminology を blocked から gated に統一する

**Change Type**: implementation

## Premise / Context

- このセッションでは、acceptance verdict の機械可読 contract が `blocked` のままなのに対し、canonical spec / reducer / UI taxonomy は acceptance 側の blocker observation を `gated` / `acceptance-gated` として扱っていることを確認した。
- canonical spec はすでに dependency wait の `blocked`、apply/rejecting hold の `stalled`、acceptance gate の `gated` を区別している (`openspec/specs/orchestration-state/spec.md`, `openspec/specs/parallel-execution/spec.md`)。
- 一方で acceptance parser / prompt contract / skill docs では `pass|fail|continue|blocked` が残っており、acceptance path だけが dependency `blocked` の語彙を再利用している。
- 既存 active change `classify-acceptance-followup-routing` は acceptance follow-up routing の話であり、verdict vocabulary 自体の統一は扱っていない。

## Requested Artifact

- implementation proposal for unifying acceptance verdict vocabulary on `gated`
- keep dependency wait vocabulary on `blocked`
- preserve backward compatibility long enough to avoid breaking older orchestrator/skill combinations

## Problem / Context

Conflux では acceptance blocker の canonical concept が `acceptance-gated` であるにもかかわらず、acceptance verdict contract は依然として `blocked` を返す。そのため、仕様・表示・イベント・prompt contract のあいだで「acceptance 由来の gate」と「dependency wait としての blocked」が同じ語に見えたり、境界説明が二重化したりする。

この不一致は単なる文言差ではなく、proposal / prompt / parser / runtime / frontend 間の protocol surface を曖昧にする。acceptance verdict を `gated` に揃え、`blocked` を dependency wait 専用語へ固定しない限り、今後の routing / rejection / reducer work でも vocabulary drift が再発しやすい。

## Proposed Solution

acceptance verdict の canonical machine-readable / text contract を `gated` ベースへ移行し、dependency wait の `blocked` と明確に分離する。

- acceptance parser の canonical verdict set を `pass|fail|continue|gated` に更新し、`blocked` は移行期間の backward-compatible input としてのみ受理する。
- `.opencode/commands/cflx-accept.md`、`skills/cflx-accept/SKILL.md`、関連 workflow references を `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` を正規 contract として更新する。
- accept スキル / prompt contract に `FAIL` と `GATED` の使い分け基準を追加し、repo 内編集で自律的に解決可能な問題のみを `FAIL`、repo 外前提・人判断待ち・外部依存解消待ち・仕様前提不足など apply を再実行しても解決不能な blocker を `GATED` とすることを明示する。
- canonical specs (`parallel-execution`, `agent-prompts`, `cli`) で acceptance verdict terminology を `gated` に統一し、dependency wait の `blocked` と役割境界を明記する。
- rejection/review や orchestration state が acceptance outcome を説明する文面も、`blocked verdict` ではなく `gated verdict` / `acceptance-gated` に寄せる。
- 互換期間中は parser が旧 `blocked` を読めても、新規 prompt/output/docs/tests は `gated` のみを生成・期待する。

## Acceptance Criteria

- acceptance の canonical JSON verdict は `{"acceptance":"gated"}` であり、canonical text fallback は `ACCEPTANCE: GATED` になる。
- dependency wait の `blocked` terminology は queue/dependency semantics に限定され、acceptance verdict / acceptance prompt / acceptance parser の正規出力契約では使われない。
- canonical spec prose と prompt contract は acceptance blocker を `gated` / `acceptance-gated` として説明し、dependency `blocked` と混同しない。
- accept prompt contract は `FAIL=repo 編集で自律修正可能`, `GATED=repo 外前提または apply 再実行だけでは解決不能` の判断基準を明文化し、`gated` の不在を `apply へ戻してよい` の推定根拠に使わせない。
- runtime parser は移行期間中に旧 `blocked` verdict を後方互換入力として受理できるが、新規テスト・prompt・docs は `gated` を期待する。
- rejection / orchestration wording で acceptance outcome を説明する箇所は `gated verdict` へ揃い、`blocked verdict` は dependency wait 文脈以外で新規追加されない。

## Explicit Completion Conditions

- OpenSpec delta が acceptance verdict vocabulary と dependency wait vocabulary の責務境界を canonical spec として記述している。
- `.opencode/commands/cflx-accept.md` と `skills/cflx-accept/SKILL.md` の両方に `gated` verdict contract が反映される実装タスクが tasks に含まれている。
- `src/acceptance.rs` と関連 parser tests で `gated` canonical / `blocked` backward-compatible input を確認するタスクが tasks に含まれている。
- `cflx openspec validate unify-acceptance-gated-verdict --strict --evidence warn` が成功する。

## Out of Scope

- acceptance follow-up routing の phase-aware 再設計
- apply-side blocked/stalled lifecycle 全体の再設計
- dependency scheduler semantics 自体の変更
