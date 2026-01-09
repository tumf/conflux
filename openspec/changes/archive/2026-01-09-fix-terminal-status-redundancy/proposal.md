# Proposal: Fix Terminal Status Redundancy in TUI Running Mode

## Change ID
`fix-terminal-status-redundancy`

## Summary
Remove redundant task count from status text for terminal states (completed, archived, error) in TUI running mode. The task count should only be displayed in the separate column, not duplicated in the status text.

## Problem Statement
In running mode, terminal states display task count redundantly:

**Current (redundant):**
```
fix-tui-complete-status     [archived 16/16]  16/16
```

**Expected (concise):**
```
fix-tui-complete-status     [archived]  16/16
```

The status text currently includes the task count (`X/Y`) which is already shown in the separate column. The status text should only show the state name (e.g., `[archived]`, `[completed]`, `[error]`).

## Scope
- `src/tui.rs`: `render_changes_list_running` function (lines 1636-1643)
- Affects only terminal states: `Completed`, `Archived`, `Error`
- Status text changes from `[status X/Y]` to `[status]`
- Separate task count column (`X/Y`) remains unchanged

## Affected Specification
- `openspec/specs/cli/spec.md` - Requirement: Terminal Status Task Count Display

## Approach
Modify the status text format for terminal states to exclude task counts, using the same format as other non-processing states: `format!("[{}]", status.display())`.

## Code Change
```rust
// Before (lines 1636-1643)
QueueStatus::Completed | QueueStatus::Archived | QueueStatus::Error(_) => {
    format!(
        "[{} {}/{}]",
        change.queue_status.display(),
        change.completed_tasks,
        change.total_tasks
    )
}

// After
QueueStatus::Completed | QueueStatus::Archived | QueueStatus::Error(_) => {
    format!("[{}]", change.queue_status.display())
}
```

## Risks
- **Low**: UI-only change with no logic impact
- **Low**: Task count remains visible in the separate column

## Validation
- Visual inspection in TUI running mode
- Verify completed, archived, and error states show `[status]` format
- Verify separate task count column still shows `X/Y`
