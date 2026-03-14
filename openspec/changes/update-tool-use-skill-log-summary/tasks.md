## Implementation Tasks

- [x] Add a dedicated `skill` branch in `src/stream_json_textifier.rs` that includes the requested skill name in the `tool_use` summary (verification: code inspection shows a `skill`-specific formatter using the tool input fields).
- [x] Ensure tool-name matching for `skill` remains case-insensitive so `Skill` and `skill` behave identically (verification: regression test covers mixed-case input).
- [x] Add or update unit tests in `src/stream_json_textifier.rs` for top-level `skill` tool events and assistant message tool blocks (verification: tests assert summaries start with `[tool_use:skill]` or preserve original displayed name while including `name=<skill-name>` metadata).
- [x] Validate the proposal with strict OpenSpec checks (verification: `openspec validate update-tool-use-skill-log-summary --strict --no-interactive`).

## Future Work

- If skill loading later gains aliases, versions, or profile metadata, extend the summary format in a follow-up change.
