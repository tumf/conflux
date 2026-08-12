## ADDED Requirements

### Requirement: Owner-scoped execution identity

The owner MUST assign one random process-local `execution_id` whenever any admission source moves a change from non-admitted into queued or active work. Admission sources include TUI, scheduler, direct `/api/v2`, CLI client, and MCP. The ID MUST bind to the current `instance_id` and `change_id`, MUST be visible in execution status, and MUST be returned by an `already_admitted` client outcome for the current episode.

Typed terminal settlement or dequeue ends the episode. Dequeue followed by admission or a later retry MUST receive a new execution ID; iterations within one admitted run MUST retain the same ID. Concurrent enqueue requests that observe one already-admitted episode MUST return the same current ID.

Execution identity is observability-only. It MUST NOT authorize commands or affect scheduler eligibility, workflow routing, acceptance, archive, merge, retry, or completion classification. Owner restart MUST invalidate every prior execution binding rather than silently rebinding it.

#### Scenario: Every admission source creates an identity

- **GIVEN** change `alpha` is not admitted
- **WHEN** the TUI, scheduler, direct API, CLI client, or MCP successfully admits it
- **THEN** the owner creates one execution ID for that episode
- **AND** later `already_admitted` clients observe that same ID

#### Scenario: Retry receives a new execution identity

- **GIVEN** execution `exec-a` for change `alpha` reached terminal settlement or was dequeued
- **WHEN** the owner later admits `alpha` again
- **THEN** it returns execution `exec-b`
- **AND** `exec-b` differs from `exec-a`
- **AND** a sink registered for `exec-a` cannot observe or control `exec-b`

#### Scenario: Restart invalidates process-local identity

- **GIVEN** an execution binding belongs to owner instance `owner-a`
- **WHEN** the owner restarts as `owner-b`
- **THEN** later operations using the old binding fail with typed `owner_restarted`
- **AND** no push delivery from the lost process is promised
- **AND** workspace-derived workflow routing is unchanged

### Requirement: Execution-sink remote resources

The owner MUST expose authenticated `GET`, `PUT`, and `DELETE /api/v2/executions/{execution_id}/sink` resources outside the closed workflow command registry. Every request MUST validate the exact `(instance_id, execution_id, change_id)` binding. Sink operations MUST create no workflow command record, require no `expected_revision` or idempotency key, and MUST NOT advance `state_revision` or mutate orchestration state.

Capabilities MUST advertise execution-sink support, execution status MUST expose the current execution ID, and the generated OpenAPI contract MUST describe these resources and schemas. Clients that reach an older owner without this capability MUST fail with a typed unsupported-owner outcome. Sink mutation MUST be accepted only over the owner Unix socket; TCP mutation MUST fail with a typed refusal even when bearer authentication succeeds.

#### Scenario: Sink registration is not a workflow command

- **GIVEN** an admitted execution and a coherent owner revision
- **WHEN** a local Unix-socket client registers a sink
- **THEN** the sink is attached without creating a command record
- **AND** `state_revision`, queue intent, scheduler state, execution marks, and workflow routing are unchanged

#### Scenario: TCP cannot register executable argv

- **GIVEN** an authenticated TCP client can read owner resources
- **WHEN** it attempts to set or clear an execution sink
- **THEN** the owner returns a typed transport refusal
- **AND** no argv is stored or executed

### Requirement: Execution-scoped completion sinks

The owner MUST allow one bounded command sink to be attached, inspected, and cleared for an exact execution binding. The sink MUST be argv data executed directly without shell interpretation. Repeating an identical set MUST be idempotent; setting different valid argv MUST atomically replace the prior sink. Registration state and delivery dedupe MUST remain process-local and observability-only.

The owner MUST classify `completed` with the same execution contract and repository completion oracle used by `cflx client wait`. Repository verification MUST run outside the reducer/orchestration critical path with bounded subprocess deadlines and bounded retries for inconclusive evidence. Change disappearance, TUI process liveness, process-wide idle presentation, or callback success MUST NOT count as workflow completion.

Every sink MUST receive the first typed terminal classification among `completed`, `failed`, and `stopped`; callers cannot disable terminal types. `stopped` MUST derive from settled stop/dequeue removal, including removal before active work. `failed` MUST derive from typed terminal unsuccessful owner state, never an unrestricted error body or disappearance. Optional `blocked` attention MUST be edge-triggered: an unchanged blocked state does not redeliver, while leaving and re-entering blocked creates a new attention edge. Registering after typed terminal settlement MUST immediately attempt that terminal delivery once.

A graceful owner shutdown MAY attempt `owner_stopping` for live registrations. A crash or forced termination cannot provide a final callback; later observation by an external adapter MUST report typed `owner_restarted` without treating it as completion.

#### Scenario: Completion notifies while TUI stays alive

- **GIVEN** a sink is registered for admitted execution `exec-a` of `alpha`
- **AND** the TUI remains running after work completes
- **WHEN** repository evidence satisfies the owner's terminal execution contract for `alpha`
- **THEN** the owner dispatches one `completed` event for `exec-a`
- **AND** TUI process exit is neither required nor inferred

#### Scenario: Registration after terminal state delivers immediately

- **GIVEN** execution `exec-a` already reached a typed terminal classification
- **WHEN** a local client registers its first sink
- **THEN** the owner immediately attempts exactly one delivery of that terminal event
- **AND** no race between enqueue settlement and registration can lose the terminal notification

#### Scenario: Disappearance is not success

- **GIVEN** the observed change disappears from one owner snapshot
- **WHEN** repository evidence does not prove the declared terminal mode
- **THEN** no `completed` event is dispatched
- **AND** observation continues or settles with a typed unsuccessful outcome

#### Scenario: Blocked attention can repeat only after recovery

- **GIVEN** execution `exec-a` enters a typed blocked state and blocked delivery is enabled
- **WHEN** the state remains unchanged
- **THEN** one `blocked` event is dispatched for that attention edge
- **AND** it is not redelivered while unchanged
- **WHEN** the execution leaves blocked and later re-enters it
- **THEN** one new `blocked` event MAY be dispatched

### Requirement: Completion-sink delivery is bounded and non-authoritative

For each delivery the owner MUST create a versioned bounded event file and provide only fixed metadata through `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Payloads MUST exclude prompts, terminal screen contents, environment dumps, credential values, and unrestricted error bodies. The event file MUST remain immutable while the callback runs, be removed after the callback is reaped, and be removed on owner shutdown if still present.

Callback runtime and captured output MUST be bounded. Spawn failure, timeout, non-zero exit, malformed callback behavior, and output overflow MUST produce bounded diagnostics only. One terminal delivery attempt is permitted per execution; failures MUST NOT retry forever, alter orchestration state, roll back completion, or change the repository-verifiable result.

#### Scenario: Callback failure cannot change completion

- **GIVEN** repository evidence proves `alpha` completed
- **AND** its registered callback exits non-zero
- **WHEN** delivery settles
- **THEN** `alpha` remains completed
- **AND** the owner records bounded delivery diagnostics
- **AND** no workflow command, retry, archive, merge, or rollback is synthesized

#### Scenario: Secrets remain outside callback artifacts

- **GIVEN** owner configuration and environment contain credentials
- **WHEN** a completion event file and callback environment are produced
- **THEN** neither contains credential values or a complete configuration/environment dump
- **AND** token values are not accepted in notification argv or returned by MCP tools
