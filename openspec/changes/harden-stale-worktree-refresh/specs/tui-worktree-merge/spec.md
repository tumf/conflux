## MODIFIED Requirements

### Requirement: Worktree Commits Ahead Detection

TUI SHALL detect whether an automatically inspectable worktree branch has commits ahead of the base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for eligible cache misses. Ineligible worktrees and unchanged cache hits SHALL NOT spawn duplicate ahead/conflict commands. A skipped observation MUST NOT be represented as `has_commits_ahead = false` when that value would enable or suppress a merge action incorrectly.

<!-- Expected canonical result after archive: commits-ahead and conflict checks remain parallel for eligible cache misses but are not executed for stale/non-active worktrees or unchanged observations. -->

#### Scenario: Eligible active worktree is inspected

- **GIVEN** a secondary worktree maps to a current active or rejected change
- **AND** no matching cached observation exists
- **WHEN** the worktree list is loaded
- **THEN** commits-ahead detection and conflict checking run in parallel
- **AND** both complete before the checked observation is returned

#### Scenario: Ineligible worktree is fail-closed

- **GIVEN** a secondary worktree does not map to a current active or rejected change
- **WHEN** the worktree list is loaded
- **THEN** commits-ahead and conflict commands are not executed for it
- **AND** merge eligibility does not infer clean or not-ahead status from the skipped checks
