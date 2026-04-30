---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/orchestration/archive.rs
  - src/orchestration/acceptance.rs
  - src/task_parser.rs
  - .opencode/commands/cflx-accept.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/cli/spec.md
---

# Change: self-modifying control-plane changes の phase boundary を harden する

**Change Type**: implementation

## Premise / Context

- このセッションでは、Conflux 自身の accept prompt / acceptance parser / follow-up routing / archive promotion を変更する change で、error が多発する理由を整理した。
- 具体的には、(1) acceptance fail と persistence failure の未分離、(2) blocker-only follow-up の apply 誤送、(3) acceptance `blocked` / `gated` の contract drift、(4) archive no-op / spec promotion 不整合が archive stall へ増幅される、という複数の phase boundary 問題が確認された。
- 既存 proposal は個別論点を扱う (`classify-acceptance-followup-routing`, `unify-acceptance-gated-verdict`, `degrade-acceptance-followup-persistence`) が、Conflux 自己変更時に phase boundary をどう harden すべきかの横断方針は未提案である。
- リポジトリ instructions では OpenSpec canonical spec と実装・prompt・state の整合が重要であり、ログで確認した failure もほぼ control-plane 自己変更に集中していた。

## Requested Artifact

- implementation proposal for reducing error bursts when Conflux changes its own control-plane contracts
- cross-cutting hardening around phase boundaries: acceptance, follow-up persistence, resume/apply routing, and archive promotion
- explicit verification strategy for self-modifying / control-plane change classes

## Problem / Context

Conflux は通常の product change だけでなく、自身の prompt contract・parser・runtime state・archive semantics も変更対象にする。しかしこの種の self-modifying control-plane change では、仕様・prompt・parser・state・archive promotion が複数 phase に跨って相互依存するため、1つの不整合が別 phase の hard error や stall に増幅されやすい。

結果として、正当な acceptance fail が metadata persistence error へ増幅されたり、archive prerequisite mismatch が archive no-op stall として遅れて観測されたりする。個別修正だけでは再発しやすく、「どの phase で何を primary diagnosis とし、何を secondary degradation とするか」「self-modifying change をどう早期検証するか」という共通 hardening policy が必要である。

## Proposed Solution

Conflux 自身の control-plane contract を変える change を、通常 change とは異なる risk class として扱い、phase boundary ごとの hardening contract を導入する。

- self-modifying / control-plane change を proposal/spec 上で識別できるようにし、acceptance・archive・resume routing にまたがる cross-phase verification ownership を要求する。
- acceptance fail / follow-up persistence / archive prerequisite mismatch / archive no-op stall を primary vs secondary の failure taxonomy として明確化し、secondary degradation が primary diagnosis を上書きしない contract を入れる。
- archive へ入る前に spec promotion / heading alignment / canonical diff feasibility を早期にチェックできる self-change preflight を導入し、archive phase で初めて no-op stall になる事態を減らす。
- runtime は blocker-only follow-up, persistence degradation, archive precondition failure のような non-progress condition を phase-specific に区別し、同じ empty-WIP stall へ雑に集約しない。
- self-modifying change 向け regression suite を定義し、accept prompt / parser / routing / archive promotion が同時に変わる case でも、実行ログベースで primary diagnosis が安定することを確認する。

## Acceptance Criteria

- control-plane / self-modifying change と通常 product change の区別が proposal/spec か verification policy 上で表現される。
- acceptance fail, follow-up persistence degradation, archive prerequisite failure, archive no-op stall が少なくとも spec / logs / tests のいずれかで primary/secondary を区別して扱われる。
- self-modifying change は archive に入る前に spec promotion feasibility または equivalent preflight を通し、見出し整合や no-op canonical diff の問題を archive phase で初めて発見しない。
- blocker-only follow-up や persistence degradation のような non-progress condition が、empty WIP stall の generic error だけで表現されない。
- regression tests か scripted verification が、accept prompt / parser / routing / archive promotion を同時に含む self-change scenario を少なくとも1本カバーする。

## Explicit Completion Conditions

- OpenSpec delta が self-modifying control-plane change の risk class と phase-boundary hardening expectations を canonical spec として記述している。
- tasks に acceptance, routing, archive preflight, observability の少なくとも4層を跨ぐ実装/検証項目が含まれている。
- self-modifying change を再現する integration or e2e verification が tasks に含まれている。
- `cflx openspec validate harden-self-modifying-phase-boundaries --strict --evidence warn` が成功する。

## Out of Scope

- 個別 change の実装修正そのもの
- すべての stalled / blocked / gated vocabulary の全面再設計
- archive promotion engine の全面刷新
