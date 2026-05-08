## Implementation Tasks

- [x] Update warning popup rendering to preserve line breaks, wrap long lines, use a diagnostics-sized modal, and show a footer hint. (verification: unit - add or update TUI render-focused coverage where available, or a focused helper test, so multi-line popup content is converted/rendered without losing line boundaries; completion: `src/tui/render.rs` renders `WarningPopup.message` as line-preserving content with wrapping and a footer such as scroll/close instructions.)

- [x] Add popup-local scroll state with lifecycle reset. (verification: unit - add or update `src/tui/state.rs` tests or neighboring TUI state tests to show new warning popups start at scroll offset zero and clearing/reopening does not retain stale scroll; completion: `AppState` or an equivalent TUI state owner stores warning popup scroll offset, initializes it to zero, and resets it whenever a new popup is shown or cleared.)

- [x] Route popup scroll keys before normal TUI navigation while the warning popup is visible. (verification: unit - add or update `src/tui/key_handlers.rs` tests to prove scroll keys adjust popup scroll and do not clear the popup, while `Esc` clears it; completion: `src/tui/key_handlers.rs` handles `Up`/`Down` or `k`/`j` plus page scroll keys for visible warning popups without moving the underlying change cursor/log scroll, and handles explicit close keys to dismiss the popup.)

- [x] Preserve multi-line `on_merged` hook failure text in popup state and logs. (verification: unit - extend `src/tui/state/event_handlers/errors.rs::handle_on_merged_hook_failed_surfaces_merge_wait_not_merged` or add a neighboring test using a multi-line error string and asserting the popup message contains the expected newline-separated diagnostics; completion: `src/tui/state/event_handlers/errors.rs` keeps the full error string, including embedded newlines, in `WarningPopup.message`, and still records the full message in the log.)

- [x] Verify no workflow-control state is introduced. (verification: manual - review `src/tui/render.rs`, `src/tui/state.rs`, and `src/tui/key_handlers.rs` diff to confirm new state is UI-only and complies with `openspec/CONSTITUTION.md` workspace-local workflow state law; completion: popup scroll/readability state is limited to TUI presentation and is not used by orchestration, acceptance, archive, merge, or resume routing.)

- [x] Run repository verification for the implemented change. (verification: integration - run targeted Rust tests such as `cargo test tui::` or the narrower module tests covering TUI warning popup behavior, plus the repository's configured lint/typecheck/test commands; completion: relevant Rust unit tests pass, followed by the repository lint/typecheck/test commands identified from project scripts or documentation.)

## Future Work

- Consider a separate WebUI proposal if server-mode toast errors need a dedicated expandable or pre-wrapped error dialog.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate improve-warning-popup-readability --archive-gate`
