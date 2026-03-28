## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active, or a deferred change that has been auto-promoted into the next resolve flow after dependency or merge preconditions are satisfied.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

Workspace observation alone MAY recover `MergeWait` for archived-but-unmerged workspaces, but it MUST NOT erase reducer-owned auto-resolve intent that was established from `MergeDeferred` reason tracking.

When the user manually requests resolve for a `MergeWait` change and the merge cannot start because the base branch is dirty, the system MUST classify the dirty-base reason.
If the reason indicates another merge or resolve is already in progress and the blocked change can be retried automatically after that prerequisite completes, the change MUST enter reducer-owned `ResolveWait` rather than reverting to manual-only `MergeWait`.
If the reason indicates uncommitted changes or another condition that requires user repair of the base workspace, the change MUST revert to `MergeWait`.

#### Scenario: Auto-promoted deferred change enters reducer-owned resolve wait
- **GIVEN** a change was deferred because another merge or resolve had to complete first
- **WHEN** that prerequisite completes and the reducer receives the promotion signal
- **THEN** the change enters reducer-owned `ResolveWait` or `Resolving`
- **AND** subsequent refresh reconciliation does not regress it to `MergeWait`

#### Scenario: Workspace refresh does not overwrite auto-resolve intent
- **GIVEN** a change has already been auto-promoted from deferred merge waiting into reducer-owned resolve intent
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer preserves the auto-resolve wait state
- **AND** the displayed status does not regress to a stale manual-wait state

#### Scenario: Manual resolve blocked by another active resolve becomes resolve pending
- **GIVEN** a change is currently in `MergeWait`
- **AND** the user manually requests resolve for that change
- **AND** the base branch is dirty only because another merge or resolve is already in progress
- **WHEN** the resolve attempt is classified as auto-resumable deferred work
- **THEN** the change enters reducer-owned `ResolveWait`
- **AND** the displayed status is `resolve pending`
- **AND** the change is eligible for automatic retry after the prerequisite completes

#### Scenario: Manual resolve blocked by uncommitted base changes stays merge wait
- **GIVEN** a change is currently in `MergeWait`
- **AND** the user manually requests resolve for that change
- **AND** the base branch is dirty because of uncommitted changes that require user repair
- **WHEN** the resolve attempt is classified as manual-intervention-required deferred work
- **THEN** the change remains or returns to `MergeWait`
- **AND** the displayed status is `merge wait`
- **AND** no automatic retry is scheduled until the user repairs the workspace
