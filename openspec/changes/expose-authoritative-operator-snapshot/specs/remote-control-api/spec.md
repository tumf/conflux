## MODIFIED Requirements

### Requirement: Serialized optimistic revision control

One process-local projection owner MUST serialize command admission, snapshot mutation, `state_revision`, `event_sequence`, event storage, and publication. For each state-affecting input it MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; a new stale command MUST fail without service execution. Snapshot mutations MUST publish all related decision fields coherently at the same resulting revision.

#### Scenario: State event and snapshot share one revision

**Given**: A state-affecting execution event changes the current snapshot
**When**: The projection owner processes it
**Then**: It increments revision once
**And**: The stored snapshot and published event contain the same resulting revision

#### Scenario: No-op does not advance revision

**Given**: An input produces no snapshot change
**When**: The projection owner processes it
**Then**: `state_revision` is unchanged

#### Scenario: Stale new command is rejected

**Given**: The current state revision is 12
**When**: A new command supplies expected revision 11
**Then**: The server returns HTTP 409 with `stale_revision` and current revision 12
**And**: No command side effect occurs

#### Scenario: Mark mutation reads back coherently

**Given**: An accepted command changes an execution mark without changing queue intent
**When**: The resulting state revision is read
**Then**: The snapshot reports the new execution mark and unchanged queue intent together
**And**: No client-side inference is required

## ADDED Requirements

### Requirement: Authoritative operator snapshot

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs.

#### Scenario: Client discovers and snapshots operator state

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot
**And**: The snapshot includes distinct execution mark and queue intent, attention state, blocker and error details, action and parallel eligibility, timing, latest activity, and change-to-worktree relation when applicable

#### Scenario: Replay gap restores operator decisions

**Given**: A client loses retained event history
**When**: It replaces local authoritative data with `GET /api/v2/state`
**Then**: Every server-authoritative operator decision field is restored from the snapshot
**And**: The client does not infer missing state from logs, display strings, paths, or prior events

#### Scenario: Restart preserves workspace-derived authority

**Given**: A process exposes marked changes and attention state
**When**: The process restarts with unchanged workspace and Git evidence
**Then**: Ephemeral operator state is cleared or recomputed
**And**: Workflow routing remains derived from the workspace rather than the prior API snapshot
