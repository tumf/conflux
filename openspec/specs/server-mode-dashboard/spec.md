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
