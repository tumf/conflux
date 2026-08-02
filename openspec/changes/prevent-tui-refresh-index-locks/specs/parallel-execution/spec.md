## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting

Parallel analysis MUST target only queued changes. Repository-monitoring queries used to classify committed and uncommitted changes for TUI refresh, parallel startup, or queue filtering MUST preserve staged, unstaged, renamed, and untracked change detection without taking optional Git index locks. Monitoring MUST NOT contend with repo-mutating lifecycle work merely to refresh index stat information.

Disabling optional locks MUST NOT weaken path classification: active change directories remain detectable, while archive entries, hidden directories, and unrelated paths remain excluded. Commands that intentionally mutate the index are not monitoring queries and retain their existing locking and failure behavior.

#### Scenario: Periodic refresh does not contend with an on_merged mutation

**Given**: A TUI refresh is classifying uncommitted OpenSpec changes in the root repository
**And**: An `on_merged` hook may stage and commit release artifacts in the same repository
**When**: The refresh executes its Git status query
**Then**: The query does not request an optional index lock
**And**: Current staged, unstaged, renamed, and untracked OpenSpec changes remain visible to classification

#### Scenario: Monitoring does not refresh a stale index

**Given**: The Git index stat information can be refreshed from current worktree metadata
**When**: Conflux runs the uncommitted-change monitoring query
**Then**: The query reports current working-tree changes
**And**: The query does not replace or update the Git index as an optional optimization

#### Scenario: Monitoring exclusions remain stable

**Given**: Uncommitted paths exist under an active change, the archive directory, a hidden change directory, and an unrelated repository directory
**When**: Conflux classifies change IDs with uncommitted files
**Then**: Only the active change ID is returned
