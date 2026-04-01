## Implementation Tasks

- [ ] 1.1 Add `pending_merge_count: Arc<AtomicUsize>` field to `ParallelExecutor` in `src/parallel/mod.rs` (verification: field exists, initialized to 0)
- [ ] 1.2 Increment `pending_merge_count` in `spawn_merge_task` before `tokio::spawn` in `src/parallel/queue_state.rs` (verification: grep shows `fetch_add` before spawn)
- [ ] 1.3 Decrement `pending_merge_count` in `handle_merge_result` in `src/parallel/queue_state.rs` (verification: grep shows `fetch_sub` in handler)
- [ ] 1.4 Add `pending_merge_count > 0` to break condition at `orchestration.rs` L164-177 (verification: break condition includes pending merge check)
- [ ] 1.5 Add `pending_merge_count > 0` to break condition at `orchestration.rs` L202-211 (verification: break condition includes pending merge check)
- [ ] 1.6 Clone `pending_merge_count` into the new `ParallelExecutor` created in `spawn_merge_task` so decrement works from inside the spawned task (verification: Arc is shared)
- [ ] 1.7 Add unit test: single change archive → scheduler waits for merge task → MergeCompleted (verification: `cargo test fix_scheduler_premature_exit` passes)
- [ ] 1.8 Run `cargo test` to verify no regressions (verification: all tests pass)
- [ ] 1.9 Run `cargo clippy -- -D warnings` (verification: no warnings)
