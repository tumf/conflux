## Implementation Tasks

- [x] Update `wrap_log_message` to wrap by display width and never slice at non-char boundaries (verification: add a unit test that reproduces the prior panic and confirm it no longer panics).
- [x] Update any width computations used by log wrapping/rendering to be consistent (bytes vs. display width) (verification: add/adjust a test that uses a multi-byte prefix and a narrow width).
- [x] Add regression tests for messages starting with `\u{2192}` and for `available_width=1` (verification: `cargo test` passes and the test asserts no panic).
- [x] Run formatting and lint checks (verification: `cargo fmt --check` and `cargo clippy -- -D warnings`).

## Future Work

- Consider adding a small shared helper (e.g., `src/tui/utils.rs`) for "take by display width" so other render paths can reuse it.
