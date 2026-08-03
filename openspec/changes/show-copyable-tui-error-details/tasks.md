## Implementation Tasks

- [ ] Update change-level error synchronization so every TUI-visible `error` row retains the full final diagnostic and never substitutes an operator-facing placeholder such as `reducer`; preserve clearing on transitions away from `error` and keep this state presentation-only. Completion requires state/event-handler tests for direct events, reducer synchronization, and retry/status transition clearing. (verification: unit - `cargo test tui::state`; verification-id: tui-error-details-tests)
- [ ] Refactor the shared Changes-row preview selection minimally so Select and Running views prefer `Error: <error_message_cache>` for `error` rows, fall back to `Error details unavailable` only when no diagnostic exists, and otherwise preserve the latest-log preview behavior. Completion requires render tests proving error precedence after matching logs are evicted, non-error compatibility, readable error styling, minimum-width omission, and Unicode-safe truncation. (verification: unit - `cargo test tui::render`; verification-id: tui-error-details-tests)
- [ ] Add an Error Details popup opened by `Enter` on an `error` row, showing the change ID and complete retained diagnostic with wrapping, popup-local scrolling, visible `c: copy`/scroll/`Esc` hints, and input ownership over the underlying Changes and Logs views. Completion requires key-handler and render tests for open, scroll isolation, close, and unchanged `Enter` behavior on non-error rows. (verification: integration - `cargo test tui`; verification-id: tui-error-details-tests)
- [ ] Add the smallest cross-platform clipboard integration needed to copy `Change: <id>\nError: <diagnostic>` from the popup, using an injectable test double and no shell-command fallback; keep the popup open and show inline success or actionable failure feedback. Completion requires deterministic tests for exact copied text, success feedback, failure feedback, and no developer clipboard mutation during tests. (verification: unit - `cargo test tui`; verification-id: tui-error-details-tests)
- [ ] Run the repository TUI test target plus the project-provided formatting, lint, and compile/typecheck gates, fixing only regressions caused by this change. Completion requires successful command output recorded in the implementation handoff. (verification: integration - `cargo test tui`; verification-id: tui-error-details-tests)

## Future Work

- Arbitrary terminal text selection, error-history browsing, log export, and WebUI parity remain separate changes.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate show-copyable-tui-error-details --archive-gate`
