## MODIFIED Requirements

### Requirement: Local client compatibility discovery

The versioned single-instance API MUST expose enough generated capability, instance, authoritative-state, execution-status, command-record, and event information for the source-matched `cflx client` client to inspect and operate a command-capable owner without parsing logs or display strings. The client MUST check API compatibility and process incarnation before mutation and during command settlement. A typed command-capability field MUST identify whether the executor is bound. An unbound command executor MUST return a distinct `command_executor_unbound` wire error rather than an ordinary lifecycle conflict or a command queued for later execution.

The local client MUST validate an environment-provided bearer token as an HTTP header value before opening a connection or constructing a request. It MUST reject CR, LF, all other HTTP-forbidden control characters, and DEL with a typed error that identifies only the environment-variable source, not the token value. Valid header values MUST be transmitted unchanged.

This compatibility surface MUST include a minimal typed owner execution contract at an `instance_id` and `state_revision`: base branch identity and terminal mode (`merged`, `base_published`, or `branch_pushed`), plus selected remote and pushed branch when applicable. The generated OpenAPI document MUST publish these fields. They are observational facts only and MUST NOT become durable workflow authority; repository and workspace evidence remain authoritative for routing and truthful completion.

#### Scenario: Source-matched client discovers required behavior

**Given**: a command-capable owner serves `/api/v2`
**When**: the same build's client reads capabilities, instance, state, and execution status
**Then**: it can determine supported observation and command behavior from typed fields
**And**: it does not need to parse logs, prose details, or TUI presentation strings

#### Scenario: Owner contract identifies truthful terminal proof

**Given**: an owner integrates changes locally or publishes them to a selected remote
**When**: the client reads the owner execution contract at the matching state revision
**Then**: it receives the base branch identity and terminal success mode
**And**: `base_published` identifies the selected remote
**And**: `branch_pushed` identifies the selected remote and pushed branch without claiming base integration
**And**: the generated OpenAPI contract describes these typed fields

#### Scenario: Multi-resource observation rejects mixed revisions

**Given**: owner state advances while a client reads state, execution status, and owner execution contract
**When**: the returned incarnation or revision boundaries do not agree
**Then**: the client cannot treat the values as one coherent observation
**And**: bounded reread or a typed observation conflict is required before mutation or completion reporting

#### Scenario: Incompatible API fails before mutation

**Given**: the discovered owner lacks a required route, command, field, or compatible API version
**When**: the client prepares an enqueue operation
**Then**: it returns an incompatible-owner outcome
**And**: no mutation command is submitted

#### Scenario: Unbound executor remains fail closed

**Given**: a process serves read resources but has no bound command executor
**When**: the client submits or prepares a mutation
**Then**: typed command capability reports that mutation is unavailable
**And**: command execution is refused with `command_executor_unbound`
**And**: the request is not queued for a future executor

#### Scenario: Malformed bearer token is rejected before transport

**Given**: the named authentication environment variable contains CR, LF, another forbidden HTTP control character, or DEL
**When**: the client prepares any owner request
**Then**: it returns a typed redacted client error before connecting or writing request bytes
**And**: neither stdout nor stderr contains the token value

#### Scenario: Valid bearer token remains supported

**Given**: the named authentication environment variable contains a valid HTTP header value
**When**: the client connects to an authenticated owner
**Then**: the Authorization header carries that value unchanged
**And**: normal authentication behavior is preserved
