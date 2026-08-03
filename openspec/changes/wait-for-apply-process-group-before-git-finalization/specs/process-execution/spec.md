## ADDED Requirements

### Requirement: Apply process-group cleanup gates repository finalization

On Unix, when Conflux observes a stable Apply completion condition while its owned command is still running, it MUST complete bounded process-group cleanup before any repository finalization or handoff. Confirmed quiescence requires both reaping the spawned leader and probing the original process group as absent. Signal-0 success means present, `ESRCH` means absent, and `EPERM` or any other error means unknown. Present or unknown MUST fail closed. Windows job-object behavior remains unchanged.

#### Scenario: descendant releases synthetic Git lock before finalization

- **GIVEN** a Unix Apply command has reached a stable completion condition
- **AND** a synthetic descendant in the owned process group holds the managed-worktree `index.lock`
- **WHEN** completion-grace cleanup begins
- **THEN** Conflux does not start a WIP snapshot, cleanup review, or final Apply commit while the descendant remains
- **AND** finalization may begin only after the leader is reaped and the process-group probe reports `ESRCH`

#### Scenario: leader exits before descendant

- **GIVEN** the owned Unix Apply leader has been reaped
- **AND** signal 0 still reports the process group present
- **WHEN** Conflux evaluates cleanup completion
- **THEN** leader exit alone does not establish quiescence
- **AND** bounded cleanup continues or returns an unconfirmed failure

#### Scenario: process-group presence is unknown

- **GIVEN** leader reaping has completed
- **AND** the process-group probe returns `EPERM` or another error besides `ESRCH`
- **WHEN** Conflux evaluates cleanup completion
- **THEN** it treats quiescence as unconfirmed
- **AND** it does not begin repository finalization or successful handoff

#### Scenario: forceful cleanup cannot prove quiescence

- **GIVEN** graceful cleanup did not confirm both required conditions
- **AND** the forceful cleanup deadline also expires without leader reaping plus `ESRCH`
- **WHEN** cleanup returns to Apply
- **THEN** Apply fails with phase, PGID, leader-reap, probe, and signal diagnostics
- **AND** no WIP/final commit, cleanup review, rejecting handoff, or Acceptance starts
