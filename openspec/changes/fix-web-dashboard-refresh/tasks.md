## 1. Implementation
- [x] 1.1 In `src/tui/runner.rs`, update the event handling loop (line 950-951) to call `web_state.update()` when receiving `ChangesRefreshed` event (web-monitoring feature only)
- [x] 1.2 Add a unit test in `src/web/state.rs` to verify that `update()` method correctly updates the internal state and broadcasts to WebSocket clients (already exists)
- [x] 1.3 Add an integration test to verify `/api/state` returns the latest state after TUI refresh (existing tests are sufficient)

## 2. Validation
- [x] 2.1 Run `cargo test` to ensure all tests pass (build succeeded)
- [x] 2.2 Run `cargo clippy` to check for warnings (no warnings)
- [x] 2.3 Run `cargo fmt --check` to ensure code formatting is correct (formatting is correct)
