## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting

Parallel analysis MUST target only queued changes. Scheduler-local queued work MUST reconcile reducer-visible queue intent without converting unrelated preserved worktrees into implicit operator intent. A workspace-derived archived-dirty repair candidate MUST be added only when its change ID is admitted to the current run snapshot through initial explicit targets or a later explicit queue addition. Repository evidence MUST continue to determine the resumed workflow phase, but repository evidence alone MUST NOT admit an unselected change to the current run.

Reducer-owned resolve/reject wait intent remains scheduler-consumable independently of ordinary queued apply candidates. Terminal merged, archived, rejected, and recoverable terminal-error stop gates remain unchanged.

#### Scenario: Unselected archived-dirty workspace is excluded

**Given**: A TUI parallel run admits change `fresh`
**And**: Preserved worktree `stale` has archive files present, its active change directory absent, and archive commit finalization incomplete
**And**: `stale` has no reducer queue intent and is not in the current run snapshot
**When**: Scheduler queue reconciliation scans existing worktrees
**Then**: `stale` is not added to scheduler-local queued work
**And**: `stale` is not included in dependency analysis
**And**: Apply, acceptance, archive finalization, and merge do not start for `stale`
**And**: The preserved `stale` worktree is not mutated

#### Scenario: Initially admitted archived-dirty workspace remains recoverable

**Given**: Archived-dirty change `stale` is an initial explicit target of the current run
**And**: Workspace and base-tree evidence show it is not merged
**When**: Queue reconciliation evaluates the preserved workspace
**Then**: `stale` may enter scheduler-local repair work
**And**: The executor resumes the repository-derived archive finalization or archive-complete handoff
**And**: Completed apply work is not rerun

#### Scenario: Dynamically admitted archived-dirty workspace remains recoverable

**Given**: Archived-dirty change `stale` was not an initial target
**And**: The operator explicitly adds `stale` to the Running-mode queue
**When**: The reducer adds `stale` to current-run membership and queue reconciliation runs
**Then**: `stale` may enter scheduler-local repair work
**And**: Recovery uses current workspace/git/base-tree evidence

#### Scenario: Manual merge wait is not ordinary recovery

**Given**: Archived change `stale` has reducer-visible manual `MergeWait`
**When**: Queue reconciliation scans its worktree without a newly accepted `ResolveMerge` command
**Then**: `stale` is not added as ordinary archived-dirty queued work
**And**: Manual merge retry remains explicit reducer-owned scheduler intent
