---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/dependency.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
  - src/parallel/tests/manual_resolve.rs
verifications:
  - id: resolve-dependency-dispatch-tests
    requirement: Dependent apply dispatch remains blocked until its resolving dependency is repository-visibly integrated, while unrelated work can use free capacity
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test parallel::tests::executor && cargo test parallel::tests::manual_resolve
    rerun: cargo test parallel::tests::executor && cargo test parallel::tests::manual_resolve
    prerequisites: []
---

# Change: resolve中の依存関係をdispatch gateへ保持する

**Change Type**: implementation

## Problem/Context

Parallel schedulerはresolve処理中でも空きslotへ独立changeをdispatchできる。この並列性自体は必要だが、resolve中のchangeに依存するqueued changeは、依存changeがeffective dependency baseへ統合されたrepository-visible evidenceを得るまでdispatchしてはならない。

実運用で`persist-acceptance-stalled-state`のresolve完了前に、同changeへ明示依存する`bound-acceptance-retry-cycles`のapplyが開始された。`bound-acceptance-retry-cycles`のproposal metadataは依存を宣言しており、canonical parallel-execution specもarchive evidenceだけでは依存充足にならずeffective dependency baseへのmerge evidenceを要求している。したがって、全resolveを排他的に扱うのではなく、dependency classification、resolve lifecycle state、effective-base merge checkをdispatch直前まで一貫させる必要がある。

## Proposed Solution

- schedulerのdependency contextで、active resolveまたはresolve-wait中のchangeを未解決依存として保持する。
- analyzerのorder/dependency出力だけを信頼せず、dispatch selection時にproposal metadataとrepository-visible effective-base merge evidenceを再確認する。
- resolve開始からmerge完了までdependent applyをfail-closedで抑止し、resolve完了通知後の再分析で初めてdispatch可能にする。
- resolve対象と依存関係のないchangeは、既存どおり空きslotへdispatchできる。
- blocked diagnosticsは既存deduplication経路を使用し、同一状態を繰り返し出力しない。

## Acceptance Criteria

- resolve中のchangeへ直接依存するqueued changeは、依存changeがeffective dependency baseへ統合されるまでapplyを開始しない。
- 依存changeのarchive directoryが存在しても、merge evidenceがなければdependentはblockedのままとなる。
- resolve完了後にrepository-visible merge evidenceが確認できると、dependentは再分析を経てdispatch可能になる。
- resolve対象と依存関係のないqueued changeは、利用可能slotがあればresolve中でもdispatchできる。
- dependency metadata、resolve lifecycle state、merge evidenceの取得失敗または不整合はfail-closedとなり、dependent applyを開始しない。
- 同一dependency blockerに対するoperator-visible diagnosticは既存のdeduplication規則に従う。

## Explicit Completion Conditions

- `src/parallel/dependency.rs`と`src/parallel/queue_state.rs`のdispatch gateが、resolve中および未統合dependencyを一貫して未解決と分類する。
- schedulerがdependentを選択する直前にrepository-visible effective-base merge evidenceを確認し、analyzer出力だけで依存充足を決めない。
- integration testsが、依存resolve中のdependent抑止、独立changeの並列dispatch、resolve完了後のdependent dispatchを実際のscheduler eventで検証する。
- regression testが`ApplyStarted`を観測し、stub、no-op、単なるclassification helperの変更では通らない。
- `cargo test parallel::tests::executor`と`cargo test parallel::tests::manual_resolve`が成功する。

## Out of Scope

- resolve中の全apply dispatchを停止するglobal exclusion。
- max parallelismまたはslot accountingの意味変更。
- OpenSpec dependency metadata形式の変更。
- serial modeの拡張。
- acceptance retry policy自体の変更。
