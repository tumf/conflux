---
change_type: hybrid
priority: high
dependencies: []
references:
  - .opencode/commands/cflx-accept.md
  - skills/cflx-accept/SKILL.md
  - skills/cflx-workflow/SKILL.md
  - skills/cflx-workflow/references/cflx-accept.md
  - src/acceptance.rs
  - src/agent/prompt.rs
  - src/orchestration/acceptance.rs
  - src/config/defaults.rs
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/testing/spec.md
  - openspec/changes/archive/split-workflow-skills-by-operation/design.md
  - openspec/changes/archive/fix-analyze-resolve-guidance-source/design.md
---

# Change: workflow split 後の prompt contract を harden して regression を防ぐ

**Change Type**: hybrid

## Problem / Context

Conflux は v0.6 系で `cflx-workflow` から operation-specific skills へ prompt surface を分割し、acceptance については `.opencode/commands/cflx-accept.md` を fixed acceptance procedure の single source of truth とする設計へ移行した。しかし実際の acceptance 実行ログでは、agent が `## ACCEPTANCE: PASS` のように markdown heading 付き verdict を出力し、parser がこれを verdict marker として認識できず `CONTINUE` 扱いへ fallback する事象が確認された。

この事故は単一箇所の formatter 問題ではなく、workflow split 後の prompt contract 境界に関する回帰検知不足を示している。acceptance では fixed instructions が command template、operation identity が dedicated skill、runtime context が Rust prompt builder に分離されているが、どの markdown 装飾が許容されるか・runtime parser が何を machine-readable marker とみなすか・それを保護する regression tests が十分に揃っていない。加えて、split 後に analyze / resolve では fixed guidance の duplicated source を後追いで是正した履歴があり、workflow split 以降の prompt ownership drift は acceptance 以外でも再発し得る。

## Proposed Solution

- acceptance verdict contract を parser / command template / skill guidance / canonical spec の全層で明示的に一致させ、`ACCEPTANCE: PASS|FAIL|CONTINUE|BLOCKED` は markdown heading・quote・bullet・code fence なしの standalone line だけを canonical output と定義する
- acceptance parser を harden し、single-source contract を壊さない範囲で軽微な markdown drift（少なくとも heading / quote / bullet など実運用で起こりやすい prefix）に対する防御を追加するか、または parser 側を厳格に保つ代わりに prompt/template/tests でそれらを明示的に禁止し violation を検出する
- workflow split 後の ownership boundary を acceptance でも analyze / resolve と同等に audit し、どの operation が skill-owned fixed guidance / template-owned fixed guidance / Rust-owned runtime context を持つかを spec と tests に反映する
- dedicated skill / command template / Rust prompt builder / parser の組み合わせに対する drift-detection tests を追加し、acceptance に限らず split 対象 operation で machine-readable output contract や fixed-guidance ownership が再度崩れた場合にテストで即座に検出できるようにする
- legacy `cflx-workflow` compatibility router についても、new orchestrator path との契約差分が意図的なものか accidental regression かを整理し、必要な compatibility guidance は維持しつつ duplicated authoritative instructions を増やさないよう明文化する

## Acceptance Criteria

- acceptance の final verdict contract は `.opencode/commands/cflx-accept.md`・関連 skill guidance・canonical spec・runtime parser の期待値が一致し、heading / quote / bullet / fenced-block verdict の扱いが未定義のまま残らない
- acceptance の regression tests は `ACCEPTANCE: PASS` 正常系だけでなく、markdown heading や軽微装飾による drift 系ケースをカバーし、再試行ループへ誤って fallback する回帰を検出できる
- workflow split による prompt ownership boundary について、acceptance を含む対象 operation の authoritative source が spec で追跡可能になり、Rust prompt builder が fixed guidance を再定義していないことを検証するテスト方針が追加される
- `cflx-workflow` compatibility router と dedicated skills / command templates の役割分担が proposal scope 内で整理され、legacy support を維持しながら new path の primary source が曖昧にならない
- proposal の strict validation が通り、split 後の prompt-contract regression を是正・予防する作業項目が implementation evidence と verification ownership を伴って定義される

## Out of Scope

- prompt contract 監査と無関係な新機能追加
- workflow split 自体のロールバック
- acceptance 以外の operation に対する実装変更を、prompt contract / ownership drift 対策と無関係に拡張すること
