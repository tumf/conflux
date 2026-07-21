---
change_type: implementation
priority: high
verifications:
  - id: acceptance-retry-decision-tests
    requirement: Serial and parallel acceptance retries stop on repeated findings without semantic progress
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test serial_run_service
    rerun: cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test serial_run_service
    prerequisites:
      - persist-acceptance-stalled-state is implemented
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/acceptance.rs
  - src/parallel/dispatch.rs
  - src/serial_run_service.rs
---

# Change: acceptance retry cycleを進捗ベースで制限する

**Change Type**: implementation

## Problem/Context

Acceptance FAILはrepositoryに意味のある進捗があるかにかかわらずapplyへ戻る。parallel modeは固定10 cycleで停止するが、上限到達をterminal Errorとして扱い、serial pathとの判定も一致しない。既存のpermission-denial classifierとgeneric stalled lifecycleは一部blockerを停止できるが、一般の反復findingを判定できない。

## Proposed Solution

Acceptance findingを決定的に正規化し、repository-fixableとexternalへfinding単位で分類する。FAIL後は最低1回applyを許可し、同一finding identity集合が再発してsemantic progressがなければ`repeated_acceptance_findings` stalledへ移す。

Semantic progressはsource、test、config、spec、runtime管理section外のtask変更から算出する。follow-up、blocker marker、attempt counter、logsだけの変更は進捗と数えない。serial/parallelは共通判定を使い、前回finding、semantic baseline、cycle countは先行changeのworkspace-local checkpointから復元する。10 cycle到達は`acceptance_cycle_limit_exhausted` stalledとしてworkspace-local markerへ保存する。

## Acceptance Criteria

- 初回FAIL後はrepository-fixable findingをapplyへ1回戻す。
- 同一finding集合が再発しsemantic progressがなければ、次のapply前に`repeated_acceptance_findings` stalledとなる。
- substantiveなrepository変更またはfinding集合の変化があれば、10 cycle ceiling内でretryを継続できる。
- repo-local findingとexternal blockerをfinding単位で区別し、external blockerを失わない。
- cycle 10到達はterminal Errorではなく`acceptance_cycle_limit_exhausted` stalledとなる。
- serial/parallelで同一入力が同じretry/stalled分類になる。

## Explicit Completion Conditions

- shared helperがfinding normalization、identity comparison、scope classification、semantic progress、retry decisionを提供する。
- parallel dispatchとserial serviceがshared decisionを使用する。
- repeated findingとcycle exhaustionが`persist-acceptance-stalled-state`のmarker APIへ証拠を渡す。
- unit/integration testsが初回FAIL、同一finding/no progress、real progress、changed findings、mixed blockers、cycle exhaustion、serial/parallel parityを検証する。

## Dependencies

`persist-acceptance-stalled-state`を先に実装する。stalled判定をworkspace-local evidenceなしで導入するとConstitutionに反するため。

## Out of Scope

- follow-up sectionとacceptance promptのcompact化。`compact-acceptance-retry-context`で扱う。
- generic stalled lifecycle、permission-denial classifier、follow-up persistence warning degradationの再実装。
- cycle ceiling値10の設定化。
- `acceptance_max_continues` default不一致の修正。
- verdict protocolからcompatibility tokenを除去すること。
