## MODIFIED Requirements

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL start with all changes unselected and SHALL NOT auto-queue or auto-admit any change. Preserved worktree state MAY be displayed and used to derive the resume phase after explicit admission, but worktree discovery alone MUST NOT cause an unselected change to enter a TUI run.

#### Scenario: TUI startup clears execution marks

**When**: The user starts the TUI
**Then**: All changes are unselected by default
**And**: No changes are automatically queued or admitted to execution

#### Scenario: Starting another change does not admit preserved worktrees

**Given**: Change `fresh` is marked for execution
**And**: Unmarked change `stale` has a preserved recoverable worktree
**When**: The user starts processing
**Then**: The initial run snapshot contains `fresh` and not `stale`
**And**: Worktree reconciliation does not add `stale` to the run

#### Scenario: Explicit selection admits preserved workspace recovery

**Given**: Change `stale` has a preserved recoverable worktree and starts unselected
**When**: The user marks `stale` and starts processing
**Then**: `stale` is admitted to the run
**And**: Conflux derives the next phase from workspace/git/base-tree evidence
