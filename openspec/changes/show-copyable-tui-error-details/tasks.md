## Implementation Tasks

- [x] Update change-level error synchronization so every local or remote TUI-visible `error` row retains the full final diagnostic and never substitutes an operator-facing placeholder such as `reducer`; reducer-retained `TerminalState::Error` or remote API `error_detail` data MUST replace stale cached non-error reasons, while transitions away from `error` clear the presentation cache. Completion requires state/event-handler tests for direct events, reducer synchronization over a pre-existing skip reason, remote detail projection or explicit unavailable fallback, and retry/status transition clearing. (verification: unit - `cargo test tui::state`; verification-id: tui-error-details-tests)
- [x] Refactor the shared Changes-row preview selection minimally so Select and Running views prefer `Error: <error_message_cache>` for `error` rows, fall back to `Error details unavailable` only when no diagnostic exists, and otherwise preserve the latest-log preview behavior. Completion requires render tests proving error precedence after matching logs are evicted, non-error compatibility, readable error styling, minimum-width omission, and Unicode-safe truncation. (verification: unit - `cargo test tui::render`; verification-id: tui-error-details-tests)
- [x] Add an Error Details popup opened by `Enter` on an `error` row, showing the change ID and complete retained diagnostic with wrapping, popup-local scrolling, visible `c: copy`/scroll/`Esc` hints, and input ownership over the underlying Changes and Logs views; show `Enter: details` in the Changes-panel hints when the cursor is on an error row. Completion requires key-handler and render tests for open, scroll isolation, warning-popup priority, preserved `Ctrl+C`, close, hint visibility, and unchanged `Enter` behavior on non-error rows. (verification: integration - `cargo test tui`; verification-id: tui-error-details-tests)
- [x] Add the smallest cross-platform clipboard integration needed to copy `Change: <id>\nError: <diagnostic>` from the popup, using an injectable test double and no shell-command fallback; keep the popup open and show inline success or actionable failure feedback. Completion requires deterministic tests for exact copied text, success feedback, failure feedback, and no developer clipboard mutation during tests. (verification: unit - `cargo test tui`; verification-id: tui-error-details-tests)
- [x] Run the repository TUI test target plus the project-provided formatting, lint, and compile/typecheck gates, fixing only regressions caused by this change. Completion requires successful command output recorded in the implementation handoff. (verification: integration - `cargo test tui`; verification-id: tui-error-details-tests)

## Future Work

- Arbitrary terminal text selection, error-history browsing, log export, and WebUI parity remain separate changes.

## Notes

- The proposal's "remote TUI mode" wording predates `remove-multi-project-server-mode`: `--server` is rejected at parse time (`src/cli.rs` `removed_tui_server_options_are_rejected`) and no TUI client consumes `/api/v2`. The requirement was satisfied at the only place it still exists — `OrchestratorState::all_error_details` sanitizes with `crate::events::sanitize_detail`, the same helper `apply_reducer_derived_operator_state` uses for the API `error_detail`, and `tui_error_detail_matches_the_api_projected_error_detail` pins the two to the same text. The "unavailable" fallback is `ERROR_DETAILS_UNAVAILABLE`.
- The `reducer` placeholder is gone from `apply_display_statuses_from_reducer`; the diagnostic now arrives only through `apply_error_details_from_reducer`, wired into `sync_reducer_display_caches` alongside the display-status and blocker-view syncs.
- `arboard` is added with `default-features = false`: only plain text is ever copied, so the image-data support would be dead weight.
- evidence: `cargo test --lib tui::` — `test result: ok. 570 passed; 0 failed; 1 ignored`
- evidence: `cargo fmt --all -- --check` exit 0
- evidence: `cargo clippy --locked --all-targets --all-features -- -D warnings` exit 0
- evidence: verification ran with `CARGO_TARGET_DIR=/tmp/cflx-tui-error-details` because the repository's shared cargo target dir is contended by other worktrees

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate show-copyable-tui-error-details --archive-gate`
