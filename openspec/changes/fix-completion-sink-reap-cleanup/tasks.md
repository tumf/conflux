## Implementation Tasks

- [ ] In `src/web/completion_sink.rs`, remove the timeout-only cleanup fallback and make event-directory cleanup conditional on dispatcher acknowledgement that cancellation and child reap settled. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink`)
- [ ] Add a deterministic delayed-acknowledgement regression test in `tests/client_completion_sink.rs` proving the event artifact remains until reap acknowledgement and no post-cancellation delivery starts. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink`)
- [ ] Run focused/default tests, format, and clippy. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings`)

## Final Validation

Expected archive gate: `cflx openspec validate fix-completion-sink-reap-cleanup --archive-gate`
