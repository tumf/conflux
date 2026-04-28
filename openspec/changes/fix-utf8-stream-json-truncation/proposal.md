---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/stream_json_textifier.rs
  - openspec/specs/cflx-proposal-validation/spec.md
---

# Proposal: Fix UTF-8-unsafe truncation in stream-json text summaries

**Change Type**: implementation

## Summary

Prevent stream-json text summary rendering from panicking when tool arguments or tool results contain multi-byte UTF-8 characters and must be truncated for display.

## Problem

`src/stream_json_textifier.rs` currently truncates summary strings by byte length using `&s[..max_len]`. When a summary contains multi-byte UTF-8 characters and the truncation limit lands inside a code point, the process can panic while rendering assistant tool summaries or tool-result summaries.

## Solution

Replace the byte-based `truncate_string()` implementation with a UTF-8-safe truncation path that preserves character boundaries while keeping the existing bounded-summary behavior. Add regression coverage for tool summary and tool-result summary paths that currently depend on `truncate_string()`.

## Acceptance Criteria

- Stream-json summary rendering does not panic when truncated strings contain multi-byte UTF-8 characters.
- Assistant tool summaries still truncate long `filePath`, `pattern`, `url`, `prompt`, `args`, and generic scalar values after the fix.
- Tool-result summaries still truncate long content after the fix.
- Regression tests cover at least one assistant tool summary case and one tool-result case where the previous byte cutoff would have split a multi-byte code point.

## Explicit Completion Conditions

- `src/stream_json_textifier.rs` no longer slices `&str` by raw byte offset inside `truncate_string()`.
- Automated tests exercise UTF-8 truncation through real summary helpers instead of only unit-testing an isolated helper.
- `cflx openspec validate fix-utf8-stream-json-truncation --strict` passes for this proposal.

## Out of Scope

- Changing which fields are included in tool summaries.
- Redesigning stream-json line buffering or event parsing.
- Changing truncation lengths beyond what is needed to make truncation UTF-8-safe.
