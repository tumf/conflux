## ADDED Requirements

### Requirement: Apply process-group cleanup gates repository finalization

When Conflux observes a stable Apply completion condition while its owned command is still running, it MUST complete bounded process-group cleanup and confirm that no owned process-group members remain before starting any Conflux-owned index-mutating Git operation, cleanup review, rejecting handoff, or Acceptance handoff for that managed worktree. Leader exit alone MUST NOT be treated as process-group quiescence. If quiescence cannot be confirmed, Apply MUST fail with actionable cleanup diagnostics and MUST NOT report successful completion.

#### Scenario: descendant releases Git lock before finalization

- **GIVEN** an Apply command has reached a stable completion condition
- **AND** a descendant in the owned process group still holds the managed worktree `index.lock`
- **WHEN** the completion grace period expires
- **THEN** Conflux runs bounded graceful-then-forceful process-group cleanup
- **AND** Conflux does not start a WIP snapshot, cleanup review, or final Apply commit while that descendant remains
- **AND** repository finalization may begin only after no owned process-group members remain

#### Scenario: process-group cleanup cannot prove quiescence

- **GIVEN** an Apply command has reached a stable completion condition
- **AND** bounded graceful and forceful cleanup cannot confirm that the owned process group is empty
- **WHEN** the cleanup budget is exhausted
- **THEN** Apply fails with process-group cleanup diagnostics
- **AND** no WIP snapshot or final Apply commit is created after the unconfirmed cleanup
- **AND** cleanup review, rejecting handoff, and Acceptance are not dispatched

#### Scenario: leader exits before descendant

- **GIVEN** the owned Apply process-group leader exits during cleanup
- **AND** at least one owned descendant remains alive
- **WHEN** Conflux evaluates cleanup completion
- **THEN** it does not classify the process group as quiescent from leader exit alone
- **AND** it continues the bounded cleanup sequence until quiescence is confirmed or cleanup fails
