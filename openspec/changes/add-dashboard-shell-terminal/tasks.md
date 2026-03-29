## Implementation Tasks

- [ ] Add server-side terminal session lifecycle APIs for create/list/delete and resolve cwd from file browsing context (verification: API handlers and request/response types are present under `src/server/` or `src/web/`, with tests covering worktree cwd and base-repo cwd behavior)
- [ ] Add PTY-backed interactive terminal streaming over WebSocket for each session (verification: terminal WebSocket route exists and automated tests cover session attach, input forwarding, output streaming, and session cleanup)
- [ ] Add dashboard terminal client support using xterm.js with multi-tab session management (verification: terminal UI component(s) exist under `dashboard/src/components/`, dependency additions are present in `dashboard/package.json`, and component tests or store tests cover tab creation/switch/close behavior)
- [ ] Integrate the terminal panel into `dashboard/src/components/FileViewPanel.tsx` as a default-collapsed toggle below File Content with resizable vertical split when shown (verification: FileViewPanel renders the toggle, preserves hidden-by-default behavior, and keeps terminal sessions when collapsed in UI tests or component tests)
- [ ] Wire terminal cwd selection so worktree context uses the worktree path and change context uses the base repository path (verification: request payloads or server resolution tests assert the correct cwd for both contexts)
- [ ] Run dashboard lint/tests/build and Rust lint/tests for the terminal feature (verification: `npm run lint`, `npm run test`, `npm run build` in `dashboard/`, plus `cargo fmt --check`, `cargo clippy -- -D warnings`, and targeted `cargo test` succeed or documented failures are captured)

## Future Work

- Evaluate session persistence across browser reloads if users need long-lived shells
- Consider terminal access controls beyond existing server-mode authentication if multi-user deployments become common
