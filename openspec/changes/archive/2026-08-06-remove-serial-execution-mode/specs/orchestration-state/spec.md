## MODIFIED Requirements

### Requirement: Execution Mode Determines Archive Terminal Semantics

`ChangeArchived` SHALL NOT be terminal by itself. Archive completion SHALL enter post-archive handling according to reducer-owned base-mutating lane state and the configured merge or push action:

- when another non-terminal change occupies the base-mutating lane with `Resolving` or `Rejecting`, the archived change SHALL become `ResolveWait` and remain scheduler-consumable;
- when no base-mutating lane blocker exists and no concrete manual blocker has been observed, the archived change SHALL become active `Resolving`;
- only concrete manual deferral evidence SHALL set `MergeWait`.

#### Scenario: archive without blocker enters resolving

**Given**: no other non-terminal change is `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `ActivityState::Resolving`
**And**: `alpha` is not terminal solely because archive completed

#### Scenario: archive waits behind active base-mutating lane

**Given**: change `beta` is non-terminal and actively `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `WaitState::ResolveWait`
**And**: `alpha` remains scheduler-consumable

### Requirement: Rejection Flow Execution

The system SHALL execute the existing rejection flow when acceptance returns a `Blocked` verdict. The sole managed-worktree execution service SHALL own this flow and preserve its repository-verifiable marker, commit isolation, and worktree cleanup behavior.

#### Scenario: Rejection flow commits only REJECTED.md and cleans worktree

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the worktree execution service runs rejection handling
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** the base commit includes only that marker
- **AND** the rejected worktree is deleted

### Requirement: Parallel mode treats archive as merge-wait

An archived managed-worktree change MUST enter reducer-owned post-archive handling. It MUST use `MergeWait` only for a concrete deferred condition that requires waiting or user intervention; recoverable internal preconditions MUST proceed automatically.

#### Scenario: archived change does not stay merge wait for recoverable branch initialization

- **GIVEN** an archived change enters merge handling
- **AND** the Git base branch has not yet been cached
- **WHEN** the system can initialize that base branch from repository state
- **THEN** the change proceeds through merge handling
- **AND** it does not remain in `MergeWait`
