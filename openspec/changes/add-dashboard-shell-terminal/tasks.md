## Implementation Tasks

- [x] Add server-side terminal session lifecycle APIs for create/list/delete and resolve cwd from file browsing context (verification: API handlers and request/response types are present under `src/server/`, with tests covering worktree cwd and base-repo cwd behavior)
- [x] Add PTY-backed interactive terminal streaming over WebSocket for each session (verification: terminal WebSocket route exists and automated tests cover session attach, input forwarding, output streaming, and session cleanup)
- [x] Add dashboard terminal client support using xterm.js with multi-tab session management (verification: terminal UI component(s) exist under `dashboard/src/components/`, dependency additions are present in `dashboard/package.json`, and component tests or store tests cover tab creation/switch/close behavior)
- [x] Integrate the terminal panel into `dashboard/src/components/FileViewPanel.tsx` as a default-collapsed toggle below File Content with resizable vertical split when shown (verification: FileViewPanel renders the toggle, preserves hidden-by-default behavior, and keeps terminal sessions when collapsed in UI tests or component tests)
- [x] Wire terminal cwd selection so worktree context uses the worktree path and change context uses the base repository path (verification: request payloads or server resolution tests assert the correct cwd for both contexts)
- [x] Run dashboard lint/tests/build and Rust lint/tests for the terminal feature (verification: `npm run lint`, `npm run test`, `npm run build` in `dashboard/`, plus `cargo fmt --check`, `cargo clippy -- -D warnings`, and targeted `cargo test` succeed or documented failures are captured)

## Acceptance #1 Failure Follow-up

- [x] Fix `TerminalPanel.tsx:62` — `useCallback` dependency array references undefined variable `cwd`; should be `[projectId, root, isCreating]` so the callback updates when project context changes

## Future Work

- Evaluate session persistence across browser reloads if users need long-lived shells
- Consider terminal access controls beyond existing server-mode authentication if multi-user deployments become common
