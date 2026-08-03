## MODIFIED Requirements

### Requirement: Apply completion grace requires stable repository completion

When runtime observes an apply completion condition while the apply child is still running, it MAY start a bounded grace period before terminating the child. Runtime MUST re-evaluate the same repository completion condition when the grace period expires and MUST terminate the child only if that condition remains present. If completion disappears or changes during the grace period, runtime MUST cancel or restart the deadline for the current condition and continue apply.

On Unix, after stable completion causes termination, runtime MUST NOT create a WIP snapshot, run cleanup review, create the final Apply commit, enter rejecting handoff, or dispatch Acceptance until the spawned leader is reaped and a signal-0 probe reports the original process group absent with `ESRCH`. A present or unknown process group MUST fail closed rather than produce successful Apply completion.

#### Scenario: transient task completion does not terminate apply

- **GIVEN** `tasks.md` becomes complete while the apply child remains running
- **AND** runtime starts its completion grace period
- **AND** `tasks.md` becomes incomplete before the grace period expires
- **WHEN** runtime rechecks repository state at the deadline
- **THEN** it does not terminate the child based on the stale completion observation
- **AND** apply continues until a completion condition remains stable or the child exits

#### Scenario: stable completion waits for Unix process-group quiescence

- **GIVEN** `tasks.md` remains complete through the grace deadline
- **AND** runtime terminates the Unix Apply process group
- **WHEN** the leader exits but another group member remains or group presence is unknown
- **THEN** runtime does not begin repository finalization or handoff
- **AND** it proceeds only after leader reaping plus `ESRCH`, or fails Apply after bounded cleanup expires
