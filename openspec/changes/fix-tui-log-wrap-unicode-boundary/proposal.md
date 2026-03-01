# Change: Fix TUI log wrapping for Unicode boundaries

## Problem / Context

The TUI Logs view can panic when wrapping log lines that contain multi-byte UTF-8 characters.
One observed panic is:

```
thread 'main' panicked at src/tui/render.rs:952:25:
byte index 1 is not a char boundary; it is inside '\u{2192}' (bytes 0..3) of `\u{2192} Skill "cflx-workflow"`
```

This is caused by log wrapping logic that computes split points using byte lengths and then slices `&str` at non-character boundaries.

## Proposed Solution

- Update the log wrapping logic used by the Logs view to split lines by terminal display width without ever slicing inside a UTF-8 codepoint.
- Use existing Unicode width utilities (`unicode-width`) for display-width calculations.
- Add regression tests that cover narrow widths and strings that begin with multi-byte characters.

## Acceptance Criteria

- The TUI MUST NOT panic when rendering log messages containing multi-byte UTF-8 characters, including when the available wrap width is 1.
- Log wrapping MUST preserve message contents (no dropped or corrupted characters).
- `cargo test` passes.

## Out of Scope

- Rewriting the entire Logs panel layout system.
- Changing log content generation.

## Impact

- Affected specs: `tui-error-handling`
- Affected code: `src/tui/render.rs` (log wrapping)
