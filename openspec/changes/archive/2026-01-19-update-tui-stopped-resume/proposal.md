# Change: TUI Stopped Resume Policy

## Why
There are cases where resuming from STOPPED state with F5 doesn't work properly, and the queue state and resume conditions during stop are ambiguous. This change formalizes the policy of preserving only execution marks during stop and restoring queued status on resume to ensure consistent behavior.

## What Changes
- Formalize the specification that queue_status is reset to NotQueued when transitioning to Stopped, while preserving execution marks ([x])
- Space operations during Stopped mode only add/remove execution marks while maintaining queue_status as NotQueued
- On F5 resume, execution-marked changes are restored to queued and processing resumes
- Unify the handling of force-stop with the same policy

## Impact
- Affected specs: cli
- Affected code: src/tui/state/events.rs, src/tui/state/modes.rs, src/tui/runner.rs, src/tui/render.rs
