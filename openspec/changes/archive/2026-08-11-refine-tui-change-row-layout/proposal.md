---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/tui-state/spec.md
  - src/orchestration/operator_command.rs
  - src/orchestration/state.rs
  - src/tui/render.rs
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/tui/utils.rs
verifications:
  - id: tui-change-row-layout-tests
    requirement: "Changes rows omit the cursor glyph, retain a blank checkbox after archive completion, render change IDs in a fixed 36-column field, and keep post-archive mark behavior consistent with the hidden checkbox"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering reducer-to-TUI archive-fact synchronization, refresh preservation, post-archive mark refusal, Select and Running buffer layout, ASCII truncation, and Unicode display width"
    rerun: "cargo test --lib tui_change_row_layout"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Refine the TUI Changes row layout

**Change Type**: implementation

## Premise / Context

- The TUI builds `Changes` rows independently in the Select and Running render paths in `src/tui/render.rs`.
- Each row currently reserves a cursor-glyph column and renders `►` on the focused row even though Ratatui already highlights the focused row background.
- Change IDs are padded to 25 characters and are not truncated, so long IDs shift worktree, activity, status, and progress fields.
- Checkbox suppression currently keys only on the display strings `archived`, `merged`, and `pushed`.
- The reducer records archive completion separately in `archived_changes`, then truthfully advances the display to `resolving` or `resolve pending`; a post-archive row can therefore regain an empty `[ ]` checkbox.
- Existing canonical CLI requirements already require a blank post-archive checkbox placeholder, but retain stale scenarios that still describe an archived `[x]`.

## Problem / Context

The `Changes` list uses redundant focus chrome, unstable change-ID width, and an incomplete post-archive predicate. The resulting rows do not align, long IDs overwrite the intended layout, and an archive-complete change can display `[ ]` again while post-archive resolve or publication work is active.

Treating `resolving` as inherently post-archive would replace one string-based inference with another. The TUI instead needs the reducer-owned archive milestone as presentation input, while preserving the Constitution's rule that workflow decisions remain reducer-owned and workspace-derived.

## Proposed Solution

Revise the Changes-row contract as one atomic layout change:

1. Remove the `►` glyph and its dedicated column from both Select and Running row builders. Keep the existing row highlight as the only focus indicator.
2. Define the change-ID field as 36 terminal columns from ID start to the next field start: at most 35 display columns of ID content, followed by one separator column supplied by the next field's leading space.
3. Hard-truncate overlong IDs at 35 terminal display columns without an ellipsis and pad shorter IDs to exactly 35 display columns. Use Unicode display width so CJK and other wide characters do not shift following fields.
4. Synchronize the reducer's archived-ID snapshot through the existing reducer-to-TUI cache boundary. Preserve that process-local presentation fact when refresh rebuilds rows.
5. Suppress checkbox text when either the reducer has recorded archive completion or the display status is a final post-archive status. Continue rendering the existing three-column blank placeholder.
6. Use the same archive-complete fact for row mark hints and mark admission so a hidden checkbox cannot still be toggled through Space or bulk mark commands.
7. Replace duplicated byte-length preview calculations with the fixed display-column constants used by both row layouts.
8. Remove stale canonical scenarios that still require an archived `[x]`.

No new dependency, durable state, configuration option, workflow status, Web/API field, or execution transition is introduced.

## Acceptance Criteria

