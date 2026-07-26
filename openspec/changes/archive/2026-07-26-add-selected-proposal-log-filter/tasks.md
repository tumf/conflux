## Implementation Tasks

- [x] Add a non-persistent selected-proposal log-filter flag to `AppState`, default it to `false`, resolve the current target from `cursor_index`, and reset to newest auto-scroll when toggling or when cursor movement changes an active target. (verification: unit - focused `src/tui/state.rs` and `src/tui/state/log_logic.rs` tests assert default-off, toggle, cursor-follow reset, empty changes, and unchanged `AppState::logs`)
- [x] Wire `f` in the Changes view to toggle the filter without modifying proposal execution marks, and preserve existing behavior in other views and for existing keys. (verification: unit - `src/tui/key_handlers.rs` tests send `f` in Changes and Worktrees views and assert filter, selection, cursor, and log-panel state)
- [x] Filter entries by exact structured `change_id` before `render_logs` performs wrapping, line counts, visible ranges, and scroll calculations; exclude entries with no proposal identity and safely render zero matches. (verification: unit - `src/tui/render.rs` tests render mixed, global-only, and zero-match buffers and assert only matching text and filtered range/scroll state are visible)
- [x] Show the `f` operation and current filter state in the Logs panel title, including the selected proposal when width permits and a compact equivalent for constrained widths. (verification: unit - `src/tui/render.rs` title and buffer tests assert discoverable off/on hints and the active target without regressing existing PageUp/PageDown, Home/End, and `l` guidance)
- [x] Attach structured `change_id` metadata to every proposal-specific lifecycle, completion, skip, stop, and error log created under `src/tui/state/event_handlers/` when its source event carries a proposal ID; retain global logs as unscoped and avoid message parsing. (verification: unit - tests in `src/tui/state.rs` and `src/tui/state/event_handlers/{processing,completion,errors}.rs` assert representative start, completion, failure, skip, and stop entries have exact IDs while global completion/analysis entries remain unscoped)
- [x] Run repository quality gates and keep default tests below the project's one-second unit-test threshold; mark only genuinely unavoidable slow coverage as heavy. (verification: integration - `Makefile` quality gates pass via `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`, including the focused tests in `src/tui/state.rs`, `src/tui/key_handlers.rs`, and `src/tui/render.rs`)

## Future Work

- Add proposal identity to remote server stdout/stderr only if remote execution can provide a truthful per-proposal association; project-only entries remain hidden by this filter.
- Add multi-proposal, operation, level, or text filters only after separate user demand.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-selected-proposal-log-filter --archive-gate`
