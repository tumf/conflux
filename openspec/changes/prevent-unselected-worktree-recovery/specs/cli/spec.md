## MODIFIED Requirements

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL start with all changes unselected and SHALL NOT auto-queue any change. Active-change refresh and preserved worktree discovery MAY populate display/catalog state, but MUST NOT create execution eligibility. Only marked IDs accepted by Start or later accepted shared operator queue/retry intent may enter ordinary execution.

#### Scenario: TUI startup clears execution marks

**When**: The user starts the TUI
**Then**: All changes are unselected by default
**And**: No changes are automatically queued or admitted to execution

#### Scenario: Initial all-change refresh preserves selection boundary

**Given**: `fresh` is marked and `stale` is unmarked
**And**: `stale` has a preserved recoverable worktree
**When**: The user starts processing `fresh`
**And**: The initial `ChangesRefreshed` event contains both changes
**Then**: Only `fresh` enters ordinary execution eligibility
**And**: Catalog registration of `stale` does not queue, analyze, or execute it

#### Scenario: Explicit later queue enables preserved workspace recovery

**Given**: `stale` remains visible and unqueued with a preserved recoverable worktree
**When**: The user explicitly adds `stale` to the Running-mode queue
**Then**: Shared reducer queue intent makes `stale` eligible
**And**: Conflux derives its resume phase from workspace and Git evidence

#### Scenario: Queue removal revokes recovery eligibility

**Given**: `stale` was explicitly queued and has not yet completed
**When**: The user removes or successfully stops and dequeues `stale`
**Then**: Preserved worktree discovery does not requeue it
**And**: Explicit requeue is required before it can execute again
