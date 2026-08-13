## Implementation Tasks

- [x] In `src/web/completion_sink.rs`, remove the timeout-only cleanup fallback and make event-directory cleanup conditional on dispatcher acknowledgement that cancellation and child reap settled. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink`)
- [x] Add a deterministic delayed-acknowledgement regression test in `tests/client_completion_sink.rs` proving the event artifact remains until reap acknowledgement and no post-cancellation delivery starts. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink`)
- [x] Run focused/default tests, format, and clippy. (verification-id: completion-sink-reap-cleanup) (verification: integration - `cargo test --test client_completion_sink && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings`)

## Notes

- The removed fallback is only observable by outlasting it, so
  `tests/client_completion_sink.rs` also carries a heavy
  (`--features heavy-tests`) test that holds reap acknowledgement past the old
  10s grace. Verified both ways: it fails when that fallback is reinstated
  ("no elapsed time authorizes removing a live callback's artifact") and passes
  against the fix.
- `certify` now stops re-reading the repository once shutdown cancels. The
  acknowledgement `owner_stopping` waits for has no deadline of its own any
  more, and that verification loop runs on the same dispatcher, so an
  uncancellable retry chain would have held shutdown open behind it.

## Final Validation

- `cargo test --test client_completion_sink`: ok, 18 passed in 1.03s
- `cargo test`: ok, all suites passed (3863 passed in the lib suite)
- `cargo test --features heavy-tests --test client_completion_sink shutdown_`: ok, 4 passed
- `cargo fmt --all -- --check`: clean
- `cargo clippy --all-targets --all-features -- -D warnings`: clean
- `cflx openspec validate fix-completion-sink-reap-cleanup --strict`: passed

Expected archive gate: `cflx openspec validate fix-completion-sink-reap-cleanup --archive-gate`
