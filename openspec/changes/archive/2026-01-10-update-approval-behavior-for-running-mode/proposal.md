# Proposal: Update Approval Behavior Based on Orchestrator State

## Summary

Modify the `@` key behavior in TUI so that approval during orchestrator execution (Running mode) only approves without auto-queuing, while approval during stopped states (Select/Completed mode) continues to auto-queue.

## Motivation

Currently, pressing `@` on an unapproved change in Running mode automatically adds it to the queue:

```
Current behavior (Running mode):
[ ] → @ → [x] (approved + queued)
```

This is problematic because users may want to approve changes for future processing without immediately adding them to the current execution queue. The auto-queue should only happen when the orchestrator is stopped, allowing users to prepare a batch of changes before starting execution.

**Desired behavior:**
- **Running mode** (orchestrator actively processing): `[ ]` → `@` → `[@]` (approve only, no auto-queue)
- **Stopped mode** (Select/Completed): `[ ]` → `@` → `[x]` (approve + auto-queue)

## Proposed Changes

### 1. Modify `@` key behavior in Running mode

Change the state transition in Running mode:

| State | Before | After (`@` pressed) |
|-------|--------|---------------------|
| Running mode | `[ ]` → `[x]` | `[ ]` → `[@]` |
| Select/Completed mode | `[ ]` → `[x]` | `[ ]` → `[x]` (unchanged) |

### 2. Affected transitions by mode

**Running mode (orchestrator processing):**
- `[ ]` (unapproved) → `@` → `[@]` (approved only, NOT queued)
- `[@]` (approved) → `@` → `[ ]` (unapproved)
- `[x]` (queued) → `@` → `[ ]` (unapproved + removed from queue)

**Select/Completed mode (orchestrator stopped):**
- `[ ]` (unapproved) → `@` → `[x]` (approved + auto-queued) - unchanged
- `[@]` (approved) → `@` → `[ ]` (unapproved) - unchanged
- `[x]` (queued) → `@` → `[ ]` (unapproved + removed from queue) - unchanged

## Scope

- **In scope**:
  - Modify `toggle_approval` method in `tui.rs` to differentiate Running vs stopped modes
  - Add new `TuiCommand::ApproveOnly` variant for Running mode
  - Update `handle_tui_command` to process new command

- **Out of scope**:
  - Changes to Select mode behavior
  - Changes to `Space` key behavior
  - CLI run mode behavior

## Success Criteria

1. In Running mode, `@` on `[ ]` results in `[@]` (approved, not queued)
2. In Select/Completed mode, `@` on `[ ]` results in `[x]` (approved + queued) - existing behavior
3. All existing tests pass
4. User can still manually queue approved changes with `Space` in Running mode
