# Change: Improve non-Bash tool_use log summaries

## Problem / Context

When `stream_json_textify` is enabled, Conflux converts stream-json tool events into one-line log summaries.
The current implementation in `src/stream_json_textifier.rs` gives detailed summaries for some tools such as `bash`, but many non-Bash tools still produce sparse logs that do not clearly show the target, intent, or safe metadata for the operation.

This is especially noticeable for tools such as `Read`, `Edit`, `TodoWrite`, `Glob`, and `Grep`, where operators need enough context to understand what the agent is doing without exposing large or sensitive payloads.

## Proposed Solution

Extend tool summary extraction so non-Bash tools emit richer, tool-aware summaries while preserving existing safety rules.

- Normalize tool-name matching so tool-specific formatting works regardless of output casing.
- Add tool-aware summary rules for common non-Bash tools, prioritizing actionable fields such as file paths, offsets, limits, patterns, URLs, and todo counts/statuses.
- Preserve redaction for raw edit/write bodies and replace them with safe metadata such as character counts and line counts.
- Keep a bounded generic fallback for unknown tools so logs remain useful even when a tool does not have a dedicated formatter.

## Acceptance Criteria

- With `stream_json_textify=true`, non-Bash `tool_use` events include enough structured detail to identify the operation target and intent without opening the raw JSON.
- Tool-name casing differences do not prevent dedicated summary formatting from applying.
- `write`/`edit` style tools continue to avoid leaking raw body text while still emitting safe metadata.
- Unknown tools still emit a bounded, one-line summary using safe scalar inputs rather than an almost-empty log line.
- Regression tests cover representative non-Bash tools and casing variants.

## Out of Scope

- Changing the provider-emitted stream-json schema.
- Reworking `tool_result` formatting beyond any minimal adjustments needed for consistency.
- Redesigning the TUI log layout or preview rendering.
