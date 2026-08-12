## ADDED Requirements

### Requirement: Owner-scoped execution identity

A successful high-level client admission MUST identify one process-local execution episode with a random `execution_id` bound to the current `instance_id` and `change_id`. A retry or later admission of the same change MUST receive a different execution ID. The binding MUST be created only after admission succeeds and MUST be discarded when the owner process exits.

Execution identity is observability-only. It MUST NOT authorize commands or affect scheduler eligibility, workflow routing, acceptance, archive, merge, retry, or completion classification. Owner restart MUST invalidate every prior execution binding rather than silently rebinding it.

#### Scenario: Retry receives a new execution identity

- **GIVEN** execution `exec-a` for change `alpha` reached a terminal or retryable outcome
- **WHEN** the owner later admits `alpha` again
- **THEN** it returns execution `exec-b`
- **AND** `exec-b` differs from `exec-a`
- **AND** a sink registered for `exec-a` cannot observe or control `exec-b`

#### Scenario: Restart invalidates process-local identity

- **GIVEN** an execution binding belongs to owner instance `owner-a`
- **WHEN** the owner restarts as `owner-b`
- **THEN** operations using the old binding fail with a typed owner-replaced or stale-binding outcome
- **AND** workspace-derived workflow routing is unchanged

### Requirement: Execution-scoped completion sinks

The owner MUST allow one bounded command sink to be attached, inspected, and cleared for an exact `(instance_id, execution_id, change_id)` binding. The sink MUST be argv data executed directly without shell interpretation. Registration state and delivery dedupe MUST remain process-local and observability-only.

The owner MUST classify completion with the same execution contract and repository completion oracle used by `cflx client wait`. Change disappearance, TUI process liveness, process-wide idle presentation, or callback success MUST NOT count as workflow completion. Supported events MUST include terminal `completed`, `failed`, `stopped`, and `owner_replaced`, plus optional edge-triggered `blocked` attention. A blocked event MAY be followed by a later terminal event.

#### Scenario: Completion notifies while TUI stays alive

- **GIVEN** a sink is registered for admitted execution `exec-a` of `alpha`
- **AND** the TUI remains running after work completes
- **WHEN** repository evidence satisfies the owner's terminal execution contract for `alpha`
- **THEN** the owner dispatches one `completed` event for `exec-a`
- **AND** TUI process exit is neither required nor inferred

#### Scenario: Disappearance is not success

- **GIVEN** the observed change disappears from one owner snapshot
- **WHEN** repository evidence does not prove the declared terminal mode
- **THEN** no `completed` event is dispatched
- **AND** observation continues or settles with a typed unsuccessful outcome

#### Scenario: Blocked attention can precede completion

- **GIVEN** execution `exec-a` enters a typed blocked state
- **WHEN** blocked delivery is enabled
- **THEN** one edge-triggered `blocked` event is dispatched
- **WHEN** the same execution later satisfies repository-verifiable completion
- **THEN** one terminal `completed` event is dispatched

### Requirement: Completion-sink delivery is bounded and non-authoritative

For each delivery the owner MUST create a versioned bounded event file and provide only fixed metadata through `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Payloads MUST exclude prompts, terminal screen contents, environment dumps, credential values, and unrestricted error bodies.

Callback runtime and captured output MUST be bounded. Spawn failure, timeout, non-zero exit, malformed callback behavior, and output overflow MUST produce bounded diagnostics only. They MUST NOT block indefinitely, retry forever, alter orchestration state, roll back completion, or change the repository-verifiable result.

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
