## Implementation Tasks

- [ ] Replace `TempDir`-owned event-directory cleanup in `src/web/completion_sink.rs` with explicit path ownership and positive-acknowledgement deletion. Task-send failure and acknowledgement sender drop retain artifacts. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink`)
- [ ] Add a deterministic regression in `tests/client_completion_sink.rs` that drops dispatcher acknowledgement while an artifact exists, destroys the registry, and proves the artifact remains. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink`)
- [ ] Run focused/default tests, format, and clippy. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings`)

## Final Validation

Expected archive gate: `cflx openspec validate retain-unacknowledged-callback-artifacts --archive-gate`
