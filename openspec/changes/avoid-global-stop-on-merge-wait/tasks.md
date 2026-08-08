## Implementation Tasks

- [ ] Remove blocking warning-popup creation from the change-scoped `ResolveFailed` handler while retaining `merge wait`, elapsed-time accounting, structured `change_id`, diagnostic logging, resolve-slot cleanup, and the existing active-work lifecycle transition. Completion requires the handler to contain no popup or global-stop side effect and the retained log to identify the affected change. (verification: unit - `cargo test --lib tui::state::event_handlers::errors::tests`; verification-id: merge-wait-notification-tests)

- [ ] Add TUI regressions for automatic bounded exhaustion and operator-initiated manual resolve failure proving that change-scoped `ResolveFailed` leaves `warning_popup` absent, preserves `Running` with unrelated active work, permits the existing `Select` transition when none remains, and leaves the affected row retryable as `merge wait`. Completion requires assertions that would fail if either path opened an overlay, entered Error, or converted the row to terminal `error`. (verification: integration - `cargo test --lib tui::state::event_handlers::errors::tests`; verification-id: merge-wait-notification-tests)

- [ ] Preserve severity boundaries for other event classes by extending or retaining regressions that typed `RunFatal` still enters global TUI Error and unrelated warning classes such as `on_merged` hook failure still use their existing popup. Completion requires both fatal and unrelated-popup assertions to pass without deriving severity from diagnostic text. (verification: integration - `cargo test --lib tui::state::event_handlers::errors::tests && cargo test --lib parallel::tests::change_local_merge_error_scope`; verification-id: merge-wait-notification-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate avoid-global-stop-on-merge-wait --archive-gate`.

The implementation verification is `cargo test --lib tui::state::event_handlers::errors::tests && cargo test --lib parallel::tests::change_local_merge_error_scope && cargo fmt --all -- --check && cargo clippy --locked --all-targets --all-features -- -D warnings`.

## Future Work

- Consider a separate notification-severity taxonomy only if additional event classes are observed using modal presentation despite non-blocking scheduler disposition.
