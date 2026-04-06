---
change_type: implementation
priority: medium
dependencies:
  - update-proposal-verification-planning
references:
  - skills/cflx-workflow/SKILL.md
  - skills/README.md
  - skills/tests/test_spec_only_acceptance.py
---

# Update: Workflow skill enforces planned verification types

**Change Type**: implementation

## Premise / Context

- ユーザは、Conflux を使うプロジェクトの workflow 改善には skills 改良が必要だと明示している
- `skills/cflx-workflow/SKILL.md` は apply / accept / archive の自律実行方針を定義しており、verification coverage の運用を実際に守らせる責務はこの skill にある
- 既存 workflow skill には unit test boundary policy や mock-first policy があるが、proposal 時点で planned された verification type と apply/accept の判定を結びつける明示ルールは弱い
- proposal 側で verification planning を導入しても、workflow 側がそれを truthfulness / acceptance judgement に反映しなければ、実際のプロジェクト運用は改善しない

## Problem / Context

現在の `cflx-workflow` は unit test 境界や truthfulness check を持っているが、proposal が意図した verification ownership を apply / accept の実行で体系的に尊重するルールが不足している。

このため、Conflux を使うプロジェクトでは以下が起きうる。

1. proposal では `manual` や `benchmark` で管理すべき requirement が、acceptance 時に単に「自動テスト不足」と見なされる
2. unit を名乗る task に integration-style evidence しかない場合の扱いが局所的で、proposal planning との往復が弱い
3. apply が task completion を進めても、verification type の不一致が follow-up として整理されにくい
4. proposal と workflow の間で verification ownership の意味がずれる

## Proposed Solution

`skills/cflx-workflow/SKILL.md` を更新し、proposal で計画された verification type を apply / accept の判定に反映するルールを追加する。

具体的には以下を導入する。

1. apply は task completion を判断するとき、planned verification path と実際の evidence type の整合性を確認する
2. accept は requirement/task が `unit` / `integration` / `e2e` / `manual` / `benchmark` / `not-testable` のどれとして計画されているかを踏まえて findings を出す
3. `manual` / `benchmark` は intentional coverage として扱い、自動テスト不足そのものを失敗理由にしない
4. `unit` を主張する task に real boundary を跨ぐ integration-style evidence しかない場合は、unit coverage mismatch として follow-up を要求する
5. verification type が曖昧または未計画なら、acceptance は workflow 上の問題として指摘できる

これにより、Conflux を使うプロジェクトでは proposal planning と autonomous execution が同じ verification model を共有できる。

## Acceptance Criteria

1. `skills/cflx-workflow/SKILL.md` が、planned verification type と実 evidence の整合性を apply/accept の判断材料として扱う
2. `manual` / `benchmark` verification が intentional coverage として accept path に明記される
3. `unit` を主張しながら integration-style evidence しかない場合の mismatch handling が skill に明記される
4. verification type が未計画または曖昧な場合の acceptance finding 方針が skill に追加される
5. proposal planning と workflow enforcement の関係が skill 上で説明される
6. `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate update-workflow-verification-enforcement --strict` が成功する

## Out of Scope

- `cflx.py validate` の parser 実装変更
- 各 verification type に対応した CI 自動実行機構の追加
- 既存 archived change に対する acceptance 再実行
