# Change: resolve 中の MergeDeferred を自己キューしない

## Why
resolve 完了後に同一 change の resolve が自動再実行され、worktree 削除済みにより "Change directory not found" が出る誤検知が発生するため、待ち行列の挙動を明確化して防止する。

## What Changes
- resolve 実行中に `MergeDeferred` を受信した場合でも、現在 resolve 中の change は待ち行列に追加しない
- 既存の `ResolveWait` への遷移条件は維持しつつ、自己キューイングを除外する

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/state.rs`
