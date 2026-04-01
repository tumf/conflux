## Implementation Tasks

- [x] 1.1 Add `pending_merge_count: Arc<AtomicUsize>` field to `ParallelExecutor` in `src/parallel/mod.rs` (verification: field exists, initialized to 0)
- [x] 1.2 Increment `pending_merge_count` in `spawn_merge_task` before `tokio::spawn` in `src/parallel/queue_state.rs` (verification: grep shows `fetch_add` before spawn)
- [x] 1.3 Decrement `pending_merge_count` in `handle_merge_result` in `src/parallel/queue_state.rs` (verification: grep shows `fetch_sub` in handler)
- [x] 1.4 Add `pending_merge_count > 0` to break condition at `orchestration.rs` L164-177 (verification: break condition includes pending merge check)
- [x] 1.5 Add `pending_merge_count > 0` to break condition at `orchestration.rs` L202-211 (verification: break condition includes pending merge check)
- [x] 1.6 Clone `pending_merge_count` into the new `ParallelExecutor` created in `spawn_merge_task` so decrement works from inside the spawned task (verification: Arc is shared)
- [x] 1.7 Add unit test: single change archive → scheduler waits for merge task → MergeCompleted (verification: `cargo test fix_scheduler_premature_exit` passes)
- [x] 1.8 Run `cargo test` to verify no regressions (verification: all tests pass)
- [x] 1.9 Run `cargo clippy -- -D warnings` (verification: no warnings)
