## Implementation Tasks

- [x] 1. Split parallel scheduler lifetime semantics by execution model so normal cflx loop-based entrypoints remain alive while the CLI `run` job path stays finite (verification: loop-based entrypoints and CLI `run` have distinct idle-exit behavior in `src/orchestrator.rs`, `src/tui/orchestrator.rs`, `src/parallel_run_service.rs`, or equivalent orchestration entrypoints)
- [x] 2. Update the loop-based parallel scheduler so it does not exit merely because `queued`, `in_flight`, and related waiting sets are temporarily empty (verification: scheduler break conditions for normal cflx execution no longer treat temporary emptiness as terminal)
- [x] 3. Ensure idle loop-based runs stay in a wait state for dynamic queue notifications and resume re-analysis when new queued changes arrive (verification: the loop-based select loop continues waiting on queue notifications while not user-stopped)
- [x] 4. Preserve queue-triggered re-analysis for retry flows, including error rows returning to queued through existing queue commands (verification: queue add paths still feed `src/parallel/queue_state.rs` and tests cover retry-to-analysis behavior)
- [x] 5. Add regression tests covering: idle loop-based scheduler stays alive, queued addition after idle triggers analysis, and CLI `run` still exits when no work remains (verification: `cargo test` targets in `src/parallel/tests` or equivalent integration coverage)
- [x] 5. Run repository validation for the behavior change (verification: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`)

## Future Work

- Manual TUI/server dogfooding to confirm the idle-running UX feels correct under real interactive use
