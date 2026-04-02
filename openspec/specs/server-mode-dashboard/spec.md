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
