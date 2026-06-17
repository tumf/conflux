---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - openspec/specs/parallel-execution/spec.md
---

# Fix: blocked-only drain が resolve_wait の存在を無視してスケジューラを停止させる

**Change Type**: implementation

## Premise / Context

- `is_blocked_only_scheduler_state` (`queue_state.rs:2275-2287`) は `resolve_wait_changes` / `reject_wait_changes` の存在をチェックしていない
- その結果、`should_exit_when_idle` と `should_enter_persistent_idle_wait` (`orchestration.rs:43-71`) が `||` で `is_blocked_only_scheduler_state` の結果を参照するため、resolve が未完了なのにスケジューラが停止/待機してしまう
- canonical spec `parallel-execution/spec.md:61` には blocked-only drain の条件として「reducer-owned resolve/reject waiters が存在しないこと」が明記されている
- 過去に TUI ロック問題、capacity-zero 問題、スケジューラ同期ブロッキング問題の 3 回の修正が行われたが、いずれもこの特定の条件漏れには到達していなかった

## Problem / Context

`resolve_wait` 状態の change A が存在し、A に依存する change B が queued にある場合：

1. `classify_queued_work` で A → `scheduler_lane_wait`、B → `dependency_blocked` に分類される
2. `is_blocked_only()` → `true`（dispatchable が空、blocked_work が存在）
3. `is_blocked_only_scheduler_state()` → `true`（`in_flight` 空、`manual_resolve_active`=0、`pending_merge_count`=0）
4. `should_exit_when_idle` / `should_enter_persistent_idle_wait` が `true` を返す
5. スケジューラが終了（Finite）または idle wait に入る（Persistent）
6. resolve が完了すれば B は unblock されるはずなのに、analyze も dispatch も走らない

特に、`resolve_wait_changes` は空でないのに `pending_merge_count == 0`（resolve が dispatch されていない）の状態でこの問題が発生する。

## Proposed Solution

`is_blocked_only_scheduler_state` に `resolve_wait_changes` と `reject_wait_changes` が空であることのチェックを追加する。

修正箇所は 1 行のガード条件追加のみ。canonical spec の既存の条件（「reducer-owned resolve/reject waiters が存在しないこと」）に実装を合わせる修正である。

## Acceptance Criteria

- `resolve_wait_changes` または `reject_wait_changes` が空でない場合、`is_blocked_only_scheduler_state` は `false` を返す
- resolve_wait が存在する状態で dependency_blocked な change が queued にある場合、スケジューラは終了せず、resolve の dispatch または完了を待つ
- resolve 完了後、dependency_blocked だった change が unblock され、通常通り analyze → dispatch が行われる
- Persistent lifetime (TUI) では idle wait に遷移するが、resolve 完了（merge_result）で正しく wake される
- Finite lifetime ではスケジューラが早期終了しない

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` の `is_blocked_only_scheduler_state` に resolve_wait/reject_wait の空チェックが追加されている
- `src/parallel/tests/executor.rs` に、resolve_wait_changes が存在し dependency_blocked な change が queued にある状態で `is_blocked_only_scheduler_state` が `false` を返すことを検証するテストが追加されている
- `src/parallel/tests/executor.rs` に、resolve_wait 完了後に blocked だった change が dispatch されることを検証するテストが追加されている
- `cargo test` で全テストが通過すること
- `cflx openspec validate fix-blocked-only-resolve-wait-skip --strict --evidence warn` が成功すること

## Out of Scope

- resolve_wait の dispatch ロジック自体の変更
- dependency_blocked の分類ロジックの変更
- `should_dispatch_resolve_wait_retry` の条件変更
- TUI の resolve merge retry フローの変更（`fix-tui-merge-wait-clean-retry` で別途対処）
