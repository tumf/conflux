---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs:674
  - src/tui/command_handlers.rs:564
  - src/tui/orchestrator.rs:929
  - src/parallel_run_service.rs:457
  - src/parallel/orchestration.rs:151
  - src/parallel/queue_state.rs:47
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: Fix manual resolve scheduler startup with empty changes

**Change Type**: implementation

## Problem / Context

TUI の Changes view で archived change が `merge wait` のとき、ユーザーは `M` を押して manual resolve / merge retry を開始する。現在の UI は `M` 後に reducer-owned intent として `ResolveWait` を設定し、行を `resolve pending` に変える。

しかし、実ログでは `M` 後に scheduler は起動しているにもかかわらず `No committed changes available for parallel execution` で `0 change(s)` のまま終了し、`ResolveStarted` や merge retry に到達しない。

原因は、manual resolve 起動経路が `run_orchestrator_parallel(Vec::new(), ...)` を使う一方で、`ParallelRunService` が空の `changes` を通常実行の入力として扱い、executor loop に入る前に `prepare_parallel_execution()` で終了してしまうことにある。結果として、scheduler が `shared_orchestrator_state` の `ResolveWait` を同期して `retry_deferred_merges()` を呼ぶ前に停止する。

この change は、通常の parallel apply 実行に対する committed-change filter は維持しつつ、manual resolve 専用の reducer-owned `ResolveWait` 消費を空 `changes` でも実行できるようにする。

## Proposed Solution

`ParallelRunService` / scheduler 起動経路を修正し、入力 `changes` が空でも shared reducer に `ResolveWait` が存在する場合は、通常の「実行対象なし」終了にせず、executor の scheduler loop または専用 retry path に進める。

実装は次のどちらかの最小アプローチを採る。

1. `run_parallel_order_based_with_executor()` が empty changes を受け取った場合でも、shared reducer の `ResolveWait` が存在するなら executor loop を起動し、`sync_resolve_wait_from_shared_state_nonblocking()` と `maybe_dispatch_resolve_wait_retry()` が実行されるようにする。
2. または manual resolve 専用 entrypoint を追加し、committed-change filter を迂回して reducer-owned `ResolveWait` の merge retry だけを実行する。

どちらの場合も、source-of-truth は workspace/git/reducer-owned runtime state であり、外部ログや durable side state は workflow control input にしない。

## Acceptance Criteria

- `merge wait` の change に対して TUI で `M` を押したとき、`resolve pending` 表示で止まらず、scheduler が reducer-owned `ResolveWait` を消費して merge retry を開始する。
- manual resolve 起動時に `run_orchestrator_parallel(Vec::new(), ...)` 相当の空 changes 経路になっても、shared reducer に `ResolveWait` が存在する限り `No committed changes available` だけで終了しない。
- shared reducer に `ResolveWait` が存在しない空 changes の通常起動は、従来どおり安全に no-op 完了できる。
- 通常の parallel apply 実行では、コミットツリーに存在しない change や uncommitted change を除外する既存 filter の挙動を維持する。
- 実際に merge conflict がある場合は、merge retry が conflict evidence を検出し、`ResolveStarted` または適切な resolve/failure event に進む。
- conflictless merge の場合は、既存仕様どおり不要な AI conflict resolve を起動せず通常 merge completion path に進む。
- TUI 表示は reducer 由来の状態と同期し、`ResolveCompleted` / `MergeCompleted` 後に stale `resolve pending` が残らない。

## Explicit Completion Conditions

- `src/parallel_run_service.rs` または関連 scheduler 起動コードに、empty changes + reducer-owned `ResolveWait` を no-op 終了させない明示的な分岐または専用 entrypoint が存在する。
- `src/parallel/queue_state.rs` / `src/parallel/orchestration.rs` の既存 scheduler-owned retry path が、manual resolve 起動時にも到達可能であることがテストで示されている。
- `src/tui/command_handlers.rs` の `ResolveMerge` 経路は、scheduler 非稼働時に `ResolveWait` を消費できる run を起動し続ける。
- 回帰テストが、empty changes + shared reducer `ResolveWait` の条件で scheduler が即 no-op 完了せず retry dispatch へ進むことを検証している。
- 回帰テストが、empty changes + no `ResolveWait` は no-op 完了のままであることを検証している。
- 関連 Rust tests と lint/typecheck 相当の検証が成功している。少なくとも targeted tests と `cargo test` / `cargo clippy` の実行結果または明確な blocker evidence が記録されている。

## Out of Scope

- merge conflict の自動解決品質そのものの改善。
- `resolve_command` prompt 内容の変更。
- TUI 全体の mode/state refactor。
- out-of-worktree durable workflow state の追加。
- archived worktree の cleanup policy 変更。
