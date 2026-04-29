---
change_type: implementation
priority: high
dependencies:
  - separate-apply-block-from-reject
references:
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/queue_state.rs
  - src/parallel/workspace.rs
  - src/parallel/dispatch.rs
  - src/parallel/mod.rs
  - src/vcs/git/mod.rs
  - src/events.rs
  - openspec/changes/classify-acceptance-followup-routing/proposal.md
---

# Change: dependency 解消後に stale worktree を再利用せず作り直す

**Change Type**: implementation

## Premise / Context

- 現行 canonical spec は、依存制約が解決した change は実行開始時点で worktree を新規作成し、既存 worktree があっても作り直すことを要求している（`openspec/specs/parallel-execution/spec.md`）。
- `classify-acceptance-followup-routing` は `separate-apply-block-from-reject` に依存しており、依存側が完了した後に古い worktree をそのまま resume すると stale base 上で作業を継続しうる。
- 実装上は `force_recreate_worktree` フィールドが存在するが、依存解消時にそのフラグを設定・消費する end-to-end 経路が確認できず、現状は既存 worktree 再利用に偏っている。
- 依存解消後の stale worktree reuse は、spec と実装の不整合であるだけでなく、依存 change の成果物を前提とした follow-up proposal の resume correctness を損なう。

## Requested Artifact

- implementation proposal for dependency-resolved worktree recreation
- explicit runtime behavior for when stale worktrees must be discarded versus safely resumed
- verification coverage proving dependency-unblocked changes do not resume from stale worktrees

## Problem / Context

Conflux の並列実行は dependency blocked change を queue 上で保留にできるが、依存解消後にその change を再開するとき、既存 worktree の扱いが spec どおりに強制再作成されていない。結果として、依存前に作られた stale worktree がそのまま resume され、依存 change が base branch に持ち込んだ差分・状態遷移・blocking semantics を取り込まないまま apply/acceptance/archive に進みうる。

このギャップは特に「依存 change の実装が downstream proposal の前提を変える」ケースで問題になる。ユーザー期待は「依存が解消したら downstream は最新 base でやり直す」だが、現実装は `find_existing_workspace()` / `reuse_workspace()` に流れる可能性があるため、resume correctness が保証されない。

## Proposed Solution

依存 blocked → resolved の change について、次回 dispatch 時に stale worktree を再利用しない explicit recreation path を導入する。

- scheduler / queue state は、dependency blocked だった change が resolved になった瞬間に「次回 dispatch は forced recreation 必須」であることを runtime state に記録する。
- workspace 取得ロジックは、その forced recreation mark がある change では既存 worktree を reuse せず、新規 worktree を作成する。
- 既存 worktree が存在する場合は、dispatch 前または create 前に cleanup され、古い branch/worktree が downstream resume source として残らないようにする。
- forced recreation は dependency-unblocked change に限定し、通常の resume path まで壊さない。
- log / event / UI wording は「dependency resolved → fresh worktree recreation」を観測できるようにし、単なる generic resume と区別する。

## Acceptance Criteria

- dependency blocked だった change が resolved になった後の初回 dispatch では、既存 worktree が存在しても resume されず fresh worktree が使われる。
- stale worktree が残っていた場合、runtime はその change の fresh dispatch 前に cleanup または equivalent invalidation を実行し、古い worktree を downstream state detection source にしない。
- forced recreation は dependency-unblocked path に限定され、依存に関係ない通常の resume path は従来どおり existing worktree reuse を継続できる。
- logs / events / tests は「dependency blocked 解除後の fresh recreation」と「通常 resume」を区別して観測できる。
- `classify-acceptance-followup-routing` のような dependency-coupled change が、依存側完了後に stale base ではなく最新 base 前提で再開されることを検証できる。

## Explicit Completion Conditions

- OpenSpec delta が dependency-resolved change の forced worktree recreation rule、cleanup timing、resume exception boundary を canonical spec として記述している。
- `src/parallel/queue_state.rs`, `src/parallel/workspace.rs`, `src/parallel/dispatch.rs`, `src/parallel/mod.rs`, `src/vcs/git/mod.rs` を中心に、forced recreation mark の生成・消費・cleanup の実装責務が tasks に明記されている。
- dependency blocked → resolved → dispatch の統合テスト、ならびに通常 resume 非退行テストが tasks に含まれている。
- `cflx openspec validate fix-dependency-resolved-worktree-recreation --strict --evidence warn` が成功する。

## Out of Scope

- 依存解消後に worktree 差分を自動 rebase / merge して救済する高度な再利用戦略
- dependency blocked 以外の blocked/rejecting/resolving state に対する resume policy 全面再設計
- stale worktree 可視化 UI の最終 polish
