---
change_type: implementation
priority: high
dependencies: []
references:
  - src/stream_json_textifier.rs
  - src/events.rs
  - src/tui/render.rs
  - openspec/specs/observability/spec.md
  - openspec/specs/tui-architecture/spec.md
verifications:
  - id: tui-log-width-regressions
    requirement: Tool-result logs preserve useful content through the TUI boundary and each TUI surface applies its own width policy
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Focused Rust unit tests in src/stream_json_textifier.rs and src/tui/render.rs prove producer retention, wide and narrow Logs-panel rendering, single-line preview truncation, Unicode safety, and wrapped-line scroll accounting
    rerun: cargo test --lib stream_json_textifier && cargo test --lib tui::render::tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Make TUI log display width-aware

**Change Type**: implementation

## Premise / Context

- The TUI Logs panel already permits wrapped continuation lines and the Changes-row log preview is intentionally single-line.
- `stream_json_textifier` currently truncates every `tool_result` summary to 200 characters before either TUI surface receives it, so a wide terminal cannot reveal additional content and a narrow Logs panel cannot wrap the missing remainder.
- `LogEntry` already applies a separate UTF-8-safe 8,192-byte operator-facing safety bound with an explicit truncation marker.
- The requested distinction is surface-specific: Logs may use multiple lines; previews must never wrap.

## Problem / Context

The current producer-side 200-character limit makes both TUI surfaces behave as if they had a fixed display width. Increasing the terminal width does not reveal more of a tool result because the content has already been replaced with `...`. The Logs panel also cannot use additional wrapped lines to expose useful content on a narrow terminal. The Changes-row preview has the same premature data loss, although its final presentation must remain one line.

## Proposed Solution

Remove the 200-character `tool_result` display cutoff and pass the result summary through the existing shared operator-facing safety boundary instead. Preserve the current semantic summary and privacy behavior: raw stream JSON remains hidden, write/edit bodies remain omitted from tool-use summaries, and arbitrarily large messages remain bounded with an explicit marker.

Keep width ownership in the TUI renderer:

- The Logs panel renders the retained message through its existing display-width-aware wrapper. The first line uses the space after timestamp and contextual header; continuation lines start at column zero and use the full inner panel width. Content beyond the visible height remains reachable through existing log scrolling.
- The Changes-row preview remains exactly one display line. It consumes all remaining row width and adds an ellipsis only when that actual width cannot contain the retained preview.
- Both paths use Unicode display width and preserve valid UTF-8 for CJK and emoji.

Producer retention and TUI rendering must ship together. Splitting them would leave either richer messages without correct presentation coverage or width-aware rendering with content already lost.

## Acceptance Criteria

- A `tool_result` summary longer than 200 characters but below the shared operator-facing safety bound reaches `LogEntry` without a producer-added `...` at character 200.
- A very large `tool_result` remains bounded by the shared 8,192-byte safety policy and explicitly reports truncation.
- A wider Logs panel displays more retained content per line, up to its inner width.
- A narrow Logs panel wraps retained content across multiple lines rather than replacing it at a fixed character count; at least the first 200 characters remain represented in the wrapped display/scrollable line set when the source contains that much content.
- Logs-panel continuation lines use the full inner width without timestamp/header indentation, and wrapped-line range/scroll accounting keeps the newest log visible.
- A wider Changes-row preview reveals more retained content than a narrower preview.
- The Changes-row preview always occupies one line and truncates only at its actual remaining width.
- CJK and emoji content is neither split at invalid UTF-8 boundaries nor measured as byte width in either surface.

## Explicit Completion Conditions

- `src/stream_json_textifier.rs` no longer applies the 200-character cutoff to `tool_result` content and uses the shared bounded operator-facing representation without duplicating a conflicting limit.
- `src/tui/render.rs` retains separate multi-line Logs-panel and single-line preview policies; no shared helper can accidentally make previews wrap.
- Focused tests fail if producer truncation returns at 200 characters, if a wider terminal does not reveal more text, if the Logs panel stops wrapping, if a preview spans multiple rows, or if Unicode display-width handling regresses.
- Existing log filtering, auto-scroll/manual-scroll behavior, contextual headers, and the global bounded log buffer remain green under the focused TUI tests.

## Out of Scope

- Increasing or removing the 8,192-byte operator-facing safety bound.
- Displaying raw stream-json events.
- Exposing write/edit body content or changing tool-use privacy redaction.
- Allowing Changes-row previews to wrap or changing change-row height.
- Changing WebUI log presentation.
- Changing Logs-panel vertical allocation, key bindings, filtering, or retention count.
- Adding a user-configurable display-length setting.

## Verification Ownership

The tracked Rust pre-commit hooks are path-selected and therefore do not run for this proposal-only commit. During implementation, the behavior-specific tests declared by `tui-log-width-regressions` are change-blocking; repository-wide formatting and clippy remain owned by the normal Rust finalization hook when Rust files are staged.