1. No Changes row displays `►` in Select, Running, Stopping, Stopped, or Error presentation; the focused row remains identifiable through the existing highlight style.
2. Every row starts its change ID immediately after the three-column checkbox area and one separating space.
3. The change-ID field occupies exactly 36 terminal columns from ID start to the next field start: 35 columns of truncated/padded ID content and one separator column.
4. `preserve-archiving-during-tui-refresh` renders as `preserve-archiving-during-tui-refre` with no ellipsis, while shorter IDs are padded so the next badge starts in the same column.
5. Unicode and CJK IDs are truncated and padded by terminal display width rather than UTF-8 byte length or scalar count; the next field starts in the same column as for ASCII IDs.
6. A reducer-recorded archive-complete row renders neither `[x]` nor `[ ]` while its display status is `resolving`, `resolve pending`, `merge wait`, `merged`, or `pushed`.
7. The post-archive checkbox placeholder remains exactly three columns, preserving the ID and following field positions.
8. Refresh reconstruction does not briefly restore a checkbox or lose archive-complete presentation state.
9. Space and bulk mark operations treat a reducer-recorded archive-complete row as non-markable, omit mark hints, and do not create invisible execution intent.
10. An active row without reducer-recorded archive completion, such as `applying` or a fresh-process `resolving` resolve retry, remains markable and still displays its actual `[x]` or `[ ]` state.
11. WebUI/API state and display payload contracts and reducer lifecycle transitions remain unchanged; mark-command unchanged outcomes reflect reducer-recorded archive completion without adding a payload field.
12. Preview text remains safely omitted when the widened fixed row fields leave less than the existing minimum preview width.

## Explicit Completion Conditions

- `ChangeState` and `AppState` carry a process-local reducer-derived archive-complete presentation cache, synchronized in the same snapshot read as display status and reapplied after row refresh.
- Rendering, key hints, local optimistic selection, and shared mark admission use one archive-complete predicate rather than inferring completion from `resolving`.
- The shared mark classifier accepts caller-supplied archive-complete evidence: TUI callers use the synchronized presentation cache, while operator-command and API callers use `OrchestratorState::archived_changes()` from the same reducer read as display status. Orchestration code does not depend on TUI state.
- Both Changes render paths use shared constants/helpers for checkbox width, the one-column checkbox-to-ID separator, 35-column ID content, and preview base width.
- The rendered ID helper performs Unicode display-width hard truncation and right padding without adding a suffix.
- Buffer tests assert the user's two representative rows, absence of `►`, retained highlight, stable following-field columns, and safe narrow-terminal degradation.
- State/runner tests drive a real `ChangeArchived` reducer event through TUI cache synchronization and refresh reconstruction.
- Command tests prove archive-complete rows reject single and bulk mark mutation while pre-archive active rows retain current mark behavior.
- The declared `tui-change-row-layout-tests` verification passes.

## Scope Rationale

Cursor removal, fixed ID width, checkbox placeholder behavior, and preview width all change the same row prefix and its column accounting. Splitting them would create intermediate layouts with contradictory alignment rules. Archive-fact synchronization and mark admission also ship together because hiding a checkbox while leaving it mutable would create invisible operator intent.

## Fable Review Disposition

The initial Fable review was adopted for the reducer-owned archive snapshot, refresh preservation, 35-plus-1 column definition, Unicode display-width handling, hard truncation without ellipsis, stale-spec cleanup, and single-proposal scope. Its initial suggestion to split post-archive mark admission was not adopted because an invisible mutable mark would violate truthful presentation. Fable's artifact re-review conditionally supported that integration and required the accompanying `remote-control-api`, `operator-command-execution`, `tui-state`, and `tui-key-hints` deltas; those cross-capability contracts, the fresh-process `resolving` control, status-only fallback coverage, and caller-supplied archive evidence flow are included here.

## Out of Scope

- Changing reducer lifecycle transitions or display-status vocabulary.
- Changing WebUI/API state or display payload schemas, or exposing the archive-complete cache as a new field. Existing mark-command outcomes may report a stable archive-complete no-op reason.
- Adding ellipses, horizontal scrolling, configurable column widths, or responsive column priorities.
- Aligning status fields across rows that do not carry the same optional badges.
- Redesigning colors, spinner glyphs, elapsed-time formatting, task progress, or preview contents.

The tracked Rust hooks in `.pre-commit-config.yaml` are path-scoped and do not run for proposal-only commits. Requirement-specific focused tests remain explicit implementation evidence rather than being delegated to this proposal commit's hooks.
