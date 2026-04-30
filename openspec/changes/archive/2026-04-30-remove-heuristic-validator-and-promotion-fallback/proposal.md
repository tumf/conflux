---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/proposal-session/spec.md
  - openspec/specs/proposal-session-integration/spec.md
  - openspec/specs/cli/spec.md
---

# Change: proposal validation と spec promotion からヒューリスティック推測を除去する

**Change Type**: implementation

## Premise / Context

- このセッションでは、runtime や validator が自由文や曖昧な文言から意味を推測する実装は不良であり、明示 contract に寄せるべきだという方針を確認した。
- 調査の結果、特に問題なのは `src/openspec_cmd.rs` にある (1) evidence validator の keyword-based quality inference と、(2) `delta_to_canonical(...)` の parse failure 時 fallback rewrite だった。
- evidence validator は `BEHAVIOR_TASK_KEYWORDS`, `ARTIFACT_HEAVY_TASK_KEYWORDS`, `EXECUTABLE_SURFACE_HINTS` などの語彙一致で proposal/task の意味や不足を推測している。
- `delta_to_canonical(...)` は requirement block を parse できなかった場合に、section marker を `## Requirements` へ置換して canonical spec を合成する fallback を持ち、invalid delta を deterministic に reject していない。

## Requested Artifact

- implementation proposal for removing keyword-based proposal quality inference from the native OpenSpec validator
- implementation proposal for making spec delta canonicalization fail closed instead of rewriting malformed deltas
- explicit contract that proposal validation and canonical promotion rely on structured fields / parseable syntax, not wording heuristics

## Problem / Context

Conflux の proposal validation と spec promotion は、いずれも OpenSpec authoring の基盤であるべきだが、現状は一部でヒューリスティック推測に依存している。validator は task/proposal の自然文から behavior / artifact / executable surface を推測し、promotion engine は malformed delta を deterministic に reject せず fallback rewrite で canonical 化しようとする。

この設計では、同じ意味でも書き方によって warning が変わったり、壊れた delta が parse failure として失敗せず “それっぽく” 通ったりする。結果として、仕様基盤が explicit contract を検証する層ではなく、自由文解釈や自動補修を行う層になってしまう。

## Proposed Solution

proposal validation と spec promotion を fail-closed / contract-driven に寄せ、ヒューリスティック推測や fallback rewrite を排除する。

- native validator から、behavior-changing / artifact-heavy / executable-surface を keyword で推測する quality inference を削除する。
- proposal/task の必要チェックは、明示 metadata、明示 verification ownership marker、または parseable structured field が存在するかどうかに限定する。
- `delta_to_canonical(...)` の parse failure 時 fallback rewrite を削除し、requirement block を取り出せない delta は canonical promotion 不可として deterministic error を返す。
- malformed delta や missing structure は archive/promotion 前に具体的な parse error として報告し、“section marker を置換して通す” 振る舞いをしない。
- proposal/session guidance も、validator が自由文の意味を推測する前提ではなく、必要な構造を author が明示する前提へ合わせる。

## Acceptance Criteria

- native validator は task/proposal の自然文キーワード一致だけを根拠に behavior-changing / artifact-heavy / executable-surface の品質推測 warning を出さない。
- validator が proposal quality を要求する場合、その根拠は explicit metadata、verification ownership marker、または parseable structured field に限定される。
- malformed spec delta が requirement block を parse できない場合、canonicalization は deterministic error になり、fallback rewrite で `## Requirements` へ変換して通さない。
- archive/promotion failure messaging は malformed delta を parse error として明示し、silent repair や heuristic rewrite を行わない。
- proposal/session guidance と native validator responsibility が、free-text interpretation ではなく explicit structure を前提に整合する。

## Explicit Completion Conditions

- OpenSpec delta が validator の non-heuristic contract と promotion fail-closed contract を canonical spec として記述している。
- `src/openspec_cmd.rs` から keyword-based inference helper 群を削除または no-op 化する実装タスクが tasks に含まれている。
- `delta_to_canonical(...)` の fallback rewrite 削除と malformed delta regression test が tasks に含まれている。
- `cflx openspec validate remove-heuristic-validator-and-promotion-fallback --strict --evidence warn` が成功する。

## Out of Scope

- acceptance runtime の plain-text fallback 全廃
- proposal authoring UX 全体の redesign
- OpenSpec delta grammar そのものの大規模拡張
