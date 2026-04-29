---
change_type: implementation
priority: high
dependencies:
  - separate-apply-block-from-reject
references:
  - src/parallel/dispatch.rs
  - src/task_parser.rs
  - src/serial_run_service.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/changes/align-archive-readiness-failure-reporting/proposal.md
---

# Change: acceptance follow-up を phase-aware に分類して resume/apply stall を防ぐ

**Change Type**: implementation

## Premise / Context

- 現行の resumed workspace routing は `tasks.md` の総チェックボックス進捗だけを見て `Applied` state から `Apply` を強制している。
- acceptance failure は `## Acceptance #<n> Failure Follow-up` に unchecked checkbox を追記するため、archive readiness blocker や commit-path blocker も「implementation tasks incomplete」と同列に扱われる。
- 実ログでは `add-running-agents-restart-button` が、実装完了後に archive commit blocker だけ未解消の状態で `Apply` へ戻され、空 WIP commit 連続による stall error へ入った。
- `align-archive-readiness-failure-reporting` は archive failure の root cause 表示を扱うが、acceptance follow-up をどの phase に戻すかまでは扱っていない。
- `separate-apply-block-from-reject` は resumable `Blocked` state の導入を進めており、この change はその blocked lifecycle を acceptance follow-up routing に接続する。
- 直近ではこの proposal 自体がローカル環境のディスク不足で reject されたが、change の妥当性が失われたわけではなく、環境修復後に再開可能な temporary blocker であることが確認された。

## Requested Artifact

- implementation proposal for resume/apply routing and acceptance follow-up classification
- no new archive-readiness proposal; existing active change already covers archive failure reporting

## Problem / Context

Conflux は acceptance failure の follow-up を「未完了 checkbox があるかどうか」でしか解釈していない。そのため、repo 実装差分を伴う remediation task と、archive readiness / commit-path / external unblock のような non-implementation blocker が同じ apply-driving work として扱われる。

この設計では、resume 時に `Implementation Tasks` が完了していても `Acceptance #<n> Failure Follow-up` に blocker checkbox が残っているだけで `Apply` へ戻る。agent 側が「この周回の apply では差分を作れない」と判断すると空 WIP commit が蓄積し、stall detector が早期 error を出す。ユーザー視点では「error になったが再実行すると進む」揺らぎに見えるが、実際には phase routing が blocker 種別を誤分類している。

## Proposed Solution

acceptance follow-up を phase-aware に分類し、resume/apply routing が raw checkbox count ではなく **apply-driving remediation** と **non-implementation blocker** を区別できるようにする。

- acceptance follow-up の canonical format を拡張し、repo 実装差分が必要な remediation は unchecked checkbox として保持し、archive readiness / commit-path / external unblock のような blocker-only finding は non-progress note として別扱いにする。
- `task_parser` と runtime helper は follow-up section を section-aware に解析し、`Implementation Tasks` と apply-driving remediation task のみを「apply に戻す理由」として数える。
- resumed workspace が `Applied` で、未解決項目が blocker-only follow-up だけの場合、runtime は `Apply` を強制せず、依存 proposal で導入する `Blocked` hold か、同等の non-apply routing へ送る。
- acceptance fail → next cycle routing も同じ分類を使い、non-implementation blocker だけで empty WIP stall loop に再突入しないようにする。
- blocked / rejected の判断基準を spec・design・コードコメントで明文化し、環境修復や依存解消で再開可能な temporary blocker（例: `No space left on device`, commit-path blocker, archive readiness blocker, external approval待ち）は `Blocked` として保持し、change 自体を閉じるべき前提破綻・superseded・closure妥当ケースのみ `Rejected` に送る。
- log / event wording も更新し、`implementation tasks incomplete` ではなく、`acceptance follow-up requires apply remediation` か `blocker-only follow-up remains` かを区別して表示する。

## Acceptance Criteria

- `Acceptance #<n> Failure Follow-up` に repo 実装差分が必要な remediation と blocker-only finding の両方が混在しても、resume/apply routing は apply-driving task の有無で次 step を決められる。
- `Implementation Tasks` 完了後に blocker-only follow-up だけが残る resumed workspace は、unchecked checkbox 総数だけを理由に `Apply` へ戻されない。
- acceptance failure が commit-path blocker または archive readiness blocker のみを記録したケースでは、runtime は empty WIP commit を増やすためだけの `Apply` 再実行を行わない。
- acceptance failure が実装 remediation を含むケースでは、従来どおり `Apply` に戻って修正作業を継続できる。
- user-visible logs / events / tests は、`implementation incomplete` と `blocker-only follow-up` を区別して観測できる。
- disk exhaustion や一時的なローカル検証不能のように、change 妥当性を壊さず環境修復後に再開可能な failure は `Rejected` ではなく `Blocked` として分類される。

## Explicit Completion Conditions

- OpenSpec delta が acceptance follow-up classification、resume routing、stall suppression expectations を canonical spec として記述している。
- `src/task_parser.rs` と `src/parallel/dispatch.rs` を中心に、section-aware follow-up parsing と apply-driving progress 判定の追加先が tasks に明記されている。
- `add-running-agents-restart-button` 型の「implementation complete + archive blocker only」ケースを再現する Rust test coverage が tasks に含まれている。
- `Blocked` lifecycle を使う場合の dependency relationship (`separate-apply-block-from-reject`) が proposal metadata または本文で明示されている。
- `cflx openspec validate classify-acceptance-followup-routing --strict --evidence warn` が成功する。

## Out of Scope

- archive failure root-cause 表示そのものの改善
- evidence heuristics (`proposal_has_runtime_behavior` など) の文言判定ロジック再設計
- acceptance verdict protocol 全体の再設計
