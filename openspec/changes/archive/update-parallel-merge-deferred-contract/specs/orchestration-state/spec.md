## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent for auto-resumable deferred merges, including deferred changes promoted after archive when another resolve or merge prerequisite must complete first.

`MergeWait` SHALL remain the manual-intervention wait state for deferred merges that cannot be retried automatically, such as dirty-base conditions.

Workspace observation alone MAY recover `MergeWait` for archived-but-unmerged workspaces, but it MUST NOT erase reducer-owned auto-resolve intent or reclassify auto-resumable deferred merges based on free-form reason strings.

#### Scenario: auto-resumable-merge-deferred-enters-resolve-wait

**Given**: A change receives a deferred merge result that is explicitly classified as auto-resumable
**When**: The reducer applies that execution event
**Then**: the change enters reducer-owned `ResolveWait`
**And**: later refresh reconciliation does not regress it to `MergeWait`

#### Scenario: manual-deferred-merge-remains-merge-wait

**Given**: A change receives a deferred merge result that requires manual intervention
**When**: The reducer applies that execution event
**Then**: the change remains in `MergeWait`
**And**: it is not added to the auto-resumable resolve wait queue
