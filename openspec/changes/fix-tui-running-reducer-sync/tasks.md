## Implementation Tasks

- [ ] Extend TUI reducer event sync coverage so `src/tui/runner.rs` applies all Running lifecycle events that `src/orchestration/state.rs::OrchestratorState::apply_execution_event` uses to derive display status before calling `AppState::apply_display_statuses_from_reducer`. (verification: unit - add or update a focused test in `src/tui/runner.rs` or an extracted helper module, runnable with `cargo test tui::runner`, that fails if `ProcessingStarted`, `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, lifecycle completion, and failure events are omitted)

- [ ] Preserve Running mode single-row queue controls for dynamically discovered changes after reducer display sync and `ChangesRefreshed` ordering. (verification: unit - add or update TUI state tests in `src/tui/state.rs` or `src/tui/state/selection_logic.rs`, runnable with `cargo test tui::state`, covering a Running `not queued` row toggled with Space to `AddToQueue`/`queued`, then a subsequent reducer display sync and `ChangesRefreshed` that must not regress the row or lose the execution mark)

- [ ] Preserve Running mode unqueue controls for queued non-active rows after reducer display sync. (verification: unit - add or update TUI command/state tests in `src/tui/command_handlers.rs` or `src/tui/state/selection_logic.rs`, runnable with `cargo test tui::command_handlers tui::state`, proving `Space` on a queued non-active row emits `RemoveFromQueue`, clears `selected`, updates reducer queue intent to `not queued`, and prevents later dynamic-queue dispatch)

- [ ] Preserve Running mode bulk mark/unmark behavior for eligible non-active rows while excluding active rows. (verification: unit - add or update `AppState::toggle_all_marks` coverage in `src/tui/state.rs`, runnable with `cargo test tui::state`, proving `x` emits `AddToQueue`/`RemoveFromQueue` for eligible `not queued`/`queued` rows in Running mode and emits no command for `applying`/`accepting`/`archiving`/`resolving` rows)

- [ ] Restore header in-flight count by keeping active display statuses stable through lifecycle event sync and refresh. (verification: unit - add or update `src/tui/render.rs` render-buffer tests, runnable with `cargo test tui::render`, proving Running header shows `Running 1`/`Running N` for active rows and does not count merely queued rows)

- [ ] Verify the integrated regression path with targeted TUI tests and the repository's normal Rust checks. (verification: integration - run targeted `cargo test tui::runner tui::state tui::command_handlers tui::render` tests that cover runner/state/render regressions; run `cargo test --lib` or document why a narrower command is required if default tests exceed the repository heavy-test policy)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-running-reducer-sync --archive-gate`
