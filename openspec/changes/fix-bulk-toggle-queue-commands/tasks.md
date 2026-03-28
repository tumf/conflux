## Implementation Tasks

- [ ] Refactor `src/tui/state.rs` bulk toggle handling so Running mode can return per-row queue commands instead of mutating only local selection state (verification: code path emits `TuiCommand::AddToQueue` / `RemoveFromQueue` for eligible rows)
- [ ] Update `src/tui/key_handlers.rs` to send all commands returned by bulk toggle when `x` is pressed in Changes view (verification: `x` key path forwards emitted commands through `cmd_tx`)
- [ ] Preserve existing guard behavior for active, `MergeWait`, `ResolveWait`, and parallel-ineligible rows during bulk toggle (verification: targeted unit tests in `src/tui/state.rs` cover excluded rows and non-queue-only states)
- [ ] Add regression tests for Running mode bulk toggle queue add/remove behavior and ensure Select/Stopped mode semantics remain intact (verification: `cargo test bulk_toggle` or equivalent targeted test names in `src/tui/state.rs`)
- [ ] Validate formatting, lint, and full test behavior after implementation (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- Manual TUI smoke test to confirm visible queued/not-queued transitions match emitted reducer state during interactive execution
