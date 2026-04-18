---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/conflict.rs
  - src/tui/runner.rs
  - src/execution/state.rs
  - src/orchestration/state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/cli/spec.md
---

# Change: fast-forward resolve 成功判定と merge-wait 復元の退行修正

**Change Type**: implementation

## Why
parallel resolve で `git merge` が fast-forward 成功しても、resolve 完了判定が merge commit 不在を失敗として扱うため、成功済み change が再試行に入り続けます。さらに refresh が archived worktree を見て `merge wait` を復元し、実際には main に取り込まれた change が未解決のように表示されます。

## What Changes
- resolve 成功判定を merge commit 必須から、fast-forward を含む「base に統合済み」の判定へ拡張する
- fast-forward で統合済みの change を `Missing merge commits` 理由で再試行しないようにする
- refresh / reducer reconciliation が merged 済み change を `merge wait` に戻さないようにする
- fast-forward resolve 成功時の回帰テストとログ/コンテキスト要件を追加する

## Acceptance Criteria
- fast-forward merge 成功後、resolve は成功として終了し、同一 change の resolve 再試行を開始しない
- merged 済み change は後続の `ChangesRefreshed` や worktree observation によって `merge wait` に戻らない
- resolve 継続理由として `Missing merge commits for change_ids` を使うのは、merge commit が本当に必要な未完了ケースに限られる
- fast-forward 成功ケースを再現するテストが追加される

## Out of Scope
- fast-forward 以外の merge conflict 解消フローの再設計
- 一般的な git sync / server mode の fast-forward 判定変更

## Impact
- Affected specs: parallel-execution, orchestration-state, cli
- Affected code: `src/parallel/conflict.rs`, `src/tui/runner.rs`, `src/orchestration/state.rs`, `src/execution/state.rs`
