---
change_type: implementation
priority: high
dependencies: []
references:
  - src/stream_json_textifier.rs
  - src/events.rs
  - src/tui/render.rs
  - src/tui/state.rs
  - src/tui/state/log_logic.rs
  - src/tui/key_handlers.rs
  - src/agent/runner.rs
  - openspec/specs/observability/spec.md
  - openspec/specs/tui-architecture/spec.md
verifications:
  - id: tui-log-width-regressions
    requirement: "Tool-event logs preserve useful content through the operator-facing boundary, Logs support within-entry line navigation, and previews remain width-bound single-line output"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust unit and integration tests prove tool_use/tool_result retention, one truthful final safety bound, runner-to-LogEntry propagation, wide and narrow Logs rendering, within-entry display-line navigation, single-line preview truncation, and Unicode safety"
    rerun: "cargo test --lib stream_json_textifier && cargo test --lib agent::runner && cargo test --lib tui::"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Make TUI log display width-aware

**Change Type**: implementation

## Premise / Context

- The TUI Logs panel already permits wrapped continuation lines and the Changes-row log preview is intentionally single-line.
- `stream_json_textifier` currently truncates every `tool_result` summary to 200 characters and selected `tool_use` fields to 60–100 characters before either TUI surface receives them, so a wide terminal cannot reveal additional content and a narrow Logs panel cannot wrap the missing remainder.
- `LogEntry` already applies a UTF-8-safe 8,192-byte operator-facing safety bound with an explicit truncation marker.
- Logs currently wrap by display width, but navigation offsets are entry-based; a single entry taller than the viewport exposes only its trailing lines and its leading wrapped lines cannot be reached.
- The requested distinction is surface-specific: Logs may use multiple navigable lines; previews must never wrap.

## Problem / Context

The current producer-side 60–200-character limits make both TUI surfaces behave as if they had a fixed display width. Increasing the terminal width does not reveal more of a tool event because content has already been replaced with `...`. The Logs panel wraps retained text, but entry-based navigation cannot reach the leading lines of one wrapped entry taller than the viewport. The Changes-row preview has the same premature data loss, although its final presentation must remain one line.

## Proposed Solution

Remove fixed display-length cutoffs from displayable `tool_result` content and permitted `tool_use` scalar fields. Build the complete semantic summary first, including its `[tool_result:<tool_use_id>]` or tool-use prefix, then apply the existing operator-facing sanitization and 8,192-byte safety bound exactly once to that complete summary before it reaches CLI/TUI consumers. `LogEntry` construction remains idempotent for an already-sanitized bounded summary: it must not replace a truthful truncation marker with a second marker that reports only prefix-overflow bytes. Preserve privacy behavior: raw stream JSON remains hidden, write/edit bodies remain omitted, and unsafe/control content remains sanitized. This intentionally makes the same retained sanitized summary visible in non-TUI `cflx run` output.

Keep surface width ownership in the TUI renderer and make wrapped content reachable:

- The Logs panel renders the retained message through its display-width-aware wrapper. The first line uses the space after timestamp and contextual header; continuation lines start at column zero and use the full inner panel width.
- Logs navigation uses a process-local anchor over the current filtered, wrapped display-line sequence rather than only a count of skipped entries. Existing `PgUp`, `PgDn`, `Home`, and `End` keys remain unchanged, but can move within a single entry taller than the viewport. Width/filter/log-buffer changes clamp or reset the anchor deterministically; auto-scroll still follows the newest line.
- The Changes-row preview remains exactly one display line. It consumes all remaining row width and adds an ellipsis only when that actual width cannot contain the retained preview.
- Both paths use Unicode display width and preserve valid UTF-8 for CJK and emoji.

Producer retention, display-line navigation, and preview rendering must ship together. Splitting them would leave either richer messages without reachable presentation or width-aware surfaces with content already lost.

## Acceptance Criteria

- A permitted `tool_use` scalar or `tool_result` content longer than its former 60–200-character cutoff but below the shared operator-facing safety bound reaches the operator-facing message without a producer-added fixed-position `...`.
- A very large complete tool-event summary is sanitized and bounded once by the shared 8,192-byte policy; its final marker reports the actual bytes omitted from that complete prefixed summary.
- The same sanitized bounded summary reaches non-TUI CLI output and TUI `LogEntry` without intermediate re-truncation.
- A wider Logs panel displays more retained content per line, up to its inner width.
- A narrow Logs panel wraps retained content across multiple lines, and `PgUp`/`PgDn`/`Home`/`End` can reach every wrapped line—including at least the first 200 source characters when present—even when one entry exceeds viewport height.
- Logs-panel continuation lines use the full inner width without timestamp/header indentation; filtering, resize, buffer trimming, manual navigation, and auto-scroll maintain a valid deterministic line anchor and keep the newest line visible when auto-scroll is enabled.
- A wider Changes-row preview reveals more retained content than a narrower preview.
- The Changes-row preview always occupies one line and truncates only at its actual remaining width.
- CJK and emoji content is neither split at invalid UTF-8 boundaries nor measured as byte width in either surface.

## Explicit Completion Conditions

- `src/stream_json_textifier.rs` no longer applies 60–200-character display cutoffs to permitted tool-event fields; one shared helper sanitizes and bounds the fully prefixed summary exactly once and preserves truthful omitted-byte accounting through final `LogEntry` construction.
- The runner/textifier integration has a regression test proving a >200-character summary reaches the final operator-facing event without intermediate fixed-length truncation.
- `src/tui/state.rs`, `src/tui/state/log_logic.rs`, `src/tui/key_handlers.rs`, and `src/tui/render.rs` share one process-local display-line navigation contract that can address lines within an oversized entry without changing key assignments or durable workflow state.
- `src/tui/render.rs` retains separate multi-line Logs-panel and single-line preview policies; no shared helper can accidentally make previews wrap.
- Focused tests fail if producer truncation returns at a former cutoff, omitted-byte accounting is false, the first 200 characters of an oversized wrapped entry cannot be reached by navigation, resize/filter/trimming leaves an invalid anchor, a preview spans multiple rows, or Unicode display-width handling regresses.

## Out of Scope

- Increasing or removing the 8,192-byte operator-facing safety bound.
- Displaying raw stream-json events.
- Exposing write/edit body content or weakening tool-use privacy redaction.
- Allowing Changes-row previews to wrap or changing change-row height.
- Changing WebUI log presentation.
- Changing Logs-panel vertical allocation, key assignments, filtering semantics, or retention count.
- Persisting the display-line anchor or using Logs presentation state as workflow control.
- Adding a user-configurable display-length setting.

## Verification Ownership

The tracked Rust pre-commit hooks are path-selected and therefore do not run for this proposal-only commit. During implementation, the behavior-specific tests declared by `tui-log-width-regressions` are change-blocking; repository-wide formatting and clippy remain owned by the normal Rust finalization hook when Rust files are staged.
