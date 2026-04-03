## Implementation Tasks

- [ ] 1. Split parallel scheduler lifetime semantics by frontend context so TUI/server sessions remain alive while CLI `run` stays finite (verification: the TUI/server path and CLI `run` path have distinct idle-exit behavior in `src/parallel_run_service.rs`, `src/tui/orchestrator.rs`, or equivalent orchestration entrypoints)
- [ ] 2. Update the TUI/server parallel scheduler so it does not exit merely because `queued`, `in_flight`, and related waiting sets are temporarily empty (verification: scheduler break conditions for TUI/server no longer treat temporary emptiness as terminal)
- [ ] 3. Ensure idle TUI/server runs stay in a wait state for dynamic queue notifications and resume re-analysis when new queued changes arrive (verification: the TUI/server select loop continues waiting on queue notifications while not user-stopped)
- [ ] 4. Preserve queue-triggered re-analysis for retry flows, including error rows returning to queued through existing TUI commands (verification: `src/tui/command_handlers.rs` AddToQueue path still feeds `src/parallel/queue_state.rs` and tests cover retry-to-analysis behavior)
- [ ] 5. Add regression tests covering: idle TUI/server scheduler stays alive, queued addition after idle triggers analysis, and CLI `run` still exits when no work remains (verification: `cargo test` targets in `src/parallel/tests` or equivalent integration coverage)
- [ ] 5. Run repository validation for the behavior change (verification: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`)

## Future Work

- Manual TUI/server dogfooding to confirm the idle-running UX feels correct under real interactive use
