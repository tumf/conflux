---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/execution/state.rs
  - openspec/specs/parallel-execution/spec.md
  - ~/.local/state/cflx/logs/conflux-bda270b8/2026-04-06.log
---

# Change: Applied resume must complete acceptance before archive handoff

**Change Type**: implementation

## Premise / Context

- 現在のセッションでは `fix-rejected-marker-removal-reactivation` worktree が `Applied` 状態で resume された後、archive guard が `durable acceptance-pass state missing` で失敗している。
- durable acceptance state の実体は `~/.local/state/cflx/acceptance-state/afefe686ceb30b6efae7df23d9afa131.json` で、対象 revision `c2fda72...` に対して `state: failed` が記録されている。
- ログでは `Resume route forcing acceptance ...` / `state=Applied -> Acceptance` と判定されているにもかかわらず、その直後に archive guard failure が発生している。
- 既存 archived proposal `fix-parallel-acceptance-resume-archive-bypass` は archive 前提条件の強化を扱っているが、今回の再発は `ResumeAction::Acceptance` から archive handoff までの実行整合性が保たれていない点に集中している。

## Problem / Context

parallel resume では、`Applied` workspace に対して durable acceptance-pass state が存在しない revision は acceptance に戻すべきである。しかし実際には `ResumeAction::Acceptance` が選ばれても、その cycle で archive フェーズに到達してしまい、archive guard によって `durable acceptance-pass state missing` で失敗することがある。

この挙動は resume routing と実際の phase 実行を食い違わせ、ユーザに「acceptance を再実行すべき状態なのに archive failed になる」という誤解を与える。さらに、archive guard を最終防衛線としては使えていても、dispatch control flow が acceptance→archive handoff 条件を満たしていないことを示している。

## Proposed Solution

`ResumeAction::Acceptance` が選ばれた cycle では、current revision に対する durable acceptance-pass が acceptance 処理内で確認されるまで archive に進ませないことを明示的に保証する。

具体的には:

1. `Applied` resume の acceptance 経路では、acceptance が `Pass` を返した場合にのみ apply/acceptance loop を抜けて archive handoff する。
2. durable acceptance state が `failed` / `missing` / `stale` の revision では、archive phase への到達を dispatch 側で禁止する。
3. `ResumeAction::Acceptance` を選んだ cycle で archive を試みた場合は、制御フロー不整合としてログ・テストで検知可能にする。
4. resume routing のログと実際に開始した phase を一致させ、`state=Applied -> Acceptance` の後には acceptance 実行が観測できるようにする。
5. archive guard は最終防衛線として維持するが、archive 誤遷移の主抑止は dispatch control flow 側で担保する。

## Acceptance Criteria

- `Applied` workspace の current revision に durable acceptance-pass state が存在しない場合、resume 後に archive phase は開始されない。
- `ResumeAction::Acceptance` が選ばれた cycle では、acceptance が `Pass` を返すまで archive handoff しない。
- durable acceptance state が `failed` の revision に対して archive guard failure を先に出すのではなく、acceptance 再実行へ進む。
- `state=Applied -> Acceptance` の resume ログが出たケースでは、同 cycle 内で acceptance 実行ログが観測できる。
- 回帰テストで `Applied + failed durable state`, `Applied + missing durable pass`, `Applied + passed durable pass` の 3 経路が検証される。

## Out of Scope

- durable acceptance state の保存先やファイル形式の再設計
- archive guard 自体の撤廃
- serial モードの resume 制御変更
