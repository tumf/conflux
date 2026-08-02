## MODIFIED Requirements

### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. `/api/v2` is the only versioned remote-control namespace; the removed multi-project `/api/v1` namespace MUST NOT be reintroduced. The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs.

#### Scenario: Client discovers and snapshots one process

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot
**And**: The snapshot includes distinct execution mark and queue intent, attention state, blocker and error details, action and parallel eligibility, timing, latest activity, and change-to-worktree relation when applicable

#### Scenario: Replay gap restores operator decisions

**Given**: A client loses retained event history
**When**: It replaces local authoritative data with `GET /api/v2/state`
**Then**: Every server-authoritative operator decision field is restored from the snapshot
**And**: The client does not infer missing state from logs, display strings, paths, or prior events

### Requirement: Serialized optimistic revision control

One process-local projection owner MUST serialize command admission, snapshot mutation, `state_revision`, `event_sequence`, event storage, and publication. For each state-affecting input it MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; a new stale command MUST fail without service execution. Snapshot mutations MUST publish all related decision fields coherently at the same resulting revision.

#### Scenario: Mark mutation reads back coherently

**Given**: An accepted command changes an execution mark without changing queue intent
**When**: The resulting state revision is read
**Then**: The snapshot reports the new execution mark and unchanged queue intent together
**And**: No client-side inference is required

## ADDED Requirements

### Requirement: Execution intent remains ephemeral

Execution marks, queue presentation intent, and UI attention state exposed by `/api/v2` MUST remain process-local and non-durable. They MUST NOT become authoritative workflow evidence and MUST reset or be recomputed on process restart according to workspace and Git state.

#### Scenario: Restart clears ephemeral operator state

**Given**: A process exposes marked changes and attention state
**When**: The process restarts with unchanged workspace and Git evidence
**Then**: Ephemeral operator state is cleared or recomputed
**And**: Workflow routing remains derived from the workspace rather than the prior API snapshot
