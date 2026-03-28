# Change: Fix bulk toggle queue commands in TUI running mode

**Change Type**: implementation

## Problem/Context

In the TUI, pressing `Space` on a single change row while running emits `AddToQueue` or `RemoveFromQueue` commands, which updates reducer state and transitions the row to `queued` or `not queued`.

However, pressing `x` for bulk mark toggle only mutates the local `selected` flag and does not emit queue commands in Running mode. As a result, rows appear marked but do not enter the queue, producing inconsistent behavior between single-row and bulk interactions.

The current canonical spec for bulk execution mark toggle only defines behavior for Select and Stopped modes, while the implementation currently allows bulk toggle targets in Running mode. This proposal aligns the spec with the intended behavior already partially present in code and requires queue command emission for bulk operations during Running mode.

## Proposed Solution

- Update the TUI bulk toggle behavior so that Running mode uses the same queue mutation semantics as single-row `Space` toggles
- Make bulk toggle return the queue commands needed for each affected row in Running mode
- Ensure `x` adds eligible `NotQueued` rows to the dynamic queue and removes eligible `Queued` rows from it
- Preserve existing protections for active rows, resolve-wait/merge-wait rows, and parallel-ineligible changes
- Add regression tests covering both queue addition and queue removal via bulk toggle in Running mode

## Acceptance Criteria

- Pressing `x` in Running mode on eligible unmarked `NotQueued` rows causes those rows to become queued through reducer-visible `AddToQueue` commands
- Pressing `x` in Running mode when all eligible rows are already queued causes those rows to be removed from the queue through reducer-visible `RemoveFromQueue` commands
- Active rows are not converted into stop requests by bulk toggle
- MergeWait and ResolveWait rows continue to affect only execution marks and do not mutate dynamic queue membership
- Existing Select and Stopped mode bulk toggle behavior remains unchanged
- Regression tests cover the Running mode command emission path for bulk toggle

## Out of Scope

- Changing the key binding for bulk toggle
- Altering non-bulk single-row `Space` behavior
- Redesigning dynamic queue scheduling or reducer internals
