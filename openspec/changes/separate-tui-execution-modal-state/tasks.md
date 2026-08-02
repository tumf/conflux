## Implementation Tasks

- [ ] Replace the mixed TUI `AppMode` with separate execution and modal enums, migrate `AppState` initialization and state accessors, and remove `previous_mode` without introducing a compatibility field that preserves mixed-state behavior. (verification: unit - `cargo test tui::state:: tui::types::` covers default state and typed transition helpers; verification-id: tui-state-tests)

- [ ] Rewire QR, worktree-delete, and force-kill modal transitions so opening, canceling, confirming, closing, and event-driven invalidation set or clear only modal state and safely clear associated pending payloads where required. (verification: unit - `cargo test tui::state:: tui::key_handlers::` proves round trips from every execution mode and rejects stale confirmation payloads; verification-id: tui-state-tests)

- [ ] Update key and command routing to consume warning-popup and modal input before ordinary view input, and derive bulk-mark admission from Changes view, modal absence, and the shared `OperatorMode` lifecycle matrix with distinct Error and Stopping feedback. (verification: integration - `cargo test tui::key_handlers:: tui::state::selection_logic:: tui::command_handlers::` proves modal keys cannot mutate cursor, marks, queue intent, stop state, or retry state and covers the full execution-mode matrix; verification-id: tui-state-tests)

- [ ] Migrate execution event handlers to mutate only execution state, preserving current change-local versus fatal-global error classification and existing AllCompleted/Stopped terminal-retention behavior while allowing valid overlays to survive background execution transitions. (verification: unit - `cargo test tui::state::event_handlers::` covers ProcessingError, fatal Error, Stopping, Stopped, AllCompleted, and modal-present transitions; verification-id: tui-state-tests)

- [ ] Update TUI rendering to select base content from execution and view state and render modal overlays independently, retaining existing titles, key hints, layouts, and warning-popup behavior for every supported execution/modal combination. (verification: unit - `cargo test tui::render::` asserts underlying execution presentation and overlay output without fallback mode rewrites; verification-id: tui-state-tests)

- [ ] Update typed lifecycle snapshots and projection to report confirmations as blocked, QR as the underlying execution lifecycle, and execution-only canonical mode tokens without changing external adapter ownership or Web operator semantics. (verification: unit - `cargo test tui::lifecycle::` covers all execution modes crossed with no modal, QR, delete confirmation, and force-kill confirmation; verification-id: tui-state-tests)

- [ ] Run the complete TUI regression suite and resolve compiler, lint, and formatting failures introduced by the state migration without broadening behavior beyond this proposal. (verification: integration - `cargo test tui:: && cargo fmt --check && cargo clippy -- -D warnings`; verification-id: tui-state-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate separate-tui-execution-modal-state --archive-gate`.

## Future Work

- Reclassify specific background merge failures as recoverable warnings only through a separate proposal with event-ownership and scheduler-safety evidence.
- Expose modal presentation state through remote monitoring only if a future frontend requirement needs it; execution `app_mode` remains unchanged here.
