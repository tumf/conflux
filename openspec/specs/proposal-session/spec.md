
### Requirement: proposal-session-opencode-config

Proposal sessions must not auto-generate or inject opencode configuration files. When no `OPENCODE_CONFIG` is specified in `proposal_session.transport_env`, opencode uses its own default configuration. Users may optionally override the config by setting `OPENCODE_CONFIG` in their `.cflx.jsonc`.

#### Scenario: default-no-config

**Given**: No `OPENCODE_CONFIG` is set in `proposal_session.transport_env`
**When**: A proposal session is created
**Then**: The ACP subprocess is spawned without `OPENCODE_CONFIG` in its environment, and opencode uses its built-in defaults

#### Scenario: user-custom-config

**Given**: `OPENCODE_CONFIG` is set to `/path/to/custom/opencode.json` in `proposal_session.transport_env`
**When**: A proposal session is created
**Then**: The ACP subprocess is spawned with `OPENCODE_CONFIG=/path/to/custom/opencode.json` in its environment


### Requirement: auto-generate-opencode-proposal-config

Auto-generation of `opencode-proposal.jsonc` with `"mode": "spec"` is removed because opencode does not support arbitrary mode values in external config and the default opencode configuration is sufficient.

## Requirements

### Requirement: Proposal Session Database-Backed Lifecycle

The ProposalSessionManager SHALL accept an optional ServerDb reference and persist session lifecycle events (creation, status changes, closure) to SQLite when available.

#### Scenario: Session survives server restart

- **GIVEN** an active proposal session with a valid worktree on disk
- **WHEN** the server process is restarted
- **THEN** the session is restored from the database with a re-spawned ACP subprocess and the same session ID, project ID, worktree path, and branch name

#### Scenario: TimedOut session restored as Active

- **GIVEN** a proposal session with status `timed_out` in the database and its worktree still exists
- **WHEN** the server restarts
- **THEN** the session is restored with a new ACP subprocess and its status is set back to `active`

#### Scenario: Activity updates throttled

- **GIVEN** an active proposal session receiving frequent WebSocket messages
- **WHEN** `touch()` is called multiple times within 60 seconds
- **THEN** only the first call writes to the database; subsequent calls within the window are skipped

### Requirement: Proposal Session Message Database Persistence

The ProposalSessionManager SHALL persist chat messages to SQLite at turn boundaries for history restoration across server restarts.

#### Scenario: User prompt persisted immediately

- **GIVEN** a user sends a prompt to an active proposal session
- **WHEN** `record_user_prompt` is called
- **THEN** the user message is immediately inserted into the `proposal_session_messages` table

#### Scenario: Assistant message persisted on turn complete

- **GIVEN** an assistant turn is in progress with accumulated text chunks
- **WHEN** `complete_active_turn` is called
- **THEN** the complete assistant message (content + tool_calls) is inserted into the `proposal_session_messages` table
