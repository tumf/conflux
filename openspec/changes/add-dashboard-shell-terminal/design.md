## Context

The dashboard already supports file browsing in the right-side Files pane, and the server has one-shot command execution helpers but no interactive shell transport. This proposal adds an embedded shell terminal to the dashboard workflow without changing unrelated server-mode screens.

## Goals / Non-Goals

- Goals:
  - Provide an interactive shell directly below File Content in the Files pane
  - Support multiple concurrent terminal tabs in one dashboard session
  - Use cwd that matches the current browsing context
  - Keep the panel hidden by default and cheap when unused
- Non-Goals:
  - Terminal session persistence across reloads or restarts
  - Collaborative/shared terminals across clients
  - General-purpose terminal support outside the Files pane

## Decisions

- Decision: Use a PTY-backed server session per terminal tab and attach the browser via WebSocket
  - Rationale: a shell terminal needs interactive stdin/stdout behavior, terminal sizing, and long-lived streaming that the existing one-shot command API cannot provide
- Decision: Keep terminal sessions server-managed and keyed by session id
  - Rationale: the dashboard needs to create, switch, close, and temporarily hide tabs without losing server-side process state
- Decision: Resolve cwd from the active file browsing context on session creation
  - Rationale: the terminal should match the user workflow; worktree browsing should operate inside the selected worktree, while change browsing without a worktree should still be useful from the base repository
- Decision: Preserve sessions when the panel is collapsed
  - Rationale: the user explicitly requested show/hide behavior rather than destroy/recreate semantics
- Decision: Use xterm.js on the frontend
  - Rationale: the current dashboard has no terminal widget, and xterm.js is the standard browser terminal component with fit and link support

## Alternatives Considered

- Reuse the existing one-shot command execution API
  - Rejected because it cannot provide interactive terminal behavior or preserve shell state across commands
- Destroy sessions when the terminal panel is collapsed
  - Rejected because it breaks the expected toggle behavior and loses in-progress shell context
- Split backend terminal API and frontend panel into separate proposals
  - Rejected because the feature is only useful when both sides ship together and they share the same acceptance criteria

## Risks / Trade-offs

- PTY handling adds platform-sensitive code and process-lifecycle concerns
  - Mitigation: scope the first version to the existing server runtime assumptions and cover cleanup in tests
- Long-lived shell sessions may outlive visible UI state
  - Mitigation: explicit close API, cleanup on tab close, and no persistence across restart
- xterm.js increases dashboard bundle size
  - Mitigation: keep terminal UI lazy-initialized behind the collapsed toggle when practical

## Migration Plan

1. Add terminal session APIs and PTY streaming routes on the server
2. Add the dashboard terminal component and session/tab management
3. Integrate the panel into FileViewPanel with toggle and resize behavior
4. Verify dashboard and Rust test/lint/build flows

## Open Questions

- None for the currently requested scope
