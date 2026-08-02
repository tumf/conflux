## ADDED Requirements

### Requirement: Repository monitoring avoids optional Git index locks

Repository-monitoring queries used to classify uncommitted OpenSpec changes for TUI refresh, parallel startup, or queue filtering MUST preserve current worktree classification without requesting optional Git index locks. Optional-lock suppression MUST be local to the monitoring child command and MUST NOT alter index-mutating Git commands or process-wide environment state.

The monitoring query MUST continue to classify active change paths from staged, unstaged, renamed, and untracked state while excluding clean committed paths, archive entries, hidden change directories, ignored files, and unrelated repository paths. Results MAY represent a point-in-time observation and MAY converge on concurrent mutations during a later poll.

#### Scenario: Periodic refresh does not request an optional index lock

**Given**: A TUI refresh is classifying uncommitted OpenSpec changes in the root repository
**And**: A lifecycle hook may stage and commit files in the same repository
**When**: The refresh executes its Git status query
**Then**: The query disables Git optional locks for that child command
**And**: Repo-mutating Git commands retain their existing lock behavior

#### Scenario: Active change classifications remain visible

**Given**: Active change paths include staged or unstaged additions, modifications, deletions, an untracked file, and a rename within the same change
**When**: Conflux classifies change IDs with uncommitted files
**Then**: The affected active change IDs are returned
**And**: A clean committed path is not returned as an uncommitted change

#### Scenario: Monitoring exclusions remain stable

**Given**: Paths exist under an active change, the archive directory, a hidden change directory, an ignored path, and an unrelated repository directory
**When**: Conflux classifies change IDs with uncommitted files
**Then**: Only the qualifying active change ID is returned

#### Scenario: Monitoring does not persist an optional index refresh

**Given**: A repository fixture has index stat information that a normal status command demonstrably persists
**When**: Conflux runs the uncommitted-change monitoring query
**Then**: The query reports current working-tree changes
**And**: The complete Git index bytes remain unchanged
