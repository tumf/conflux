## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

When apply execution records a rejection proposal by generating `openspec/changes/<change_id>/REJECTED.md`, the runtime SHALL transition the workspace into a dedicated `rejecting` stage even if `tasks.md` still contains unchecked implementation tasks. A workspace in `rejecting` SHALL NOT re-enter the normal apply retry loop or empty-WIP stall policy before rejection review runs.

`APPLY_BLOCKED/marker.md` SHALL remain the marker for resumable apply-side stalled handoff. `REJECTED.md` SHALL remain the marker for terminal-rejection proposal review and MUST route to rejecting review.

#### Scenario: apply-generated REJECTED.md skips apply stall detection

- **GIVEN** apply execution for change `fix-auth` generates `openspec/changes/fix-auth/REJECTED.md`
- **AND** `openspec/changes/fix-auth/tasks.md` still has unchecked tasks
- **WHEN** the apply iteration completes or is grace-terminated after observing the handoff artifact
- **THEN** the runtime exits the apply retry loop as a rejecting handoff
- **AND** the empty WIP stall detector is not used to convert the change into terminal `Error`
- **AND** the next orchestration step is rejection review

#### Scenario: APPLY_BLOCKED remains distinct from REJECTED

- **GIVEN** apply execution for change `fix-auth` generates `openspec/changes/fix-auth/APPLY_BLOCKED/marker.md`
- **WHEN** the apply loop evaluates handoff artifacts
- **THEN** the runtime treats the change as a resumable stalled/apply hold
- **AND** it does not route to terminal rejection review solely because an apply blocker exists
