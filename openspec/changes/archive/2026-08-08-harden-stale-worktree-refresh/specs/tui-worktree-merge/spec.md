## MODIFIED Requirements

### Requirement: Worktree Commits Ahead Detection

TUI SHALL detect whether an automatically inspectable worktree branch has commits ahead of the base branch during worktree list loading.

Detection SHALL run in parallel with conflict checking for eligible cache misses. Both periodic TUI and periodic Web/UDS refresh SHALL share the same observation cache. Ineligible worktrees and unchanged cache hits SHALL NOT spawn duplicate ahead/conflict commands. A skipped observation MUST NOT be represented as `has_commits_ahead = false` when that value would enable or suppress a merge action incorrectly.

Periodic filtering MUST NOT remove operator control. An operator-initiated merge or deletion SHALL perform a fresh targeted observation of the selected worktree before eligibility is decided, including branches such as `ws-session-*` that do not map to an OpenSpec change. A not-inspected periodic row SHALL receive an inspection-required diagnostic rather than the false message that it has no commits ahead.

<!-- Expected canonical result after archive: commits-ahead and conflict checks remain parallel for eligible cache misses but are not executed for stale/non-active worktrees or unchanged observations. -->

#### Scenario: Eligible active worktree is inspected

- **GIVEN** a secondary worktree maps to a current active or rejected change
- **AND** no matching cached observation exists
- **WHEN** the worktree list is loaded
- **THEN** commits-ahead detection and conflict checking run in parallel
- **AND** both complete before the checked observation is returned

#### Scenario: Ineligible worktree is fail-closed during periodic refresh

- **GIVEN** a secondary worktree does not map to a current active or rejected change
- **WHEN** either periodic refresh path loads the worktree list
- **THEN** commits-ahead and conflict commands are not executed for it
- **AND** merge eligibility does not infer clean or not-ahead status from the skipped checks
- **AND** the presentation reports that inspection is required rather than reporting no commits ahead

#### Scenario: Operator merge reinspects an unclassified worktree

- **GIVEN** a `ws-session-*` or other selected worktree was not inspected by periodic refresh
- **WHEN** the operator requests its merge
- **THEN** Conflux performs a fresh targeted ahead/conflict observation
- **AND** decides merge eligibility from that current repository evidence

#### Scenario: Operator deletion reinspects a stale worktree

- **GIVEN** a stale selected worktree was not inspected by periodic refresh
- **WHEN** the operator requests its deletion
- **THEN** Conflux performs a fresh targeted observation before deletion eligibility is decided
- **AND** periodic filtering alone does not make the worktree permanently undeletable
