## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active, or a deferred change that has been auto-promoted into the next resolve flow after dependency or merge preconditions are satisfied.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

Workspace observation alone MAY recover `MergeWait` for archived-but-unmerged workspaces, but it MUST NOT erase reducer-owned auto-resolve intent that was established from `MergeDeferred` reason tracking.

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
