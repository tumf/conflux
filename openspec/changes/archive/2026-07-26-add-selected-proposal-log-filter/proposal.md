---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/observability/spec.md
  - src/events.rs
  - src/tui/state.rs
  - src/tui/state/log_logic.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - src/tui/state/event_handlers/
verifications:
  - id: selected-proposal-log-filter-local
    requirement: The TUI defaults to all logs and can show only logs structurally associated with the cursor proposal without mutating stored logs or workflow state
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: focused TUI unit test output and repository quality-gate results recorded in the acceptance review
    rerun: make test
    prerequisites: []
---

# Add Selected Proposal Log Filter

**Change Type**: implementation

## Problem / Context

The TUI Logs panel renders every buffered log entry. During parallel execution, output from multiple proposals is interleaved, so an operator following the proposal under the cursor must visually search through unrelated output.

`LogEntry` already has an optional `change_id`, and change rows already use it for per-change log previews, but the Logs panel has no proposal filter. Some proposal lifecycle, completion, and error handlers also place the proposal ID only in message text instead of the structured field, which would make a strict filter incomplete.

The filter must remain non-authoritative presentation state. It must not change buffered logs, queued proposals, scheduler behavior, resume routing, acceptance, archive, or other workflow decisions, in accordance with `openspec/CONSTITUTION.md`.

## Proposed Solution

Add one TUI-local boolean filter, defaulting to off. Bind `f` in the Changes view to toggle whether the Logs panel renders all buffered entries or only entries whose structured `change_id` matches the proposal at `cursor_index`.

When the filter is on:

- moving the cursor immediately changes the filter target;
- entries without a matching `change_id`, including global and project-only logs, are hidden;
- the visible log set is selected before wrapping, counts, and scroll bounds are calculated;
- toggling the filter or changing its cursor target returns the panel to the newest visible position with auto-scroll enabled;
- the Logs panel title exposes the key and current off/on target state;
- an empty matching set renders safely without altering `AppState::logs`.

Attach structured `change_id` metadata to proposal-specific TUI lifecycle, completion, skip, stop, and error logs that currently encode the ID only in message text. Do not infer proposal identity by parsing messages.

Keep the feature as one proposal because filter correctness depends on both the UI state/render path and complete structured metadata for proposal-specific TUI logs; shipping either part independently would expose an incomplete filter.

## Acceptance Criteria

- A newly initialized TUI has the proposal log filter off and renders all buffered Logs-panel entries as before.
- Pressing `f` in the Changes view toggles a filter for the proposal currently under the cursor without changing its execution mark.
- While enabled, the Logs panel shows only entries whose structured `change_id` matches the cursor proposal and hides global, project-only, and other-proposal entries.
- Moving the cursor while the filter is enabled immediately follows the new proposal and resets the visible log position to its newest matching output.
- Disabling the filter restores all still-buffered entries; toggling never deletes or rewrites `AppState::logs`.
- Filtering occurs before wrapping, visible-line counts, title ranges, and scroll bounds are calculated, including the zero-match case.
- The Logs panel visibly identifies `f` as the filter key and reports either off or the selected proposal as the active target, with a compact equivalent allowed for narrow terminals.
- Proposal-specific lifecycle, completion, skip, stop, and error entries emitted by TUI event handlers carry structured `change_id` metadata.
- Remote entries that identify only a project and cannot identify a proposal are excluded while the proposal filter is enabled.
- The filter remains presentation-only and cannot affect scheduler or workflow-control behavior.

## Explicit Completion Conditions

- `AppState` contains a non-persistent filter flag initialized to `false`, a toggle operation, and a safe way to resolve the cursor proposal ID.
- The Changes-view key handler processes `f`; unrelated views and existing keys retain their current behavior.
- `render_logs` derives one filtered iterator or collection before all wrapping and scrolling calculations and does not mutate the log buffer.
- Proposal-specific handlers under `src/tui/state/event_handlers/` consistently set `LogEntry::change_id` where the event carries a proposal ID.
- Focused unit/render/key-handler tests prove default-off behavior, toggling, cursor following, strict matching, global-log exclusion, zero matches, buffer preservation, filtered scroll bounds, metadata attachment, and visible hints.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cflx openspec validate add-selected-proposal-log-filter --strict --evidence warn` pass.

## Out of Scope

- Filtering by checked/queued proposal marks or by multiple proposals.
- Parsing proposal IDs from unstructured log messages.
- Persisting the filter in `.cflx.jsonc` or outside the process.
- Adding a generic log query language, level filter, operation filter, or search UI.
- Changing the WebUI or persistent log files.
- Redesigning remote server stdout/stderr to add proposal identity where only project identity is currently available.
- Displaying the Logs panel in TUI modes or layouts where it is not currently rendered.
