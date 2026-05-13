## Implementation Tasks

- [x] Extend TUI reducer event sync coverage so `src/tui/runner.rs` applies all Running lifecycle events that `src/orchestration/state.rs::OrchestratorState::apply_execution_event` uses to derive display status before calling `AppState::apply_display_statuses_from_reducer`. (verification: unit - added `should_apply_event_to_tui_reducer` coverage in `src/tui/runner.rs`, verified with `cargo test tui::runner`, including `ProcessingStarted`, `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, lifecycle completion, and failure events)

- [x] Preserve Running mode single-row queue controls for dynamically discovered changes after reducer display sync and `ChangesRefreshed` ordering. (verification: unit - added `running_not_queued_toggle_survives_reducer_sync_and_changes_refreshed` in `src/tui/state.rs`, verified with `cargo test tui::state`, covering a Running `not queued` row toggled with Space to `AddToQueue`/`queued`, then a subsequent reducer display sync and `ChangesRefreshed` that does not regress the row or lose the execution mark)

- [x] Preserve Running mode unqueue controls for queued non-active rows after reducer display sync. (verification: unit - added state and command handler coverage in `src/tui/state.rs` and `src/tui/command_handlers.rs`, verified with `cargo test tui::state` and `cargo test tui::command_handlers`, proving `Space` on a queued non-active row emits `RemoveFromQueue`, clears `selected`, updates reducer queue intent to `not queued`, and removes/marks the dynamic queue entry so later dispatch is prevented)

- [x] Preserve Running mode bulk mark/unmark behavior for eligible non-active rows while excluding active rows. (verification: unit - existing `AppState::toggle_all_marks` coverage in `src/tui/state.rs` was verified with `cargo test tui::state`, proving `x` emits `AddToQueue`/`RemoveFromQueue` for eligible `not queued`/`queued` rows in Running mode and emits no command for `applying`/`accepting`/`archiving`/`resolving` rows)

- [x] Restore header in-flight count by keeping active display statuses stable through lifecycle event sync and refresh. (verification: unit - added `running_header_count_reflects_reducer_synced_active_status_after_refresh` in `src/tui/render.rs` and verified with `cargo test tui::render`; existing render-buffer tests also prove Running header shows `Running 1`/`Running N` for active rows and does not count merely queued rows)

- [x] Verify the integrated regression path with targeted TUI tests and the repository's normal Rust checks. (verification: integration - ran targeted TUI tests as `cargo test tui::runner && cargo test tui::state && cargo test tui::command_handlers && cargo test tui::render`; ran repository normal Rust lib checks via `agent-exec run -- cargo test --lib` and `agent-exec wait fda3a4c0687ce0b68630eedb8f11342b` with exit_code 0; ran `cargo fmt --check`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-running-reducer-sync --archive-gate`
