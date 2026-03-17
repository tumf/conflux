# Change Proposal: fix-parallel-f5-queue-stall

## Problem / Context

- In TUI parallel mode, a change can appear eligible when the user presses `F5`, but get filtered out moments later by the parallel executor's fresh Git-based eligibility check.
- The current flow marks selected or execution-marked changes as `Queued` before that fresh eligibility check completes.
- If the executor drops every selected change as uncommitted, the run returns early without emitting a state-reconciliation event, leaving the TUI row visually stuck in `Queued` even though nothing is actually scheduled.
- This violates the existing stopped-mode and parallel-mode expectations that `F5` resumes real processing and that uncommitted changes are excluded predictably.

## Proposed Solution

- Reconcile TUI queue state with the authoritative parallel-start eligibility check so `F5` cannot leave rows stranded in `Queued` after backend-side filtering.
- Ensure parallel start and stopped-mode resume both revalidate the latest committed/uncommitted status before the TUI treats a change as actively queued.
- Emit explicit UI-visible state transitions or warnings when a change is rejected during parallel start, so the row returns to a non-queued state and the user sees why execution did not begin.
- Add regression coverage for the stale-eligibility race between TUI refresh state and parallel executor filtering.

## Acceptance Criteria

- In parallel mode, if a selected change becomes uncommitted between the last refresh and `F5`, the TUI does not leave that row in `Queued` after start/resume is attempted.
- `F5` from both Select and Stopped modes uses up-to-date eligibility rules that stay aligned with the backend parallel execution filter.
- When a change is rejected during parallel start because it is uncommitted, the TUI shows a warning or log explaining the rejection.
- Regression tests cover the case where the UI initially believes a change is eligible but the backend rejects it at parallel start time.

## Out of Scope

- Changing the definition of parallel eligibility itself.
- Changing dependency analysis, slot scheduling, or worktree merge policies beyond the state-reconciliation needed for this bug.
