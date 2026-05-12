## Implementation Tasks

- [ ] Update `src/tui/state/event_handlers/completion.rs` so `handle_merge_completed()` can close an active TUI resolve lifecycle when `MergeCompleted` is the success event for a manual merge retry. Completion condition: the handler clears stale `AppState::is_resolving` when appropriate while still marking the completed row `merged`. (verification: unit - add/update tests in `src/tui/state/event_handlers/completion.rs` and run `cargo test merge_completed`.)

- [ ] Factor the resolve-queue drain behavior used by `handle_resolve_completed()` so `MergeCompleted` can dispatch the next queued resolve retry without duplicating inconsistent logic. Completion condition: both `ResolveCompleted` and eligible `MergeCompleted` paths set the next queued row to `resolve pending` and return `TuiCommand::ResolveMerge(next_change_id)` when a queued change exists. (verification: unit - add/update tests in `src/tui/state/event_handlers/completion.rs` and run `cargo test resolve_queue`.)

- [ ] Update `src/tui/state/event_handlers/mod.rs` so the `OrchestratorEvent::MergeCompleted` arm returns any follow-up command produced by merge completion handling. Completion condition: `app.handle_orchestrator_event(OrchestratorEvent::MergeCompleted { ... })` can return `Some(TuiCommand::ResolveMerge(...))` for queued resolve work. (verification: unit - add/update an event-handler test in `src/tui/state/event_handlers/mod.rs` or adjacent TUI state tests and run `cargo test handle_orchestrator_event`.)

- [ ] Add a regression test for the original stuck-pending path: after a manual retry completes via `MergeCompleted`, a later `M` press on another `merge wait` row must take the immediate command path. Completion condition: the test sets up stale-prone state, processes `MergeCompleted`, selects a `merge wait` row, calls `resolve_merge()`, and observes `Some(TuiCommand::ResolveMerge(...))` rather than `None`. (verification: unit - add/update a test in `src/tui/state.rs` and run `cargo test resolve_merge`.)

- [ ] Preserve non-resolve merge completion behavior. Completion condition: ordinary `MergeCompleted` still marks the row `merged`, records elapsed/progress data when available, and logs `Merge completed for '<change_id>'`. (verification: unit - add/update a focused non-resolve merge completion assertion in `src/tui/state/event_handlers/completion.rs` and run `cargo test merge_completed`.)

- [ ] Run targeted verification for the affected modules. Completion condition: targeted Rust tests covering `tui::state` / event handlers pass locally. (verification: integration - run the narrowest available `cargo test` filters for the new TUI tests and record the command/output in the implementation notes.)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-merge-completed-resolve-flag --archive-gate`

## Future Work

- Manual TUI smoke test: run the TUI with two archived `merge wait` changes, press `M` on one, let it complete via merge, then press `M` on the second while other apply/archive work is present and confirm it transitions through `resolve pending` to `resolving` or a visible defer/failure state.
