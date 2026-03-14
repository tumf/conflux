## Implementation Tasks

- [x] Update `src/stream_json_textifier.rs` to normalize `tool_use` name matching and apply dedicated summary rules for common non-Bash tools such as `read`, `write`, `edit`, `todowrite`, `grep`, `glob`, and `webfetch` (verification: code inspection shows case-insensitive matching and per-tool extraction branches).
- [x] Preserve log-safety rules for body-bearing tools by continuing to suppress raw payload fields and emitting safe metadata instead (verification: unit tests assert summaries omit raw `text`/`content`/`old_string`/`new_string` values while including counts).
- [x] Add a bounded generic fallback for unknown tools that extracts a small set of safe scalar inputs when no dedicated formatter exists (verification: unit tests assert unknown tools still produce informative one-line summaries).
- [x] Add or update regression tests in `src/stream_json_textifier.rs` for representative non-Bash tools and casing variants (verification: tests cover at least one file tool, one search tool, one todo tool, and a mixed-case tool name).
- [x] Validate the proposal with strict OpenSpec checks (verification: `openspec validate update-tool-use-nonbash-log-summaries --strict --no-interactive`).

## Future Work

- Consider standardizing summary field ordering and field naming across all tools if operators want machine-parsable log summaries later.
