## MODIFIED Requirements

### Requirement: Parallel mode treats archive as merge-wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the wait state becomes `MergeWait`
- **AND** the terminal state remains `None`
- **AND** the derived display status is `merge wait`

A parallel archived change MUST leave `MergeWait` as soon as merge handling can proceed automatically. Internal recoverable preconditions such as lazy base-branch initialization MUST NOT keep the change in `MergeWait`; only deferred merge conditions that truly require waiting or user intervention may do so.

#### Scenario: archived change does not stay merge wait for recoverable branch initialization
- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event
- **AND** merge handling discovers that the Git base branch has not yet been cached
- **WHEN** the system can initialize that base branch from repository state
- **THEN** the change proceeds through merge handling
- **AND** the reducer does not preserve `merge wait` solely because of the missing cached branch name

#### Scenario: archived change enters error instead of merge wait on unrecoverable branch discovery failure
- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event
- **AND** merge handling cannot determine the base branch because the repository is detached HEAD
- **WHEN** the failure is reported
- **THEN** the change is treated as an execution error
- **AND** the reducer does not classify the failure as `merge wait`
