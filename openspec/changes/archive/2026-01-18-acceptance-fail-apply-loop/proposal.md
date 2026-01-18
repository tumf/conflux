# Change: acceptance failure returns to apply loop with task updates

## Why
Acceptance failure currently leaves parallel execution in a completed state without returning to the apply loop, so tasks are not adjusted and the UI can remain stuck in a processing state.
We need a consistent behavior in both serial and parallel flows: acceptance failure should update tasks, record the failure reason, and resume apply with the same iteration counter.

## What Changes
- Ensure acceptance failure returns the change to the apply loop in both serial and parallel modes.
- Update tasks.md on acceptance failure by adding a follow-up task or unchecking a completed task, with the failure reason recorded.
- Keep the apply iteration counter continuous when re-entering the apply loop after acceptance failure.
- Emit error/status events so the UI reflects that the change returned to apply instead of being marked complete.

## Impact
- Affected specs: cli, parallel-execution
- Affected code: acceptance orchestration, apply loop control, task updates, TUI status events
