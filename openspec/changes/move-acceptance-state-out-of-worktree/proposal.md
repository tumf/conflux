---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/acceptance_state.rs
  - src/parallel/executor.rs
  - src/parallel/dispatch.rs
  - src/parallel/merge.rs
  - openspec/specs/parallel-execution/spec.md
---

# Change: worktree 内の acceptance state ファイル生成を廃止する

**Change Type**: implementation

## Premise / Context

- 現セッションでのユーザー要求は一貫して `.cflx/acceptance-state.json` による dirty worktree が merge を妨げる点の解消であり、ignore 方法の工夫ではなく「そのファイルを作らないこと」が本題である。
- 現行実装 `src/parallel/acceptance_state.rs` は workspace/worktree 配下の `.cflx/acceptance-state.json` に durable acceptance state を保存している。
- `src/parallel/merge.rs` の merge 前判定は `git status --porcelain` ベースの dirty worktree 検出に依存しており、worktree 内に生成される internal artifact も merge defer 要因になりうる。
- parallel execution spec は acceptance state の durable persistence 自体は要求しているが、その保存先を worktree 内に限定していない。
- したがって、要求の本質は「durable acceptance state は維持しつつ、Git worktree を汚さない保存先へ移すこと」である。

## Problem / Context

`.cflx/acceptance-state.json` は acceptance resume / archive guard のための durable state として導入されたが、保存先が worktree 内であるため、Git から見ると uncommitted/untracked artifact になりうる。

この artifact は ignore 設定の有無にかかわらず merge readiness の根本リスクであり、dirty worktree を理由に merge が defer/blocked される。ユーザー要求は internal state artifact を隠すことではなく、そもそも worktree に生成しないことにある。

## Proposed Solution

Conflux は durable acceptance state を維持しつつ、その保存先を worktree 外の Conflux 管理領域へ移し、worktree 配下には `.cflx/acceptance-state.json` を生成しない。

推奨保存先は `~/.local/state/cflx/acceptance-state/` 配下とし、workspace absolute path を主キーにした外部 state ファイルまたは同等の外部 persistence を使う。

具体的には:

1. acceptance state persistence API を worktree path 基準のファイル保存から、`~/.local/state/cflx/acceptance-state/` 配下の外部ストア基準へ置き換える。
2. state key は workspace absolute path を主キーとし、change_id・revision・updated_at を payload に含める。
3. apply 完了 / acceptance 開始 / PASS / FAIL 時の state 更新は従来どおり維持するが、保存先は Git 管理外の Conflux state 領域とする。
4. resume routing と archive guard は新しい外部 persistence から acceptance state を読み、現行の durable semantics を保つ。revision mismatch や stale state は archive 解放条件として扱わない。
5. archive 完了または workspace cleanup 完了後は対応する外部 acceptance state を削除または無効化し、長期残留 state が次回実行に干渉しないようにする。
6. worktree 配下には `.cflx/acceptance-state.json` を作成しないことを保証する。
7. merge readiness と dirty worktree 判定の回帰テストで、Conflux 生成 acceptance state artifact が merge の妨げにならないことを確認する。

## Acceptance Criteria

- Conflux は parallel workspace/worktree 配下に `.cflx/acceptance-state.json` を生成しない。
- durable acceptance state は `~/.local/state/cflx/acceptance-state/` 相当の worktree 外管理領域に保持され、resume routing と archive guard は現行同等の `pending` / `running` / `passed` / `failed` semantics を維持する。
- state key は workspace absolute path ベースで安定して再解決でき、revision mismatch または stale pass では archive が解放されない。
- acceptance 実行後も Conflux 自身が生成した acceptance state artifact によって merge が dirty worktree 扱いで defer されない。
- 再起動後も interrupted acceptance は archive に進まず、外部 persistence に基づいて acceptance 再実行へ戻る。
- archive 完了または workspace cleanup 完了後は外部 acceptance state が削除または無効化される。
- 回帰テストで worktree 配下に acceptance state ファイルが存在しないことと、merge readiness が internal artifact で壊れないことを確認できる。

## Out of Scope

- durable acceptance state の概念自体を削除すること
- acceptance/archive guard の判定ロジックを spec から外すこと
- `.cflx/` 配下の他ファイル一般の整理
