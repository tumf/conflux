# Design: REJECTED.md handoff from apply to rejecting

## Current behavior

`src/execution/apply.rs` recognizes apply completion during a still-running child process only when tasks are complete or `APPLY_BLOCKED/marker.md` exists. If the apply agent writes `openspec/changes/<change_id>/REJECTED.md` while tasks remain unchecked, the apply loop continues and can eventually trip empty-WIP stall detection.

## Target behavior

Apply completion detection should distinguish three successful handoff categories:

- `TasksComplete`: implementation tasks are complete and normal acceptance/archive can proceed.
- `BlockedHandoff`: `APPLY_BLOCKED/marker.md` exists and the change should become a resumable stalled/apply hold.
- `RejectingHandoff`: `REJECTED.md` exists and the change should enter dedicated rejection review.

`RejectingHandoff` is not an apply failure and is not subject to empty WIP stall detection. It is a control-flow handoff into `run_rejection_review`.

## Runtime routing

The apply result should expose enough structured information for dispatch/orchestration to route without reparsing logs. Parallel dispatch should emit/record `Rejecting` status and call the existing rejection review path. Resume detection already maps a workspace containing `REJECTED.md` to `WorkspaceState::Rejecting`; this change makes the live apply path match that resume behavior.

## Verification approach

Regression coverage should simulate an apply command that writes `openspec/changes/<change_id>/REJECTED.md` while tasks remain incomplete. The test must assert that the result is a rejecting handoff and not an `AgentCommand` stall error.
