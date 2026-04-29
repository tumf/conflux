---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - .opencode/commands/cflx-accept.md
  - skills/cflx-accept/SKILL.md
  - skills/cflx-archive/SKILL.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/changes/archive/retire-cflx-py-validator/proposal.md
  - openspec/changes/archive/2026-04-29-align-archive-readiness-failure-reporting/proposal.md
---

# Change: behavior-task品質判定をarchive validatorからacceptanceへ移す

**Change Type**: implementation

## Premise / Context

- 現セッションでは、`tasks.md: Runtime behavior is claimed without implementation-facing tasks` が archive failure を大量に誘発していることを確認した。
- この判定は native validator のヒューリスティックとして `src/openspec_cmd.rs` に入り、導入元は `retire-cflx-py-validator` である。
- 一方で repository の acceptance 契約は、behavior-changing work の verification planning や integration evidence を acceptance review が判断する責務をすでに持っている。
- ユーザ要求は、LLM/acceptance で扱うべき proposal-quality 判断を archive blocker にしないこと、必要なら accept prompt を強化すること、ヒューリスティックな validator 判定を取り除くことである。

## Problem / Context

Conflux の現行 proposal-quality enforcement には責務逆転がある。

`retire-cflx-py-validator` で native validator に移された behavior-centric checks は、verification ownership や executable-surface runnable coverage のような structural guidance だけでなく、`runtime behavior is claimed without implementation-facing tasks` のような wording-dependent heuristic まで archive 実行前提の validation path に流し込んでいる。その結果、acceptance で判断・修正すべき proposal/task 品質の問題が archive readiness blocker として発火し、real commit path failure と proposal authoring quality failure が混ざる。

この状態では、

1. acceptance が PASS した後でも archive が proposal-quality heuristic で落ちうる
2. user-facing error は archive failure に見えるため、修正箇所が acceptance/tasks quality なのか real archive blocker なのか分かりにくい
3. LLM を使う acceptance layer と text heuristic validator layer が同じ種類の品質判断を二重に行い、しかも stricter side が archive にある

という不整合が起きる。

## Proposed Solution

behavior-task adequacy の判断を archive validator から外し、必要な品質判定は acceptance review 契約へ寄せる。

- `cflx openspec validate` から、proposal/task wording に依存する `runtime behavior is claimed without implementation-facing tasks` heuristic を削除する。
- native validator は proposal structure・verification note presence・allowed enum など deterministic な authoring contract に集中し、LLM judgement を代替する behavior-task adequacy 判定を持たないようにする。
- acceptance prompt / spec を更新し、behavior-changing work について「implementation-facing tasks が具体的 repository evidence と結びついているか」「runtime integration / execution flow を裏付ける acceptance finding が必要か」を acceptance review が判定する責務を明文化する。
- archive 側 spec / skill では、archive readiness blocker を real commit-path failure と canonical promotion failure に限定し、proposal-quality heuristic だけを理由に archive を fail させない契約を明記する。
- `retire-cflx-py-validator` 由来の validator unification intent は維持するが、その対象を deterministic validation に限定し、behavior-quality judgement は acceptance single source に戻す。

## Acceptance Criteria

- `cflx openspec validate <change-id> --strict --evidence warn|error` は、proposal/task wording から implementation task adequacy を推定する heuristic を emit しない。
- behavior-changing proposal の implementation-task adequacy は acceptance review で判断され、必要なら concrete findings として FAIL になる。
- archive readiness contract は proposal-quality heuristic と real archive blocker を混同せず、archive failure は commit-path / archive execution / canonical promotion の実 blocker に限定される。
- existing acceptance guidance and tests can distinguish missing implementation evidence from archive readiness without depending on `src/openspec_cmd.rs` wording heuristics.
- `Runtime behavior is claimed without implementation-facing tasks` 由来の warning/error がなくても、acceptance contract だけで同種の品質懸念を actionable finding として表現できる。

## Explicit Completion Conditions

- `openspec/specs/cflx-proposal-validation/spec.md` または対応 delta から runtime-behavior / implementation-facing-task heuristic requirement が削除または縮退され、native validator responsibility が deterministic authoring checks に限定される。
- `openspec/specs/agent-prompts/spec.md` と `.opencode/commands/cflx-accept.md` で、behavior-changing work の implementation-task adequacy を acceptance review が concrete evidence ベースで判断する requirement が追加される。
- `src/openspec_cmd.rs` から `Runtime behavior is claimed without implementation-facing tasks` を発火するロジックが除去され、回帰 tests が更新される。
- acceptance-side tests or prompt-contract tests が、behavior-changing proposal で implementation-facing tasks/evidence が不足する場合に acceptance FAIL へ落とせることを固定する。
- `skills/cflx-archive/SKILL.md` と関連 archive spec delta が、proposal-quality heuristic を archive readiness blocker として再導入しないことを明示する。

## Out of Scope

- acceptance review 全体の全面 redesign
- verification ownership marker や executable-surface runnable coverage の deterministic validation まで一律廃止すること
- archived dependency handling や blocked terminology の別件修正
