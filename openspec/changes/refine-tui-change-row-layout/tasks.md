## Implementation Tasks

- [ ] Add a process-local reducer archive-completion snapshot to the TUI state boundary. Completion requires `sync_reducer_display_caches` to collect archived IDs in the same reducer read as display statuses, apply the fact to each `ChangeState`, and preserve/reapply it when refresh reconstructs rows without adding durable or workflow-authoritative state. (verification: integration - add focused runner/state tests that drive `ChangeArchived`, observe `resolving` plus archive completion, refresh the catalog, and retain the archive-complete cache; run `cargo test --lib tui_change_row_layout_archive_sync`; verification-id: tui-change-row-layout-tests)

- [ ] Unify post-archive checkbox, hint, and mark-admission behavior around the reducer-derived archive-complete fact. Completion requires archive-complete rows to render a three-column blank checkbox placeholder, omit single/bulk mark hints, and reject Space/API bulk mark mutation as a silent no-op while pre-archive active rows remain markable; no code may classify every `resolving` row as archive-complete. (verification: integration - extend TUI and operator-command fixtures for archive-complete `resolving`, `resolve pending`, `merge wait`, `merged`, and `pushed` rows plus a pre-archive `resolving` control; run `cargo test --lib tui_change_row_layout_mark_contract`; verification-id: tui-change-row-layout-tests)

- [ ] Replace both Changes row prefixes with the shared fixed-column layout: checkbox area, one separator, 35-display-column ID content, then the next field's separator. Completion requires removing the `►` span/column, retaining Ratatui highlight styling, hard-truncating without ellipsis, padding by Unicode terminal width, and using fixed display-width constants in preview availability calculations rather than UTF-8 byte lengths. (verification: unit - add shared layout-helper and TestBackend coverage in `src/tui/render.rs` for short/long ASCII, CJK wide-character boundaries, selected/unselected rows, and narrow preview suppression; run `cargo test --lib tui_change_row_layout_render`; verification-id: tui-change-row-layout-tests)

- [ ] Update existing render regressions and add an exact representative-row test. Completion requires all cursor/ID column assertions to use the new ID start, the rendered buffer to contain no `►`, the focused row to retain highlight styling, and fixtures equivalent to `fix-stale-resolve-terminal-status` and `preserve-archiving-during-tui-refresh` to align `WT`, spinner, elapsed time, status, and task progress as specified. (verification: unit - update focused buffer tests in `src/tui/render.rs` and run `cargo test --lib tui_change_row_layout`; verification-id: tui-change-row-layout-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate refine-tui-change-row-layout --archive-gate`.

## Future Work

- Responsive omission or horizontal scrolling for terminals too narrow to show the widened fixed prefix and optional preview requires a separate UX proposal.
