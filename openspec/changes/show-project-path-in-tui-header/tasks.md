## Implementation Tasks

- [ ] Pass the already captured startup repository root into `AppState` as display-only project identity, without re-reading process current directory during rendering. (verification: unit - focused state/render setup in `src/tui/render.rs`; verification-id: tui-project-path-header)
- [ ] Add a dependency-free, terminal-display-width-aware middle-elision helper that reserves one column for `…`, retains prefix and suffix, gives the suffix the odd spare column, and handles budgets too small for both sides. (verification: unit - focused ASCII, wide-Unicode, combining-mark, exact-fit, and tiny-budget cases run by `cargo test tui_header_shows_project_path --locked`; verification-id: tui-project-path-header)
- [ ] Replace the TUI header workspace concurrency/backend badge with the captured project path, deriving its width budget after status and version reservation, while preserving dirty badge, version alignment, and bounded narrow-terminal behavior. (verification: unit - `cargo test tui_header_shows_project_path --locked`; verification-id: tui-project-path-header)
- [ ] Update existing header regression assertions that currently require `[workspaces:...]` so they require a full or deterministically middle-elided project path and explicitly reject the retired badge. (verification: unit - `cargo test tui_header_shows_project_path --locked`; verification-id: tui-project-path-header)

## Future Work

Home-directory substitution or path-component-aware rewriting can be proposed separately if conventional middle elision proves insufficient.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected command: `cflx openspec validate show-project-path-in-tui-header --archive-gate`.
