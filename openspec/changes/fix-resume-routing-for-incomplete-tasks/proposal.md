---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/execution/archive.rs
  - openspec/specs/parallel-execution/spec.md
---

# Change: tasks 未完了の resumed workspace を apply へ戻す

**Change Type**: implementation

## Premise / Context

- 現セッションでは、別 repo の implementation change を Conflux で resume すると、未完了 tasks が残っているにもかかわらず Apply ではなく Acceptance に入り、その通常フローとして Acceptance > Archive を経て Archive error になる事例が共有された。
- 現行の parallel execution spec は「tasks が complete なら acceptance / archive」系の条件は持つが、resumed implementation workspace で tasks 未完了なら Apply に戻す requirement は明示していない。
- `src/parallel/executor.rs` の archive guard は tasks incomplete を拒否するが、これは誤って Acceptance に入った後の遅い失敗であり、resume routing 自体の誤りを防げていない。
- Conflux の責務は OpenSpec workflow の phase routing であり、implementation tasks 未完了の change を acceptance/archive フェーズへ進めないことは Conflux 側の修正スコープである。

## Problem / Context

implementation change の resumed workspace で `tasks.md` に未完了 implementation task が残っていても、resume routing が Apply ではなく Acceptance を選ぶと、通常の Acceptance > Archive フローを通って archive 側で tasks incomplete error になる。

この挙動では、まだ実装を続けるべき change が品質ゲート・archive フェーズへ誤進入し、ユーザーは「resume したのに作業再開ではなく後段エラーに落ちる」状態になる。

## Proposed Solution

Conflux は resumed implementation workspace の routing で task completeness を最優先 gate として扱い、unchecked implementation task が残る限り Apply に戻す。

具体的には:

1. resume routing 時に `tasks.md` の implementation task 完了状況を確認する。
2. unchecked implementation task が残っている implementation change は Acceptance や Archive へ進めず Apply にルーティングする。
3. tasks 完了後のみ、既存の durable acceptance state と workspace state を使って Acceptance / Archive routing を評価する。
4. routing 理由として `tasks incomplete; rerouting resumed workspace to apply` 相当の観測可能なログを出す。

## Acceptance Criteria

- resumed implementation workspace は、`tasks.md` に未完了 implementation task が残る限り Acceptance にルーティングされない。
- その場合 workspace は Apply に戻される。
- tasks 完了後は既存どおり durable acceptance state と workspace state に基づいて Acceptance > Archive routing が行われる。
- tasks 未完了 change を resume しても、Archive 側の tasks incomplete error に到達する前に Apply へ戻る。
- 回帰テストで incomplete tasks の resume が Apply を選び、completed tasks の resume が既存 routing を維持することを確認できる。

## Out of Scope

- tasks parser format の全面再設計
- spec-only change の別 routing policy 追加
- acceptance/archive quality gate 内容の変更
