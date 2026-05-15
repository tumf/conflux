## Requirements

### Requirement: Server mode dashboard project state must expose display-ready project identity

The server-mode dashboard state endpoints SHALL provide each project with explicit repository and branch fields suitable for direct UI rendering, rather than requiring the dashboard to infer them from a combined display string.

#### Scenario: Newly added project appears in dashboard list

**Given** a project has been added to the server registry with a git remote URL and branch
**When** the dashboard receives project state from `GET /api/v1/projects/state` or a WebSocket `full_state` update
**Then** the project payload includes a non-empty `repo` field derived from the remote URL
**And** the payload includes the configured `branch`
**And** the dashboard can render the project identity without showing only `/`

#### Scenario: Standard git remote omits .git suffix in repo display

**Given** a project remote URL ends with a repository name followed by `.git`
**When** the server constructs the dashboard project snapshot
**Then** the `repo` field contains the repository name without the `.git` suffix

#### Scenario: Unusual remote URL still avoids empty project identity

**Given** a project remote URL cannot be cleanly reduced to a normal repository basename
**When** the server constructs the dashboard project snapshot
**Then** it falls back to the best available non-empty repo label
**And** the project payload still allows the dashboard to avoid rendering only `/`


#


#


#

## Requirements

### Requirement: Server-mode dashboard shows per-project sync state

The server-mode dashboard SHALL render each project's remote synchronization state using display-ready metadata from the server rather than inferring status from logs.

#### Scenario: project list shows behind state
- **GIVEN** a project payload reports `sync_state = behind` and `behind_count > 0`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that the remote branch is ahead of local
- **AND** the row shows the behind count in a display-ready form

#### Scenario: project list shows ahead state
- **GIVEN** a project payload reports `sync_state = ahead` and `ahead_count > 0`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that the local branch is ahead of remote
- **AND** the row shows the ahead count in a display-ready form

#### Scenario: project list shows diverged state
- **GIVEN** a project payload reports `sync_state = diverged`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that local and remote have diverged
- **AND** the row can display both ahead and behind counts

#### Scenario: project list shows unknown state after check failure
- **GIVEN** a project payload reports `sync_state = unknown`
- **WHEN** the dashboard renders the project list
- **THEN** the project row indicates that sync state could not be determined
- **AND** the dashboard can surface the latest check failure message when available

### Requirement: Dashboard worktree deletion shows row-level pending state

The server-mode dashboard SHALL show a per-worktree pending deletion state for the worktree branch whose delete request is currently in flight, without requiring users to infer progress from the global loading state or confirmation dialog alone.

#### Scenario: Target worktree row shows deleting state while delete is pending

**Given**: the dashboard Worktrees panel is rendering worktrees `feature-a` and `feature-b`
**When**: the user confirms deletion of worktree branch `feature-a` and the delete API request has not completed
**Then**: the `feature-a` worktree row displays a visible deleting indicator with a spinner or equivalent progress affordance
**And**: the `feature-b` worktree row does not display the deleting indicator

#### Scenario: Deleting worktree row suppresses conflicting interactions

**Given**: worktree branch `feature-a` is currently being deleted from the dashboard
**When**: the dashboard renders the `feature-a` worktree row
**Then**: merge and delete controls for that row are disabled or unavailable
**And**: clicking the row does not change the active file-browse worktree selection

#### Scenario: Deleting state clears after delete success

**Given**: worktree branch `feature-a` is currently displayed as deleting
**When**: the delete API request succeeds
**Then**: the dashboard clears the deleting indicator for `feature-a`
**And**: refreshes the worktree list
**And**: clears any file-browse context that referenced `feature-a`

#### Scenario: Deleting state clears after delete failure

**Given**: worktree branch `feature-a` is currently displayed as deleting
**When**: the delete API request fails
**Then**: the dashboard clears the deleting indicator for `feature-a`
**And**: leaves the worktree row available in the current list
**And**: surfaces the existing delete failure error to the user
