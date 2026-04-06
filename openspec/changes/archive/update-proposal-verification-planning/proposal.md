---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-proposal/SKILL.md
  - skills/README.md
  - skills/tests/test_cflx_proposal_change_types.py
---

# Update: Proposal skill plans verification coverage explicitly

**Change Type**: implementation

## Premise / Context

- ユーザは、spec と unit test の対応を Conflux 本体の内向き仕様ではなく、Conflux を使うプロジェクトのワークフロー改善として扱いたいと明示している
- このリポジトリでは `skills/cflx-proposal/SKILL.md` が proposal 作成時の行動規範を定義しており、プロジェクト横断の planning 改善は skill 改良で行うのが自然である
- 直前の議論では「すべての spec を unit test 化する必要はない」が合意されており、必要なのは test coverage ではなく verification coverage の明示である
- `cflx-proposal` は既に tasks に verification note を要求しているが、verification type 自体の計画（unit / integration / manual / benchmark など）は明示必須になっていない

## Problem / Context

現在の `cflx-proposal` は、proposal 作成時に「どの requirement をどの検証レイヤーで担保するか」を体系的に計画させない。そのため、Conflux を使う各プロジェクトでは以下が起きやすい。

1. `unit test がない` のか `manual verification で管理する想定` なのかが proposal 上で区別できない
2. UI/UX や性能要件のように unit test 化に向かない requirement が、未カバーに見える
3. 実装担当や acceptance 担当が verification ownership を後から解釈する必要があり、workflow がぶれる
4. spec-driven 開発のはずなのに、proposal 時点で verification planning が弱い

## Proposed Solution

`skills/cflx-proposal/SKILL.md` を更新し、behavior-changing requirement には verification coverage planning を必須で持たせる。

具体的には、proposal 作成時の標準運用として以下を追加する。

1. requirement または対応タスクごとに verification type を決める
2. 有効な verification type として `unit` / `integration` / `e2e` / `manual` / `benchmark` / `not-testable` を扱う
3. `manual` / `benchmark` を unit test 不足ではなく intentional coverage として扱う
4. tasks.md に verification path を明示し、実装タスクと検証責務を対応づける
5. proposal を split する際も、proposal ごとに verification ownership が閉じるように誘導する

この変更により、Conflux を使うプロジェクトでは proposal 時点で「何をどの方法で検証するのか」が明示され、unit test 化の過不足ではなく verification coverage を基準に議論できるようになる。

## Acceptance Criteria

1. `skills/cflx-proposal/SKILL.md` が、behavior-changing proposal で verification coverage planning を必須の planning 要素として扱う
2. skill 内に `unit` / `integration` / `e2e` / `manual` / `benchmark` / `not-testable` の標準 vocabulary が記載される
3. `manual` および `benchmark` が intentional coverage として扱われ、unit test 不在そのものを欠陥とみなさない方針が明記される
4. tasks.md の guidance が、verification note だけでなく verification ownership を追跡できる内容になる
5. proposal 利用者が verification coverage を proposal 時点で判断すべきことが skill 上で明示される
6. `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate update-proposal-verification-planning --strict` が成功する

## Out of Scope

- `cflx.py validate` の実装変更
- `cflx-workflow` accept/apply の実行判定変更
- 既存 proposal 全件への retroactive な verification type 追記
