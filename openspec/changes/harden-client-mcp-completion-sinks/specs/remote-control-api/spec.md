## MODIFIED Requirements

### Requirement: Execution-sink remote resources

The owner MUST expose authenticated `GET`, `PUT`, and `DELETE /api/v2/executions/{execution_id}/sink` resources outside the closed workflow command registry. Every request, including inspection, MUST require and validate the exact `(instance_id, execution_id, change_id)` binding. These identifiers are readable through authenticated resources, so the binding is a coherence check rather than access control. Registered argv MUST be returned only to a request that arrived on the owner Unix socket. Sink operations MUST create no workflow command record, require no `expected_revision` or idempotency key, and MUST NOT advance `state_revision` or mutate orchestration state.

Capabilities MUST advertise execution-sink support, execution status MUST expose the current execution ID, and the generated OpenAPI contract MUST describe these resources and schemas. Clients that reach an older owner without this capability MUST fail with a typed unsupported-owner outcome. Sink mutation MUST be accepted only over the owner Unix socket; TCP mutation MUST fail with a typed refusal even when bearer authentication succeeds.

#### Scenario: Sink registration is not a workflow command

- **GIVEN** an admitted execution and a coherent owner revision
- **WHEN** a local Unix-socket client registers a sink with its exact binding
- **THEN** the sink is attached without creating a command record
- **AND** `state_revision`, queue intent, scheduler state, execution marks, and workflow routing are unchanged

#### Scenario: TCP cannot register executable argv

- **GIVEN** an authenticated TCP client can read owner resources
- **WHEN** it attempts to set or clear an execution sink
- **THEN** the owner returns a typed transport refusal
- **AND** no argv is stored or executed

#### Scenario: Sink inspection requires exact binding

- **GIVEN** a client knows an execution ID but omits or mismatches its instance or change identity
- **WHEN** it requests the sink resource
- **THEN** the owner rejects the request with a typed binding error
- **AND** callback argv is not disclosed

#### Scenario: TCP inspection does not disclose argv

- **GIVEN** an authenticated TCP client presents the correct execution binding
- **WHEN** it reads the sink resource
- **THEN** subscription presence, execution state, and delivery history are returned
- **AND** the registered argv is not returned

### Requirement: Completion-sink delivery is bounded and non-authoritative

For each delivery the owner MUST create a versioned bounded event file and provide only fixed metadata through `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Payloads MUST exclude prompts, terminal screen contents, environment dumps, credential values, and unrestricted error bodies. The event file MUST be created inside an owner-private directory with owner-read-only permissions (`0400` inside a `0700` directory), so an ordinary callback cannot open it for writing. The owner MUST NOT re-read or trust the event file after writing it, MUST remove it only after its callback is reaped, and MUST remove it on owner shutdown after every callback has been reaped. A callback runs under the owner's UID and can defeat file permissions; this is default mutation refusal, not an integrity guarantee against a hostile callback, and no owner decision may depend on the file contents. An event artifact MUST NOT be overwritten or removed while a different callback still holds it.

Callback runtime and stdout/stderr capture MUST be bounded during collection, not merely truncated after collection, and the owner MUST continue draining both streams past the retention limit so a callback is never blocked by a full pipe. Spawn failure, timeout, non-zero exit, malformed callback behavior, and output overflow MUST produce bounded diagnostics only. Output overflow alone MUST NOT terminate a callback. Timeout and shutdown cancellation MUST terminate and explicitly reap the callback. One terminal delivery attempt is permitted per execution; failures MUST NOT retry forever, alter orchestration state, roll back completion, or change the repository-verifiable result.

Graceful owner shutdown MUST stop admission and apply one finite shutdown deadline across all queued or running callbacks. Delivery MUST remain serialized. Shutdown MUST start no new delivery and create or recreate no event directory or artifact after it begins. Before event artifact cleanup and registry destruction, every callback MUST either finish and be reaped or be terminated and reaped.

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

#### Scenario: Callback output overflow remains bounded

- **GIVEN** a callback writes far more stdout and stderr than the capture limit and then exits successfully
- **WHEN** the owner collects callback output
- **THEN** owner memory retained for output remains within the configured bound plus fixed buffering overhead
- **AND** both streams continue to drain, the callback is not blocked by a full pipe, and it is reaped with its own exit status
- **AND** bounded diagnostics record that output was truncated
- **AND** workflow completion is unchanged

#### Scenario: Callback cannot open its event payload for writing by default

- **GIVEN** a callback is running with `CFLX_EVENT_PATH` under an unprivileged owner UID
- **WHEN** it opens the event file for writing or truncation without first defeating owner permissions
- **THEN** the open is refused by the file permissions
- **AND** the original payload remains readable until the callback is reaped
- **AND** no owner decision reads the event file back, so mutation cannot change a delivered classification

#### Scenario: Multi-callback shutdown reaps before cleanup

- **GIVEN** more than two callbacks are queued or running when graceful shutdown starts
- **WHEN** the global shutdown deadline is reached
- **THEN** unfinished callbacks are terminated and reaped
- **AND** no event artifact is removed while its callback remains alive
- **AND** no queued delivery starts after the deadline
- **AND** no event artifact is created after shutdown begins
