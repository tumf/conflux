## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish `Blocked` from terminal `Rejected`.

A `Blocked` change is one where the current apply or rejection-review path cannot proceed until additional information, specification clarification, dependency resolution, or explicit operator action is available, but the change itself remains valid and resumable.

For a `Blocked` change, the runtime state SHALL preserve:
- queue intent when applicable
- active worktree reference
- current WIP / latest iteration snapshot context
- tasks progress
- blocker reason and unblock metadata
- non-terminal resumable lifecycle status

The terminal result MUST continue to include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejection review or acceptance has determined the change should be closed and the base branch has recorded a durable rejection reason.

#### Scenario: apply blocked state preserves resumable worktree context
- **GIVEN** apply reports a recoverable blocker for a change
- **WHEN** the reducer applies the blocked execution input
- **THEN** the lifecycle state becomes `Blocked`
- **AND** terminal result remains `None`
- **AND** the runtime preserves worktree reference, WIP context, tasks progress, and blocker metadata
- **AND** the derived display status is `blocked`

#### Scenario: rejecting review blocked outcome preserves change instead of rejecting it
- **GIVEN** a change is in `Rejecting`
- **AND** rejecting review returns a blocked-hold outcome rather than confirm or resume
- **WHEN** the reducer applies the completion event
- **THEN** the lifecycle state becomes `Blocked`
- **AND** terminal result remains `None`
- **AND** the existing worktree remains attached to the change
- **AND** the derived display status is `blocked`

#### Scenario: blocked change can be explicitly re-queued or retried into applying
- **GIVEN** a change is in `Blocked` non-terminal state
- **AND** its worktree and unblock metadata are still present
- **WHEN** an explicit retry or resume action is issued after the unblock condition is satisfied
- **THEN** the reducer transitions the change back to `Applying`
- **AND** the prior worktree context is reused rather than recreated from scratch

#### Scenario: rejected change remains terminal and non-resumable
- **GIVEN** rejection flow has committed `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **WHEN** the reducer applies the rejected completion event
- **THEN** terminal state becomes `Rejected`
- **AND** the derived display status is `rejected`
- **AND** the change cannot be resumed through the blocked retry path
