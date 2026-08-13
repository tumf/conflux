## Implementation Tasks

- [x] Replace `TempDir` Drop cleanup in `src/web/completion_sink.rs` with randomized exclusive `TempDir::keep()` path ownership, preserve `0700`, and call `remove_dir_all` only for `Ok(Ok(()))` positive acknowledgement. Pre-deadline sender drop, post-cancellation sender drop, and task-send failure retain artifacts and emit one path-only bounded warning. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink`)
- [x] Add a hook-free deterministic regression in `tests/client_completion_sink.rs`: start callback/dispatcher on runtime A, prove callback start, drop runtime A, call shutdown on runtime B, and prove the pre-deadline sender-drop path retains the event file and directory. Do not add a public dispatcher abort/kill hook. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink`)
- [x] Update `AGENTS.md` completion-sink guidance to state that missing reap acknowledgement retains the owner-private directory and logs its path rather than risking cleanup beneath a live callback. (verification-id: unacknowledged-callback-artifact-retention) (verification: unit - `python3 -c "from pathlib import Path; s=Path('AGENTS.md').read_text(); assert 'reap acknowledgement' in s and 'retained' in s"`)
- [x] Run focused/default tests, format, and clippy. (verification-id: unacknowledged-callback-artifact-retention) (verification: integration - `cargo test --test client_completion_sink && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings`)

## Final Validation

- `cargo test --test client_completion_sink`: ok, 19 passed / 0 failed (0.96s), including `shutdown_retains_artifacts_when_the_reap_acknowledgement_is_dropped`.
- `cargo test`: every target ok, 0 failed (lib 3863 passed / 17 ignored).
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean.
- `AGENTS.md` retention guidance check passed.

Expected archive gate: `cflx openspec validate retain-unacknowledged-callback-artifacts --archive-gate`
