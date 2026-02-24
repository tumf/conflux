# Change: Textify Claude stream-json output inside cflx

## Why

When Claude Code is executed with `--output-format stream-json`, its stdout is newline-delimited JSON events. Today users often pipe that output through an external filter (e.g. `~/bin/cc-stream-filter`) to render a readable text stream.

Embedding this behavior in cflx makes logs readable by default, reduces operational dependencies, and enables consistent, line-oriented logging across CLI/TUI.

## What Changes

- Detect stream-json (NDJSON) output lines from Claude Code during streaming execution.
- Convert supported stream-json event types into user-facing text.
- Normalize the extracted text into line-oriented output:
  - Split multi-line content by newline and emit one log line per line.
  - Buffer partial text chunks until a newline boundary is observed.
- Keep non-stream-json output unchanged.
- Provide a config toggle to disable the built-in conversion for troubleshooting.

## Impact

- Affected specs: `openspec/specs/observability/spec.md`
- Affected code paths:
  - Streaming output readers in `src/ai_command_runner.rs`
  - (Optionally) legacy streaming paths in `src/agent/runner.rs`
- User impact:
  - Removes the need for external stream-json filters in normal operation
  - Improves log readability and stability (one event == one log line)
