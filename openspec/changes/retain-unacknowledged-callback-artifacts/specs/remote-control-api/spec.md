## MODIFIED Requirements

### Requirement: Completion-sink delivery is bounded and non-authoritative

For each delivery the owner MUST create a versioned bounded event file and provide only fixed metadata through `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Payloads MUST exclude prompts, terminal screen contents, environment dumps, credential values, and unrestricted error bodies. The event file MUST be created inside an owner-private directory with owner-read-only permissions (`0400` inside a `0700` directory), so an ordinary callback cannot open it for writing. The owner MUST NOT re-read or trust the event file after writing it, MUST remove it only after its callback is reaped, and MUST remove it on owner shutdown only after every callback is positively acknowledged as reaped. A callback runs under the owner's UID and can defeat file permissions; this is default mutation refusal, not an integrity guarantee against a hostile callback, and no owner decision may depend on the file contents. An event artifact MUST NOT be overwritten or removed while a different callback still holds it.

Callback runtime and stdout/stderr capture MUST be bounded during collection, not merely truncated after collection, and the owner MUST continue draining both streams past the retention limit so a callback is never blocked by a full pipe. Spawn failure, timeout, non-zero exit, malformed callback behavior, and output overflow MUST produce bounded diagnostics only. Output overflow alone MUST NOT terminate a callback. Timeout and shutdown cancellation MUST terminate and explicitly reap the callback. One terminal delivery attempt is permitted per execution; failures MUST NOT retry forever, alter orchestration state, roll back completion, or change the repository-verifiable result.

Graceful owner shutdown MUST stop admission and apply one finite shutdown deadline across all queued or running callbacks. Delivery MUST remain serialized. Shutdown MUST start no new delivery and create or recreate no event directory or artifact after it begins. When the deadline expires, the owner MUST cancel unfinished delivery and MUST wait for dispatcher acknowledgement that every active callback has been terminated and reaped before event artifact cleanup and registry destruction. A secondary timeout, task-send failure, acknowledgement sender drop, or registry destruction MUST NOT authorize or implicitly perform cleanup while a callback may remain alive. Missing acknowledgement MUST retain the owner-private directory and artifacts.

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

#### Scenario: Delayed reap acknowledgement blocks artifact cleanup

- **GIVEN** shutdown cancellation has been issued but dispatcher reap acknowledgement is delayed
- **WHEN** any secondary acknowledgement wait would otherwise expire
- **THEN** the owner retains the event directory and active callback artifact
- **AND** cleanup occurs only after dispatcher acknowledgement confirms the callback was reaped

#### Scenario: Dropped acknowledgement retains artifacts

- **GIVEN** a callback artifact exists and dispatcher acknowledgement is dropped without confirming child reap
- **WHEN** graceful shutdown and registry destruction complete their available paths
- **THEN** the owner-private directory and artifact remain
- **AND** no implicit destructor cleanup removes them
