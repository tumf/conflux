## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge or queue the change for resolve, rather than leaving it in MergeWait indefinitely.

#### Scenario: archive-completes-while-resolve-active

**Given**: Change A is in Resolving state and change B has just been archived in parallel mode
**When**: The ChangeArchived event for B is processed by the TUI orchestrator
**Then**: B transitions to ResolveWait (not MergeWait) and is added to the resolve queue for automatic execution after A's resolve completes

#### Scenario: archive-completes-no-active-resolve

**Given**: No resolve is currently active and change B has just been archived in parallel mode
**When**: The ChangeArchived event for B is processed by the TUI orchestrator
**Then**: An immediate merge attempt is initiated for B (via ResolveMerge command)
