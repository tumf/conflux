---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/state.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/parallel/acceptance_state.rs
  - src/parallel/archive_state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-events/spec.md
---

# Change: Eliminate out-of-worktree durable workflow state

**Change Type**: implementation

## Premise / Context

- ユーザは「workspace の状態は workspace だけで判定できなければならない」「durable state は完全禁止・有害」と明示している。
- 現行 parallel resume / archive 制御は `~/.local/state/cflx/acceptance-state` と `~/.local/state/cflx/archive-resume-state` を読み、resume routing と archive retry/resume context の判断材料にしている。
- `src/execution/state.rs` 自体は file/git state ベースの workspace state detection を担っている一方で、`src/parallel/dispatch.rs` と `src/parallel/executor.rs` が worktree 外 state を追加参照している。
- canonical spec `openspec/specs/parallel-execution/spec.md` にも「outside the worktree に persist する」要件が残っており、基本設計原則と矛盾している。

## Inferred Request

- workflow state としての out-of-worktree durable state を全面撤廃する proposal を作る。
- resume / archive / acceptance の判定を workspace 内 file/git evidence のみで完結させる。
- 外部 state が削除されても resume behavior が変化しないことを仕様・実装・検証で担保する。

## Problem / Context

parallel orchestration の resume routing は、本来 `detect_workspace_state` が返す workspace-local state を起点にすべきである。しかし現状は `Applied` / `Archiving` 経路で worktree 外 durable state を読んでおり、同じ workspace を別マシンへコピーした場合や `~/.local/state/cflx/**` を削除した場合に routing 結果が変わりうる。

この hidden state は、再現性・可搬性・デバッグ容易性を損なう。さらに canonical spec まで out-of-worktree persistence を要求しているため、将来の実装でも同じ設計逸脱が再生産される。workspace state machine の正本を workspace 自身へ閉じ込め、out-of-worktree data は workflow control input として使えないことを仕様で固定する必要がある。

## Proposed Solution

workspace resume / archive / acceptance gating から out-of-worktree durable workflow state を完全に除去する。

具体的には:

1. `~/.local/state/cflx/acceptance-state` と `~/.local/state/cflx/archive-resume-state` を workflow state として参照・更新するコードを削除する。
2. `Applied` resume は workspace-local evidence だけで archive handoff 可否を決める。workspace 内に acceptance completion を証明する十分な evidence がない場合は archive へ進まず acceptance を再実行する。
3. `Archiving` / `Archived` resume は current workspace の file/git state のみで判定し、archive retry reason persistence を routing 条件に使わない。
4. `~/.local/state/cflx/**` 配下の削除有無が resume behavior を変えないことを regression test で固定する。
5. canonical spec から outside-the-worktree durable workflow state の要求を除去し、「workspace state は workspace-local evidence だけで判定する」を明文化する。
6. ログや observability 用の外部出力が残る場合でも、それらは workflow control input に使わないことを明示する。

## Acceptance Criteria

- runtime は workspace の次アクションを、対象 workspace の file state / git state / base branch tree comparison だけで決定できる。
- `~/.local/state/cflx/acceptance-state` と `~/.local/state/cflx/archive-resume-state` を削除しても resume routing 結果は変化しない。
- `Applied` workspace で workspace-local acceptance completion evidence が不足している場合、resume は archive ではなく acceptance 再実行へ進む。
- `Archiving` / `Archived` workspace の判定は workspace-local evidence だけで成立し、stale external state が apply / acceptance / archive の再突入を引き起こさない。
- canonical spec から out-of-worktree durable workflow state の要求が削除され、workspace-local-only principle が requirement と scenario で表現される。
- regression tests が、同一 workspace に対して external state の有無で routing が変わらないことを検証する。

## Explicit Completion Conditions

- `src/parallel/acceptance_state.rs` と `src/parallel/archive_state.rs` の workflow-state responsibility が削除または非使用化され、resume / archive control path から参照されない。
- `src/parallel/dispatch.rs` / `src/parallel/executor.rs` / `src/execution/state.rs` に workspace-local-only routing が実装されている。
- external durable state directory を事前に作成 / 削除した両ケースで同一 routing を確認する自動テストが追加されている。
- `cflx openspec validate eliminate-durable-workflow-state --strict --evidence warn` が成功する。
- proposal 実装後の lint / test で workspace-local-only routing regression が検出可能になっている。

## Out of Scope

- serial モードの別設計刷新
- ログや metrics の保存場所そのものの全面禁止
- workflow state 撤廃と独立な archive UX 改善の追加実装
