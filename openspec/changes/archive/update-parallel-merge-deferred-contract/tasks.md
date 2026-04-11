## Implementation Tasks

- [x] Canonicalize parallel merge deferred specs into a single resolve-priority contract across `parallel-merge`, `parallel-execution`, and `orchestration-state` deltas (verification: the new spec deltas describe one consistent rule for resolve-active deferral, dirty-base manual wait, and scheduler lifetime)
- [x] Replace string-based auto-resumable detection in the parallel merge pipeline with an explicit merge result contract (verification: `src/parallel/merge.rs` and `src/parallel/queue_state.rs` no longer derive `auto_resumable` via `reason.contains("Resolve in progress")`)
- [x] Align reducer / queue handling so auto-resumable deferred merges consistently enter `ResolveWait` and manual intervention cases remain `MergeWait` (verification: `src/orchestration/state.rs` and the parallel queue/merge flow agree on wait-state transitions)
- [x] Add or update regression tests for resolve-active deferral, dirty-base manual wait, archive-incomplete deferral, and scheduler non-exit while pending merge tasks remain (verification: targeted tests under `src/parallel/tests/` and scheduler/state tests cover all canonical branches)
- [x] Run proposal validation and Rust quality gates for the implementation (verification: `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-parallel-merge-deferred-contract --strict`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- Consider whether `MergeAttempt::Deferred` should eventually carry a typed enum reason usable by Web/TUI presentation without string formatting concerns.
