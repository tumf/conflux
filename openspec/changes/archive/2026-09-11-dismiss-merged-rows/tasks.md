## Implementation Tasks

- [x] Add typed process-local dismissed-merged ID state and individual/bulk confirmation payloads to `AppState`/`ModalState`, including status recheck, cursor repair, known-ID/NEW cleanup, and selected-log-filter cleanup on confirmation (verification: unit - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [x] Suppress dismissed retained terminal/merged rows at the catalog presentation boundary, while clearing dismissal and rendering any later active non-terminal catalog row with the same ID, without mutating reducer or repository state (verification: unit - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [x] Route `d`, `D`, case-insensitive `y`/`n`, and `Esc` for Changes-view dismissal while preserving Worktrees-view deletion and modal input ownership (verification: integration - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [x] Render context-aware individual/bulk hints and both confirmation overlays in the existing production TUI visual language, with deterministic tests for focused/non-focused, empty, cancel, and confirmation states (verification: integration - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)

## Future Work

- Durable cross-restart dismissal, automatic retention policies, and Web/API controls are intentionally excluded.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected checks: `cflx openspec validate dismiss-merged-rows --strict --evidence warn` and `cflx openspec validate dismiss-merged-rows --archive-gate`.
