## Implementation Tasks

- [ ] Pass the already captured startup repository root into `AppState` as display-only project identity, without re-reading process current directory during rendering. (verification: unit - focused state/render setup in `src/tui/render.rs`; verification-id: tui-project-path-header)
- [ ] Replace the TUI header workspace concurrency/backend badge with the captured project path while preserving lifecycle status, dirty badge, version alignment, and bounded narrow-terminal behavior. (verification: unit - `cargo test tui_header_shows_project_path --locked`; verification-id: tui-project-path-header)
- [ ] Update existing header regression assertions that currently require `[workspaces:...]` so they require the project path and explicitly reject the retired badge. (verification: unit - `cargo test tui_header_shows_project_path --locked`; verification-id: tui-project-path-header)

## Future Work

Path shortening or home-directory substitution can be proposed separately if full paths prove too wide in practice.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected command: `cflx openspec validate show-project-path-in-tui-header --archive-gate`.
