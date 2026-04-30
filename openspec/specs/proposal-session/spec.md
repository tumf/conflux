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

## Requirements

### Requirement: Proposal planning assigns verification ownership

The proposal creation workflow MUST require behavior-changing proposals to plan how each requirement will be verified so projects using Conflux can manage verification coverage explicitly instead of assuming every requirement becomes a unit test.

#### Scenario: Proposal records non-unit verification intentionally

**Given**: A user requests a feature whose UX quality cannot be judged well by unit tests
**When**: the `cflx-proposal` skill drafts the proposal
**Then**: the proposal planning guidance instructs the author to assign a verification path such as `manual` or `benchmark`
**And**: the requirement is treated as intentionally covered rather than missing unit tests

#### Scenario: Proposal records unit verification when logic is local

**Given**: A user requests a feature whose behavior is primarily local decision logic
**When**: the `cflx-proposal` skill drafts the proposal
**Then**: the proposal planning guidance instructs the author to assign `unit` verification where appropriate
**And**: related tasks identify the repository-verifiable test path

### Requirement: Proposal skill standardizes verification coverage vocabulary

The proposal creation workflow MUST provide a standard vocabulary for verification planning so Conflux-driven projects can discuss coverage consistently across proposals.

#### Scenario: Standard verification types appear in proposal guidance

**Given**: a human uses the `cflx-proposal` skill to draft a change proposal
**When**: the skill explains how to plan verification
**Then**: the guidance recognizes `unit`, `integration`, `e2e`, `manual`, `benchmark`, and `not-testable`
**And**: the guidance explains that these values represent verification ownership rather than only automated test types

### Requirement: Proposal tasks reflect verification coverage plan

The proposal workflow MUST guide authors to write tasks that make verification ownership traceable from implementation planning.

#### Scenario: Tasks connect implementation work to verification path

**Given**: a proposal contains implementation tasks
**When**: the `cflx-proposal` skill drafts `tasks.md`
**Then**: the guidance instructs the author to record the planned verification path alongside repository-verifiable evidence
**And**: reviewers can tell whether a task is intended for unit, integration, e2e, manual, or benchmark verification

### Requirement: Proposal workflow guidance must align with explicit structural validation

Proposal workflow guidance SHALL tell authors to provide explicit structure for validation-relevant fields rather than relying on wording that a validator might interpret heuristically.

When verification ownership, executable surfaces, or similar validation-relevant concerns matter, the guidance SHALL require an explicit marker or declared field recognized by the validator.

#### Scenario: proposal guidance requires explicit markers rather than wording cues
- **GIVEN** a proposal introduces validation-relevant concerns such as verification ownership or executable surfaces
- **WHEN** the workflow guidance instructs the author how to express them
- **THEN** the guidance asks for explicit markers or fields recognized by the validator
- **AND** it does not tell the author that descriptive wording alone is sufficient for machine validation
