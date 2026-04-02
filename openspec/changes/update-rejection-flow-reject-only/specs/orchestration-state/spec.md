## MODIFIED Requirements

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Blocked` verdict, including blocked verdicts that originated from apply execution through a rejection proposal file. The rejection flow MUST write and commit only `openspec/changes/<change_id>/REJECTED.md` on the base branch. The rejection flow MUST NOT stage, merge, or commit any other files from the rejected worktree, including proposal, tasks, spec deltas, or product code changes. The runtime SHALL treat the `REJECTED.md` marker commit itself as the durable rejection record and SHALL NOT require `openspec resolve <change_id>` as part of the rejection flow.

#### Scenario: rejection flow commits only REJECTED marker

- **GIVEN** acceptance confirms a blocked verdict for `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** the base branch commit includes `openspec/changes/fix-auth/REJECTED.md`
- **AND** no other files from the rejected worktree are staged or committed

#### Scenario: rejection flow does not invoke openspec resolve

- **GIVEN** acceptance confirms a blocked verdict for `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** `openspec resolve fix-auth` is not invoked
- **AND** rejection completion does not depend on OpenSpec CLI availability

#### Scenario: worktree cleanup occurs after reject marker commit

- **GIVEN** the rejection flow has committed `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **WHEN** the flow completes
- **THEN** the rejected worktree is cleaned up
- **AND** the rejected change remains represented by the base-side `REJECTED.md` marker
