---
change_type: implementation
priority: high
dependencies: []
verifications:
  - id: acceptance-marker-tests
    requirement: Acceptance stalled evidence is workspace-local and safely retryable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test execution::state && cargo test parallel::dispatch
    rerun: cargo test execution::state && cargo test parallel::dispatch
    prerequisites: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/execution/state.rs
  - src/orchestration/state.rs
  - src/parallel/dispatch.rs
---

# Change: acceptance stalled stateをworkspaceへ永続化する

**Change Type**: implementation

## Problem/Context

Confluxはgeneric stalled lifecycleと`APPLY_BLOCKED/marker.md`検出を持つが、acceptance由来stalledを構造化して書く共通contractがない。Explicit retryはreducer stateをclearしてもworkspace markerをconsumeしないため、再検出後に再び停止する。一方、markerを無条件削除するとapply由来blockerを失う。

## Proposed Solution

既存apply-blocked marker contractへacceptance origin、stable reason、evidence、resumabilityを追加する。Stall前のretry判定に必要なprevious finding identities、semantic fingerprint、cycle countもnon-blocking workspace-local checkpointとして保存する。Acceptance-generated resumable markerだけをexplicit retryでconsumeできるAPIを提供し、serial/parallel ordinary dispatchはmarkerを尊重する。Reducer stateを失ってもworkspaceから同じnext actionを再構成する。

## Acceptance Criteria

- Acceptance由来stalled markerがreason、phase、finding summary/identities、retry count、semantic progress、external blockers、resumability、next actionを保持する。
- 初回FAIL後からstalled確定までのprevious finding identities、semantic baseline、cycle countをworkspace-local checkpointから復元できる。
- Runtime state削除・process restart後も同じretry/stalled decisionとnext actionをworkspaceから復元する。
- Serial/parallel ordinary dispatchはmarkerを見つけたchangeをapply/acceptance/archiveへ戻さない。
- Explicit retryはresumable acceptance-generated markerだけをconsumeする。
- Apply-generated、unknown-origin、non-resumable markerはsilentに削除しない。
- Marker write/parse/consume failureは観測可能なerrorとなり、workflow evidenceを失わない。

## Explicit Completion Conditions

- Marker schemaとbackward-compatible parserがworkspace-local state moduleに存在する。
- Acceptance marker writerとorigin-aware consumerがshared APIとして使われる。
- Restart、ordinary dispatch、explicit retry、foreign-marker preservation、malformed markerをintegration testsが検証する。
- Out-of-worktree stateをrouting inputとして追加しない。

## Out of Scope

- Repeated findingやcycle exhaustionを判定するpolicy。`bound-acceptance-retry-cycles`で扱う。
- Generic stalled lifecycle、permission classifier、terminal Error retry gateの再実装。
- Acceptance follow-upとprompt historyのcompact化。
