## Implementation Tasks

- [ ] Replace hand-built CLI executor assembly with the existing shared builder while preserving finite-run lifetime. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)
- [ ] Remove the duplicated analyzer closure and delegate through the existing executor-taking run method. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)
- [ ] Add a test-only accessor and regressions that directly observe `post_archive_action` and graceful-stop state on the executor returned by `create_executor_with_queue_state`. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate unify-parallel-executor-construction --archive-gate`.

## Future Work

- None.
