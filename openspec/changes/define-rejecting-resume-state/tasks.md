## Implementation Tasks

- [x] `ExecutionEvent` に `RejectionReviewCompleted { change_id, outcome: RejectionOutcome }` を追加する。`RejectionOutcome` は `Confirm` / `Resume` の 2 値 enum (verification: `cargo test --lib events`)
- [x] `ExecutionEvent` に `RejectionReviewFailed { change_id, error }` を追加する (verification: コンパイル成功)
- [x] `src/orchestration/state.rs` の `apply_execution_event` に `RejectionReviewCompleted` ハンドラを追加: `Confirm` → `activity: Idle, terminal: Rejected`, `Resume` → `activity: Applying, wait_state: None` (verification: `cargo test --lib orchestration::state`)
- [x] `src/orchestration/state.rs` に `RejectionReviewFailed` ハンドラを追加: `activity: Idle, terminal: Error` (verification: reducer unit test)
- [x] `src/parallel/dispatch.rs` の rejecting フロー完了後に `RejectionReviewCompleted` イベントを送出する (verification: `cargo test --lib parallel::dispatch`)
- [x] `invariants_hold()` テストで `Rejecting + terminal = None + no completion event` の組み合わせが禁止されることを検証する (verification: `cargo test --lib orchestration::state`)
- [x] `display_status()` の既存テストに rejecting → applying / rejecting → rejected の遷移パスを追加する (verification: `cargo test --lib orchestration::state`)

## Future Work

- Serial モードでの Rejecting サポート定義
