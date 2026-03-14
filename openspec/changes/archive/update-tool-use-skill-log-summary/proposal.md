# Change: Add tool_use summaries for Skill tool

## Problem / Context

Conflux already summarizes many `tool_use` events into one-line logs when `stream_json_textify` is enabled.
Recent work expanded coverage for several non-Bash tools, but the `skill` tool is still a notable gap for operator visibility.

When an agent loads a skill, the current logs may not clearly communicate which skill was invoked, making proposal-oriented and workflow-oriented runs harder to follow from the TUI or streamed logs.

## Proposed Solution

Add a dedicated `tool_use` summary rule for the `skill` tool.

- Surface the requested skill name in the one-line summary.
- Keep the output bounded and consistent with existing tool summary formatting.
- Preserve the generic fallback for unknown tools, but ensure `skill` has a dedicated formatter because its primary field is semantically important.

## Acceptance Criteria

- With `stream_json_textify=true`, a `tool_use` event with `name=skill` renders as a one-line summary that includes the requested skill name.
- The same dedicated formatting applies when the tool name casing differs, such as `Skill`.
- Assistant message tool blocks containing `skill` also render with the dedicated summary rather than raw JSON.
- Regression tests cover top-level and mixed-case `skill` tool events.

## Out of Scope

- Changing how skill content itself is rendered after the tool result arrives.
- Reworking summaries for unrelated tools.
