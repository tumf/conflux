---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-apply/SKILL.md
  - skills/cflx-workflow/SKILL.md
  - src/openspec_cmd.rs
  - skills/cflx-workflow/scripts/cflx.py
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/handle-archived-dependency-references/proposal.md
  - openspec/changes/fix-archive-validation-blocker-reporting/proposal.md
---

# Change: optional design.md read failures should not be surfaced as apply errors

**Change Type**: implementation

## Premise / Context

- `~/.local/state/cflx/logs/.last-checked` 以降の cflx 実行ログでは、apply 中に `openspec/changes/<id>/design.md` が存在しないだけで `Error: File not found: .../design.md` が大量に記録されている。
- 現行の apply guidance は `design.md` を「if exists」で読む前提を明示している (`skills/cflx-apply/SKILL.md`, `skills/cflx-workflow/SKILL.md`)。
- つまり missing `design.md` 自体は proposal contract 違反ではなく、optional artifact の不在である。
- それにもかかわらず runtime/TUI logs では error として surfaced されるため、実際の apply failure と optional context miss が区別しにくい。
- 既存の active proposals は archived dependency handling と archive validation mode mismatch を扱っており、この optional design read surfacing はそれらと独立した別スコープである。

## Requested Artifact

- implementation proposal for downgrading optional `design.md` absence from user-visible error to non-error informational behavior
- canonical prompt/runtime contract for optional OpenSpec artifacts during apply/acceptance context gathering
- regression coverage proving missing optional design docs no longer pollute error tracking

## Problem / Context

Conflux の apply / acceptance 文脈では `proposal.md` と `tasks.md` は必須だが、`design.md` は optional である。現在の skill guidance でもその前提を採っている一方、実行ログでは `design.md` 不在が `Error: File not found` として記録されるケースが繰り返し発生している。

この挙動により、

1. optional context file が無いだけなのか、
2. 実際に apply が失敗したのか、
3. proposal author が修正すべき contract violation なのか

をログだけで判別しづらくなる。エラー件数ベースの運用や triage でもノイズになり、真の failure analysis を妨げる。

## Proposed Solution

optional OpenSpec artifact の不在を canonical contract で明文化し、runtime・prompt・user-facing log の扱いをそろえる。

- `design.md` は optional artifact であり、不在でも apply / acceptance context gathering は継続可能であることを canonical spec に追加する。
- apply / workflow guidance が optional file を読むときは、「存在すれば読む、無ければ skip して続行する」契約を runtime/TUI log wording と一致させる。
- user-facing log / event では missing optional `design.md` を error ではなく info または debug の skip message として扱い、change failure state に昇格させない。
- truly required files (`proposal.md`, `tasks.md`) の missing は引き続き error / failure として扱い、optional vs required の境界を tests で固定する。
- archived dependency / archive validation mode のような既存 active proposal の failure surfacing 改善とは独立に、optional artifact noise suppression を non-overlapping scope で進める。

## Acceptance Criteria

- `openspec/changes/<id>/design.md` が存在しない proposal に対して、apply / acceptance 文脈収集は継続し、missing design doc が user-visible error として記録されない。
- `proposal.md` または `tasks.md` の missing は引き続き failure になり、optional file handling と required file handling が混同されない。
- prompt/runtime/spec のいずれを見ても、`design.md` は optional であることと、不在時の expected behavior が一致している。
- regression tests が missing optional `design.md` case を再現し、error noise が再発しないことを確認する。
- `cflx openspec validate optional-design-read-noise --strict --evidence warn` が成功する。

## Explicit Completion Conditions

- canonical spec delta が optional OpenSpec artifact (`design.md`) の read contract と non-error behavior を requirement/scenario として定義している。
- `design.md` 不在を optional skip として扱う repository-verifiable な実装経路が tasks に明記されており、少なくとも確認済みの artifact reader (`src/openspec_cmd.rs`, `skills/cflx-workflow/scripts/cflx.py`) と必要に応じた error/log surfacing 経路の特定・修正が含まれている。
- `skills/cflx-apply/SKILL.md` / `skills/cflx-workflow/SKILL.md` と runtime behavior の整合を確認する task が含まれている。
- optional `design.md` missing と required file missing の両方を区別する regression coverage が tasks に含まれている。

## Out of Scope

- proposal authoring 時に `design.md` を自動生成する機能
- apply / acceptance 以外の任意ローカルファイル read policy 全体の再設計
- archived dependency handling や archive validation mode mismatch の修正そのもの
