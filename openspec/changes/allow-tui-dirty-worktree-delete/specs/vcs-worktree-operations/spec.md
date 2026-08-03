## ADDED Requirements

### Requirement: Explicit dirty discard is a local deletion permission

Managed worktree deletion MUST treat known dirty-content discard as an explicit permission independent from teardown skipping. The permission MUST default to disabled. It MAY waive only a known dirty-state refusal and MUST NOT waive main-worktree, active/deleting target, unresolved base merge, unknown dirty state, branch-identity mismatch, or commits-ahead guards. Only the local TUI destructive-confirmation path MAY enable this permission; remote deletion MUST remain fail-closed and MUST NOT expose it.

#### Scenario: Ordinary deletion refuses known dirty state

**Given**: A managed worktree has known uncommitted or untracked changes
**And**: Dirty-discard permission is disabled
**When**: Managed deletion eligibility is evaluated
**Then**: Deletion is refused as dirty
**And**: Teardown and Git removal are not executed

#### Scenario: Explicit local dirty discard proceeds

**Given**: A non-main managed worktree has known dirty changes and no commits ahead
**And**: No active/deleting ownership, unresolved base merge, or identity mismatch exists
**And**: The local TUI supplies explicit dirty-discard permission after destructive confirmation
**When**: Managed deletion revalidates the target under the repository mutation guard
**Then**: Deletion may proceed
**And**: Teardown runs unless independently skipped
**And**: The system records that dirty content was intentionally discarded before removal

#### Scenario: Dirty discard does not authorize unknown state

**Given**: The system cannot determine whether the managed worktree is dirty
**And**: Dirty-discard permission is enabled
**When**: Managed deletion eligibility is evaluated
**Then**: Deletion is refused as dirty-state unknown
**And**: The worktree is retained

#### Scenario: Dirty discard does not waive other guards

**Given**: Dirty-discard permission is enabled
**And**: The target is main, active, deleting, branch-identity mismatched, base-merge blocked, or ahead of base
**When**: Managed deletion is evaluated or revalidated
**Then**: Deletion is refused for the applicable non-dirty guard
**And**: The worktree is retained

#### Scenario: Skip teardown alone does not permit dirty discard

**Given**: A managed worktree has known dirty changes
**And**: Teardown skipping is enabled but dirty-discard permission is disabled
**When**: Managed deletion eligibility is evaluated
**Then**: Deletion is refused as dirty
**And**: Git removal is not executed

#### Scenario: Remote callers cannot request dirty discard

**Given**: A remote client addresses a dirty managed worktree
**When**: The client requests deletion or submits unsafe dirty-discard, force, teardown-skip, path, or branch parameters
**Then**: Normal dirty deletion is refused or the unsafe request shape is rejected
**And**: Remote removal is not invoked
