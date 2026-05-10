## Implementation Tasks

- [ ] Add transient TUI state for in-flight worktree deletions keyed by canonical/display path, with helper methods to mark, clear, and query deletion progress. (verification: unit - tests in `src/tui/state.rs` or `src/tui/state/worktree_action_logic.rs` assert marking a path makes `is_worktree_deleting(path)` true and clearing it makes the query false; completion condition: the state is process-local and not serialized or read by orchestration reducers/schedulers)
- [ ] Mark the selected worktree as deleting immediately when delete confirmation is accepted via `Y` and when skip-teardown deletion is accepted via `S`. (verification: unit - tests in `src/tui/state.rs` and/or `src/tui/key_handlers.rs`, expected filter `cargo test worktree_delete`, assert the marker is set before `TuiCommand::DeleteWorktreeByPath` is sent for both normal and skip-teardown paths; completion condition: a stubbed slow delete command is not required for the marker to become visible)
- [ ] Render deletion progress in Worktrees view rows and footer/status text. (verification: unit - render tests in `src/tui/render.rs` assert a deleting target row includes `[Deleting...]` and footer/status text includes `Deleting worktree` while the marker is active; completion condition: tests fail if rendering ignores the deleting marker)
- [ ] Suppress duplicate and target-row operations while a worktree is deleting. (verification: unit - tests in `src/tui/state/worktree_action_logic.rs` and/or `src/tui/key_handlers.rs` assert duplicate `D` delete is rejected with `Worktree is already being deleted`, and merge/shell/editor actions for that selected row do not emit their normal command while the marker is active; completion condition: no second `DeleteWorktreeByPath` can be emitted for the same path while marked deleting)
- [ ] Clear deletion progress after `DeleteWorktreeByPath` completes on both success and failure without changing existing success, branch warning, refresh, or failure popup behavior. (verification: unit/integration - tests in `src/tui/command_handlers.rs` use success and failure stubs or isolated command paths to assert the marker is cleared in both branches and existing log/popup expectations remain; completion condition: failed deletion leaves the worktree retryable and successful deletion still refreshes the worktree list)
- [ ] Keep deletion progress UI-only and non-authoritative. (verification: integration - inspect `src/tui/state.rs`, `src/tui/key_handlers.rs`, `src/tui/command_handlers.rs`, `src/tui/render.rs`, `src/orchestration/state.rs`, and `src/parallel/*` after implementation, then run focused tests; completion condition: no scheduler dispatch, resume routing, acceptance, archive, or next-action decision reads the deleting marker)
- [ ] Run repository verification for touched Rust code. (verification: integration - run focused tests such as `cargo test worktree_delete` plus relevant render/state filters, then run configured lint/typecheck commands such as `cargo fmt --check` and `cargo clippy` when available; completion condition: commands pass or failures are documented with exact output and rationale)

## Future Work

- If operators later need teardown script output or progress percentages, create a separate proposal for streamed deletion progress.
- If Web UI manual deletion has the same feedback problem, create a separate Web UI scoped proposal.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate show-worktree-delete-progress --archive-gate`.
