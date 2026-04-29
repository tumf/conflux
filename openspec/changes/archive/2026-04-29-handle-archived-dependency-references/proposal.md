---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/parallel_run_service.rs
  - src/openspec_cmd.rs
  - src/execution/state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/classify-acceptance-followup-routing/proposal.md
  - openspec/changes/clarify-blocked-status-terminology/proposal.md
  - openspec/changes/archive/2026-04-29-separate-apply-block-from-reject/proposal.md
---

# Change: archived dependency references を analyze/validate で明示的に扱う

**Change Type**: implementation

## Premise / Context

- `~/.local/state/cflx/logs/.last-checked` 以降の cflx 実行ログでは、`clarify-blocked-status-terminology` と `classify-acceptance-followup-routing` が `dependencies: [separate-apply-block-from-reject]` を保持したまま、その依存先が active tree には存在せず archive 側にのみ存在するため、LLM analyze が `Invalid dependency reference` を返していた。
- 現行 analyzer は queued set と in-flight set だけを合法 dependency target とみなし、範囲外 ID は parse error にする（`src/analyzer.rs:631-670`）。
- しかし outer error は `Analysis returned invalid JSON ...` に再ラップされるため、実際には dependency contract failure なのに invalid JSON と見える（`src/analyzer.rs:214-238`）。
- archive 済み change の存在判定や dated archive path の解決は別実装で既に扱われており、runtime には archive state を識別する基盤がある（`src/execution/state.rs` の archive existence helpers）。
- したがって今回のギャップは「archived dependency reference を active proposal metadata に残したときの canonical contract が曖昧で、analyze/validate/log がその状態を正しく扱えない」点にある。

## Requested Artifact

- implementation proposal for making archived dependency references explicit in OpenSpec validation and analyze diagnostics
- canonical rule for whether archived dependencies are ignored as already satisfied or rejected as invalid authoring state
- regression coverage proving archived dependency references no longer surface as generic invalid JSON

## Problem / Context

active proposal が archive 済み change ID を `dependencies:` に残すことは現状起こりうる。依存先が active queue にいないこと自体は正常でありうるが、analyze phase はその参照を queued/in-flight contract 違反として hard parse error にし、その後 `invalid JSON` と誤表示する。

この挙動だと、実際の問題が

1. authoring 側で archive 後に dependency frontmatter を更新していないのか、
2. runtime が archive 済み dependency を「すでに満たされた依存」として扱うべきなのか、
3. validation が事前に止めるべきなのか

をログから判別できない。結果として scheduler は全件 parallel fallback へ落ち、根本原因が見えにくいまま再発する。

## Proposed Solution

archived dependency references に対する canonical contract を定義し、OpenSpec validation・analyzer・user-facing diagnostics を同じルールでそろえる。

- `dependencies:` に archive 済み change ID が現れたときの扱いを spec で固定する。最小方針は「archive 済み dependency は analyze target には含めず、依存充足済みとして扱うか、少なくとも generic invalid JSON ではなく dedicated diagnostics を返す」のどちらかを明示する。
- `cflx openspec validate` は active change の frontmatter dependency が active / in-flight / archived / missing のどれかを識別し、archived reference を authoring warning または canonical no-op dependency として明示的に報告する。
- analyzer は invalid dependency failure を `invalid JSON` へ潰さず、dependency contract failure として surfacing する。
- queued/in-flight closed-world rule 自体は維持しつつ、archive 済み dependency だけは dedicated branch で扱い、`separate-apply-block-from-reject` のような archived prerequisite が残っても scheduler 全体が misleading failure に落ちないようにする。
- active proposal authoring guidance も更新し、archive 後に dependency metadata をどう保守するかを repo から判断できるようにする。

## Acceptance Criteria

- active proposal が archive 済み change ID を `dependencies:` に含む場合、runtime と validation はそれを generic `invalid JSON` としては報告しない。
- analyzer / scheduler logs から、failure が JSON parse ではなく dependency contract or archived dependency handling に起因することが判別できる。
- canonical spec は archived dependency references の扱いを明文化し、authoring時に allowed/ignored/invalid の境界が分かる。
- regression tests が、archive 済み `separate-apply-block-from-reject` を参照する active proposals 相当ケースを再現し、current misleading failure が再発しないことを確認する。
- `cflx openspec validate <change-id> --strict --evidence warn` 相当の検証で、archived dependency reference に対する expected outcome が安定して観測できる。

## Explicit Completion Conditions

- `openspec/specs/parallel-execution/spec.md` と必要なら validation まわりの canonical spec に、archived dependency reference contract が requirement/scenario として追加されている。
- `src/analyzer.rs` の dependency validation と outer error shaping のどちらをどう変えるかが tasks に明記されている。
- `src/openspec_cmd.rs` または validation 経路で archived dependency references を事前に検出・分類する task が含まれている。
- queued / in-flight / archived / missing dependency の4分類をカバーする test plan が tasks に含まれている。
- active proposal metadata guidance を更新する task が含まれている。

## Out of Scope

- dependency analysis の non-LLM 化
- proposal archive 全体の自動リライト機構
- acceptance follow-up routing や blocked taxonomy 自体の再設計
