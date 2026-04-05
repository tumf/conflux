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

implementation change の resumed workspace で `tasks.md` に未完了 task が残っていても、resume routing が Apply ではなく Acceptance を選ぶと、通常の Acceptance > Archive フローを通って archive 側で tasks incomplete error になる。

根本原因は resume routing の tasks 判定スコープと archive guard の tasks 判定スコープの不一致にある。resume routing (`src/parallel/dispatch.rs` の `read_implementation_task_progress`) は `## Implementation Tasks` セクション配下の checkbox のみを数えるが、archive guard (`src/task_parser.rs` の `parse_content`) はファイル全体の checkbox を数える。このため `## Acceptance #N Failure Follow-up` 等の追加セクションに未完了 checkbox が残っていても resume routing は「tasks 完了」と判断して Acceptance に送り、archive guard が「tasks 未完了」で拒否する。

## Proposed Solution

Conflux は resume routing の tasks 判定スコープを archive guard と一致させ、ファイル全体の未完了 checkbox がある限り Apply に戻す。

具体的には:

1. resume routing の task completeness 判定を `## Implementation Tasks` セクション限定から、archive guard と同じファイル全体スコープ（`task_parser::parse_content` 相当）に変更する。
2. ファイル全体で unchecked task が残っている implementation change は Acceptance や Archive へ進めず Apply にルーティングする。
3. tasks 完了後のみ、既存の durable acceptance state と workspace state を使って Acceptance / Archive routing を評価する。
4. routing 理由として `tasks incomplete; rerouting resumed workspace to apply` 相当の観測可能なログを出す。

## Acceptance Criteria

- resumed implementation workspace は、`tasks.md` にファイル全体で未完了 task が残る限り Acceptance にルーティングされない。
- その場合 workspace は Apply に戻される。
- tasks 完了後は既存どおり durable acceptance state と workspace state に基づいて Acceptance > Archive routing が行われる。
- resume routing の tasks 判定スコープと archive guard の tasks 判定スコープが一致しており、routing は通すが archive で落ちる不整合が起きない。
- tasks 未完了 change を resume しても、Archive 側の tasks incomplete error に到達する前に Apply へ戻る。
- 回帰テストで incomplete tasks の resume が Apply を選び、completed tasks の resume が既存 routing を維持することを確認できる。

## Out of Scope

- tasks parser format の全面再設計
- spec-only change の別 routing policy 追加
- acceptance/archive quality gate 内容の変更
