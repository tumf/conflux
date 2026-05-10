# Design: streaming log mining helper

## Context

The helper is a repository-bundled diagnostic script. It must be safe to run over large local Conflux logs that may contain confidential content. The script output is an operator-facing observability report, not workflow-control input.

## Approach

Use a streaming scanner for each selected log file:

- Iterate files selected by marker mtime as today.
- Iterate each file line-by-line.
- Maintain a bounded rolling buffer for previous context lines.
- For each hit, capture the current line plus bounded nearby context without retaining the full file.
- If following context is required, keep a small pending-hit queue until the configured context radius has elapsed.
- Keep report-level example counts bounded by `--max-examples`, `--top`, and the existing marker caps.

## Compatibility

The report shape should remain stable:

- Text output keeps the same section names.
- JSON output keeps the same top-level keys and hit dictionaries.
- `--change-id` continues to filter examples/timeline hits by text plus captured context.

## Privacy and safety

The helper must not write mined log content into repository-tracked files. Tests should generate synthetic logs with non-sensitive fixture content. Redaction behavior for grouped keys must remain in place and may be expanded, but should not remove actionable diagnostics from examples.

## Trade-offs

Streaming makes exact symmetric context harder than full-file slicing. A small pending-hit queue preserves the current context-radius semantics while keeping memory bounded by hit count and context radius instead of file size.
