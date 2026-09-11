## Implementation Tasks

- [ ] Add process-local dismissed-merged ID state to `AppState`, including immediate individual/bulk removal, cursor repair, known-ID/NEW cleanup, and selected-log-filter cleanup; add no dismissal modal state (verification: unit - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [ ] Suppress dismissed retained terminal/merged rows at the catalog presentation boundary, while clearing dismissal and rendering any later active non-terminal catalog row with the same ID, without mutating reducer or repository state (verification: unit - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [ ] Route `d` and `D` as immediate Changes-view dismissal actions while preserving Worktrees-view deletion (verification: integration - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)
- [ ] Render context-aware individual/bulk hints in the existing production TUI visual language, with deterministic tests for focused/non-focused and empty states (verification: integration - `cargo test tui:: --lib`; verification-id: merged-row-dismissal-tests)

## Future Work

- Durable cross-restart dismissal, automatic retention policies, and Web/API controls are intentionally excluded.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected checks: `cflx openspec validate dismiss-merged-rows --strict --evidence warn` and `cflx openspec validate dismiss-merged-rows --archive-gate`.
