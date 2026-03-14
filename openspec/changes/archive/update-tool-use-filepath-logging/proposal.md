# Change: Improve tool_use summaries for file tools

## Problem / Context

When `stream_json_textify` is enabled, tool-related events are summarized into one-line log entries.
For file-oriented tools (`read`, `write`, `edit`), the current summaries are not consistently actionable:

- The target file path is not always visible.
- For `write`/`edit`, the input body may be large and/or sensitive, creating noisy logs and increasing disclosure risk.

## Proposed Solution

Refine the stream-json `tool_use` summarization rules:

- For `read`/`write`/`edit` tool_use events, include the file path in the one-line summary (prefer `input.filePath`; support common aliases when present).
- For `write`/`edit`, do not include raw input body text (e.g., `input.text`) in the summary.
- For `write`/`edit`, the summary may include safe metadata (e.g., character count / line count) instead of the body content.
- Existing truncation behavior for long values remains in effect.

## Acceptance Criteria

- With `stream_json_textify=true`, `tool_use` events for `read`/`write`/`edit` display a one-line summary that includes the file path.
- With `stream_json_textify=true`, `tool_use` events for `write`/`edit` do not expose the input body content in logs.
- Behavior for other tools is unchanged.
- Unit tests cover read/write/edit cases and prevent regressions.

## Out of Scope

- Changing the schema emitted by the underlying agent/provider.
- Reformatting unrelated log output.
- Path normalization (e.g., repo-relative rendering) unless separately proposed.
