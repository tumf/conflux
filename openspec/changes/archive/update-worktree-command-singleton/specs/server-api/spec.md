## ADDED Requirements

### Requirement: busy root API requests fail fast
server-mode API は busy な base/worktree root を対象とする競合操作を受けた場合、要求を待機させず即時 `409 Conflict` を返さなければならない（MUST）。

#### Scenario: sync request fails fast on busy root
**Given** a base root for project `p1` already has an active command
**When** `POST /api/v1/projects/p1/git/sync` is called for the same root
**Then** the server responds with `409 Conflict`
**And** the response body identifies that the root is already busy

### Requirement: full-state includes active commands
WebSocket `full_state` messages and equivalent server-mode state payloads SHALL include the currently active command list so clients can reconstruct busy root state after reconnect or reload.

#### Scenario: full-state carries active command snapshot
**Given** a dashboard client is connected
**When** the server sends a `full_state` update while one or more roots are busy
**Then** the payload includes an `active_commands` field
**And** each entry includes `project_id`, root identity, and `operation`
