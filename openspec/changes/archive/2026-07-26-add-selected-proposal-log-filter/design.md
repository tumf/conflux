# Design: Selected Proposal Log Filter

## Decision

Keep filtering inside the TUI presentation layer. Add one boolean to `AppState`; derive the target from the existing Changes-view cursor instead of storing a second proposal ID.

This avoids state drift: cursor movement automatically changes the target, and no new workflow command or scheduler state is needed.

## Display Flow

1. The TUI buffers all `LogEntry` values through the existing `AppState::add_log` path.
2. `f` toggles selected-proposal filtering while the Changes view is active.
3. `render_logs` reads the current cursor proposal ID.
4. When filtering is off, all entries enter the existing render pipeline.
5. When filtering is on, only entries with `entry.change_id == cursor_change_id` enter the pipeline.
6. Wrapping, total display lines, visible range, and scrolling use that resulting set.

The filter never mutates `AppState::logs`. Disabling it therefore restores every entry still retained by the existing bounded buffer.

## Identity Contract

Structured `LogEntry::change_id` is the only proposal association. Message parsing is excluded because wording is not a stable identity contract and can match unrelated text.

Proposal-specific event handlers must set the field whenever their source event provides an ID. Logs that describe orchestration as a whole remain unscoped. Remote logs with only a project ID are not promoted to a proposal identity for filtering.

## Scrolling

The existing offset is log-entry based while rendering expands entries into wrapped lines. Filter changes can invalidate an old offset, so toggling the filter and moving the cursor under an active filter reset `log_scroll_offset` to zero and enable auto-scroll. New matching entries then follow the existing newest-output behavior.

Nonmatching entries may still be buffered while the filter is active. They must not alter the visible filtered range or remove matching history, and rendering must clamp safely if the target has no entries.

## Key and Hint

Use lowercase `f`, currently unassigned in the Changes view. The Logs panel title reports both the operation and state. The full form names the selected proposal; narrow layouts may shorten the wording while retaining the key and on/off meaning.

## State Boundary

The flag is ephemeral TUI state. It is not serialized, sent through `TuiCommand`, exposed to the scheduler, or used to select work. This preserves the constitutional rule that UI and observability state cannot control workflow decisions.

## Testing

Use focused module tests without launching an interactive terminal:

- state tests for default, toggling, cursor following, and buffer preservation;
- key-handler tests for Changes-view scope and selection independence;
- render buffer/title tests for exact matching, global exclusion, zero matches, ranges, and hints;
- event-handler tests for structured proposal identity and intentionally unscoped global entries.

No new dependency or external service is required.
