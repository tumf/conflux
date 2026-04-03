## Implementation Tasks

- [ ] 1. Update parallel scheduler lifetime semantics in `src/parallel/orchestration.rs` so the loop does not exit merely because `queued`, `in_flight`, and related waiting sets are temporarily empty (verification: scheduler break conditions no longer treat temporary emptiness as terminal)
- [ ] 2. Ensure idle parallel runs stay in a wait state for dynamic queue notifications and resume re-analysis when new queued changes arrive (verification: `src/parallel/orchestration.rs` select loop continues waiting on queue notifications while not user-stopped)
- [ ] 3. Preserve queue-triggered re-analysis for retry flows, including error rows returning to queued through existing TUI commands (verification: `src/tui/command_handlers.rs` AddToQueue path still feeds `src/parallel/queue_state.rs` and tests cover retry-to-analysis behavior)
- [ ] 4. Add regression tests covering: idle scheduler stays alive, queued addition after idle triggers analysis, and scheduler exits only on user stop/cancel (verification: `cargo test` targets in `src/parallel/tests` or equivalent integration coverage)
- [ ] 5. Run repository validation for the behavior change (verification: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`)

## Future Work

- Manual TUI/server dogfooding to confirm the idle-running UX feels correct under real interactive use
