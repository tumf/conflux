## ADDED Requirements

### Requirement: Coherent immutable owner incarnation birth

A local orchestration owner's diagnostic metadata `started_at` and
`GET /api/v2/instance.started_at` MUST be byte-for-byte copies of one nonempty
RFC 3339 value generated once in memory for that OS process. Both surfaces MUST
report the same process PID. The birth MUST be final before the first API
response, including responses before TUI state/repository binding, and MUST
remain unchanged across endpoint publication, state refresh, and runtime binding.
All listeners of that owner MUST expose the same incarnation.

A replacement process MUST sample its own birth and create its existing fresh
random `instance_id`; it MUST NOT inherit identity from old owner metadata.
Fresh sampling does not promise wall-clock monotonicity or uniqueness; the
random instance ID's existing replay/correlation role is unchanged. This
contract MUST NOT introduce tolerant, rounded, normalized, or PID-only identity
matching. Exact PID/birth consumers must be able to reject disagreement in
either component independently.

The OS advisory lock MUST remain the ownership authority; diagnostic metadata
MUST remain best-effort observability, not workflow state. Missing, stale,
malformed, or unwritable metadata MUST NOT become an identity source or change
lock/startup policy. Existing endpoint publication/rollback, JSON field shapes,
authentication, and permission contracts MUST remain unchanged. An old live
owner's record MUST NOT be rewritten to retrofit this equality; rollout requires
a fresh owner running a binary containing the fix, not a data migration.

#### Scenario: TUI identity is coherent before late binding

- **GIVEN** a fresh default or explicit TUI owner acquired its repository lock
- **AND** its API is reachable before TUI `AppState` and repo-root binding
- **WHEN** a client reads its owner metadata and `/api/v2/instance`
- **THEN** PID and the exact nonempty birth string agree
- **AND** later binding and repeated reads retain that birth and instance ID

#### Scenario: Endpoint publication does not mint another birth

- **GIVEN** an owner has published metadata without endpoints
- **WHEN** all requested UDS/TCP listeners bind and endpoints are published
- **THEN** the metadata PID and birth are unchanged
- **AND** both listeners report the same instance ID, PID, and exact birth
- **AND** a partial bind failure retains the existing rollback and advertises no successful transaction

#### Scenario: Replacement ignores stale metadata

- **GIVEN** a prior owner exited leaving diagnostic metadata behind
- **WHEN** an authorized replacement acquires the OS lock and serves the API
- **THEN** it generates a new instance ID and samples its own birth in memory
- **AND** its new metadata and API agree without consulting the stale birth
- **AND** no running old owner's record is patched to manufacture agreement

#### Scenario: Exact consumer identity comparison stays strict

- **GIVEN** serialized owner metadata and an API instance response with equal PID and birth
- **WHEN** a consumer applies exact PID and birth equality
- **THEN** that pair is eligible for identity confirmation
- **AND** changing only PID is rejected even if birth matches
- **AND** changing only birth by one nanosecond is rejected even if PID matches
- **AND** matching identity does not bypass the consumer's other custody, endpoint, or authentication checks

#### Scenario: Diagnostic failures do not become identity authority

- **GIVEN** an old owner record is missing, malformed, or cannot be written
- **WHEN** an invocation follows the existing ownership/startup path
- **THEN** its in-memory birth is generated independently of that record
- **AND** OS lock decisions and best-effort diagnostics retain their existing behavior
