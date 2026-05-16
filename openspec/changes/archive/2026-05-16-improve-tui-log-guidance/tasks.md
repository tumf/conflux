## Implementation Tasks

- [x] Add compact log navigation guidance to the visible Logs panel when `app.logs_panel_enabled` is true, preserving existing range/offset/auto-scroll information in `src/tui/render.rs` (verification: unit - render-buffer tests in `src/tui/render.rs` assert the visible Logs panel includes `PageUp`/`PageDown` or compact equivalents, `Home`/`End`, and `l` when logs are shown).

- [x] Preserve existing log navigation behavior for `PageUp`, `PageDown`, `Home`, `End`, and `l` without changing scheduler/workflow state (verification: unit - focused key/state tests cover `src/tui/key_handlers.rs` dispatch to `AppState::scroll_logs_up`, `scroll_logs_down`, `scroll_logs_to_top`, `scroll_logs_to_bottom`, and `toggle_logs_panel`, or existing tests are extended to assert equivalent state changes).

- [x] Keep the existing `l: logs` Changes panel hint visible in select and running modes while adding Logs-panel-specific scroll guidance (verification: unit - existing/updated `src/tui/render.rs` tests assert `l: logs` remains visible in select and running mode buffers).

- [x] Run formatting and focused quality gates after implementation (verification: integration - `cargo fmt --check` and focused TUI render/key tests such as `cargo test log_panel --lib` or the final equivalent command pass).

## Future Work

- Add optional mouse wheel support for Logs panel scrolling if terminal compatibility demand emerges.
- Add user-configurable TUI keybindings for log controls under the user-level TUI config surface if a later change expands TUI keybinding customization.
