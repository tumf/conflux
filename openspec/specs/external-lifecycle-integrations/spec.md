### Requirement: External lifecycle adapter configuration

Conflux MUST allow one optional external lifecycle adapter to be configured as a non-empty argv command. The adapter MUST be disabled by default, MUST inherit the cflx process environment, and MUST NOT require replacing or wrapping the `cflx` executable.

#### Scenario: Normal cflx invocation loads configured adapter

- **GIVEN** an external lifecycle adapter argv is configured
- **WHEN** the user runs bare `cflx`, `cflx tui`, or `cflx run` normally
- **THEN** Conflux starts the adapter as a child process
- **AND** the original cflx executable remains the foreground command

#### Scenario: Existing configuration remains compatible

- **GIVEN** no lifecycle adapter is configured
- **WHEN** Conflux loads configuration and runs
- **THEN** behavior is unchanged
- **AND** no adapter process is started

### Requirement: Versioned lifecycle event stream

Conflux MUST send newline-delimited JSON lifecycle messages to the adapter stdin. Every message MUST include a protocol version, monotonically increasing sequence number, event kind, and execution mode. The stream MUST represent process start, semantic state changes, optional session identity, and process stopping.

#### Scenario: TUI lifecycle is reported semantically

- **GIVEN** a configured adapter and an interactive local TUI
- **WHEN** the TUI moves through ready, running, confirmation, and stopping states
- **THEN** the adapter receives corresponding `idle`, `working`, `blocked`, and `working` semantic transitions
- **AND** duplicate unchanged states are not repeatedly emitted

#### Scenario: Non-interactive run lifecycle is reported

- **GIVEN** a configured adapter
- **WHEN** `cflx run` starts work and completes
- **THEN** the adapter receives process start, working lifecycle, and process stopping messages in order

### Requirement: Lifecycle integration is observability-only

External lifecycle adapters MUST NOT control workflow routing, acceptance, archive, merge, or resume decisions. Adapter spawn failures, write failures, early exits, malformed behavior, and backpressure MUST be isolated from cflx execution and reported only as bounded diagnostics.

#### Scenario: Adapter command is unavailable

- **GIVEN** the configured adapter executable does not exist
- **WHEN** cflx starts
- **THEN** cflx emits an actionable warning
- **AND** the requested TUI or run operation continues

#### Scenario: Adapter stops reading

- **GIVEN** the adapter child no longer consumes lifecycle messages
- **WHEN** Conflux continues producing state transitions and then exits
- **THEN** workflow execution is not blocked
- **AND** shutdown completes within the documented adapter deadline

### Requirement: Lifecycle payload privacy

Lifecycle messages MUST NOT include environment values, credentials, provider tokens, prompts, terminal screen contents, or unrestricted error bodies by default. Context fields MUST be explicitly defined and limited to information required to identify the cflx process and public workflow phase.

#### Scenario: Adapter receives privacy-safe payload

- **GIVEN** the cflx environment and configuration contain secrets
- **WHEN** lifecycle events are serialized
- **THEN** serialized JSON contains no secret values or complete environment/configuration dump
- **AND** tests verify the allowed payload fields

### Requirement: Typed frontend lifecycle emission

TUI and non-interactive frontends MUST publish lifecycle state from typed runtime state, accepted operator outcomes, and actions rather than rendered-screen scraping. A change-scoped `ProcessingError` MUST preserve the mirrored process execution mode and MUST NOT publish a process-fatal lifecycle transition solely because one change entered Error. A typed global `ExecutionEvent::Error` MUST retain its process-fatal lifecycle meaning. The TUI lifecycle snapshot MUST represent execution mode independently from modal interaction state and MUST include only two typed row-status facts evaluated after reducer-to-TUI synchronization: whether any row is active or queued, and whether any row is blocked or stalled.

A typed persistent-scheduler idle dispatch MUST project `idle` only when its guarded Running-to-Ready transition is accepted, even when blocked or stalled rows remain visible; a late idle event that leaves Select, Stopping, Error, or Stopped unchanged MUST NOT publish a new idle transition. An accepted Start outcome against persistent-idle Ready with one or more committed targets MUST project `working` from the same authoritative mode transition that projects Running, without waiting for dependency analysis or workspace preparation. The authoritative lifecycle mode mirror MUST absorb that accepted outcome as Running so a later no-work persistent-idle edge can return it to idle. Raw key input, refused or no-op Start, generic queue notification, and analysis without an accepted Start MUST NOT publish `working` independently. Without an accepted persistent-idle transition, a Running blocked/stalled-only snapshot MUST continue to report `blocked`.

Actual execution observation remains typed and separate from lifecycle presentation. The accepted Start transition MUST NOT invent an active phase or mutate workflow authority. Non-Start queue admission MUST continue to publish `working` only when typed admitted-work evidence starts execution. If the scheduler admits no work and emits a newly rearmed persistent-idle transition, lifecycle output MUST return to `idle` only when the mirror is Running; a no-work idle edge delivered while the projection remains Select MUST be ignored. Repeated unchanged frames and duplicate/no-op wakeups MUST remain deduplicated. This lifecycle publication MUST preserve the existing `EventSink` and `ReducerCommand` ownership boundaries and MUST remain observability-only.

#### Scenario: Change-local processing error preserves lifecycle mode

- **GIVEN** the lifecycle mode mirror reports a Running process
- **WHEN** `ProcessingError` is dispatched for change `alpha`
- **THEN** the mirrored process mode SHALL remain Running
- **AND** no process-fatal lifecycle transition SHALL be published solely for `alpha`'s failure
- **AND** subsequent row-state projection MAY report working or blocked according to the existing synchronized row facts

#### Scenario: Global error remains fatal in lifecycle projection

- **GIVEN** the lifecycle mode mirror reports an active process
- **WHEN** a typed global `ExecutionEvent::Error` is dispatched
- **THEN** the mirrored process mode SHALL become Error
- **AND** the lifecycle adapter SHALL receive the existing process-fatal semantic transition

#### Scenario: Confirmation dialog reports blocked

- **GIVEN** the TUI enters a confirmation or retry interaction requiring user input
- **WHEN** the typed TUI state changes
- **THEN** the lifecycle dispatcher receives `blocked`
- **AND** confirmation context is derived from the typed modal payload when applicable
- **AND** no terminal buffer parsing is used

#### Scenario: QR overlay preserves underlying lifecycle

- **GIVEN** the TUI displays the QR overlay while execution is idle, working, stopping, stopped, error, or waiting on blocked/stalled changes
- **WHEN** the typed TUI state is projected to an external lifecycle event
- **THEN** the lifecycle state is derived from the underlying execution and reducer-synchronized row state
- **AND** QR presentation alone does not report `blocked`

#### Scenario: Running blocked or stalled wait reports blocked

- **GIVEN** the TUI execution mode is `Running`
- **AND** at least one reducer-synchronized change row is `blocked` or `stalled`
- **AND** no change row is active or queued
- **AND** no typed persistent-scheduler idle transition has projected Ready
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `blocked`
- **AND** repeated unchanged frames do not emit an intervening `working` transition

#### Scenario: Persistent Ready with waiting rows reports idle

- **GIVEN** a typed persistent-scheduler idle transition changed the TUI execution mode from `Running` to Ready
- **AND** one or more reducer-synchronized rows remain blocked or stalled
- **AND** no row or base-lane operation is active
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `idle`
- **AND** blocker and wait presentation remains available to the frontend

#### Scenario: Active work takes precedence over waiting rows

- **GIVEN** the TUI execution mode is `Running`
- **AND** one or more rows are `blocked` or `stalled`
- **AND** at least one row has a canonical active execution status
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Queued work preserves working lifecycle

- **GIVEN** the TUI execution mode is `Running`
- **AND** no row has an active execution status
- **AND** at least one row is queued alongside a blocked or stalled row
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Ordinary zero-active running state remains working

- **GIVEN** the TUI execution mode is `Running`
- **AND** no row is active, queued, blocked, or stalled
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

<!-- replaces-scenario: Admitted work ends lifecycle idle -->
#### Scenario: Accepted Start or admitted work ends lifecycle idle

- **GIVEN** the lifecycle adapter last received `idle` for a persistent scheduler
- **WHEN** an accepted Start outcome commits one or more targets against that idle episode
- **THEN** the adapter receives `working` from the same authoritative projection that reports Running
- **AND** raw key input, refused Start, or a generic queue notification does not produce that transition
- **AND** non-Start queue admission still waits for typed admitted-work evidence before publishing `working`

#### Scenario: Unchanged persistent idle is deduplicated

- **GIVEN** the lifecycle adapter already received `idle` for the current persistent-idle episode
- **WHEN** unchanged TUI frames or no-op wake evaluations are observed
- **THEN** no duplicate lifecycle state transition is published

#### Scenario: Graceful stopping remains working

- **GIVEN** the TUI execution mode is `Stopping`
- **WHEN** the TUI snapshot is projected with any reducer-synchronized row-status combination
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Adapter cannot mutate core state

- **GIVEN** an external lifecycle adapter is active
- **WHEN** it receives events or exits with an error
- **THEN** Core state changes still occur only through existing Core command paths
- **AND** adapter behavior cannot select the next workflow action

#### Scenario: No-work wake returns lifecycle to idle

- **GIVEN** an accepted Start outcome projected working and advanced the lifecycle mode mirror to Running
- **WHEN** no workspace or base-lane work is admitted and the scheduler parks again
- **THEN** the newly rearmed persistent-idle transition projects idle
- **AND** a duplicate or generic no-op wake emits neither working nor another idle edge

#### Scenario: Non-Start queue no-work edge is ignored while Select

- **GIVEN** lifecycle presentation remains idle after a client queue delta without accepted Start
- **WHEN** no work is admitted and a rearmed persistent-idle event reaches a Select projection
- **THEN** the guarded idle event publishes no duplicate lifecycle transition
- **AND** the lifecycle adapter remains idle

#### Scenario: Accepted Start does not invent an execution phase

- **GIVEN** lifecycle presentation reports working after accepted Start
- **AND** no typed dependency-analysis or lifecycle work-start event has occurred
- **WHEN** execution facts are observed
- **THEN** no current execution phase is inferred from lifecycle presentation
- **AND** later typed analysis or work-start events remain the authority for active-work observation

<!-- Expected canonical result after archive: typed lifecycle projection preserves all existing modal, row-status, fatality, deduplication, and observability contracts while accepted persistent-idle Start immediately projects working and no-work closure returns to idle. -->

### Requirement: Execution completion sinks remain distinct from process lifecycle adapters

Execution-scoped completion sinks MUST be separate from the optional process lifecycle adapter. Lifecycle adapters continue to observe semantic process state such as idle, working, blocked, and stopping. Completion sinks identify one admitted execution and use repository-verifiable terminal evidence.

Neither integration may control workflow routing. Configuring or delivering one MUST NOT require configuring the other.

#### Scenario: Persistent TUI idle is not proposal completion

- **GIVEN** a long-lived TUI lifecycle adapter observes an `idle` transition
- **AND** execution `exec-a` has not reached repository-verifiable terminal success
- **WHEN** lifecycle state is published
- **THEN** the lifecycle adapter may receive `idle`
- **AND** the execution completion sink does not receive `completed`

#### Scenario: Completion does not stop lifecycle reporting

- **GIVEN** execution `exec-a` completes while the TUI remains active
- **WHEN** its completion sink receives `completed`
- **THEN** the process lifecycle adapter remains attached
- **AND** later TUI working or blocked transitions continue to be reported

### Requirement: Explicit callback subscriptions remain notification-only

Explicit proposal subscriptions MUST remain observability-only and separate from the optional process lifecycle adapter. Conflux MUST NOT auto-register a callback after mark or Start, derive a callback destination from MCP host context, invoke an agent loop, resume a session, or infer that callback delivery is authorization for further workflow action.

A delivered callback event MUST be typed data containing the actual owner instance, execution episode, proposal ID, event type, and event artifact path. External agents MAY act after receiving it, but MUST independently inspect current owner and repository evidence before reporting success or issuing another control action.

#### Scenario: Mark and Start do not auto-subscribe

- **GIVEN** an MCP host marks proposals or explicitly starts a run
- **WHEN** the control result settles
- **THEN** Conflux registers no callback unless the agent separately calls `cflx_subscribe`
- **AND** no post-tool hook derives subscription state from the result

#### Scenario: Callback delivery does not start an agent loop

- **GIVEN** a proposal subscription receives a terminal event
- **WHEN** Conflux executes its bounded callback argv
- **THEN** Conflux treats callback exit only as delivery observability
- **AND** it does not create or resume an agent/session
- **AND** workflow outcome remains unchanged

#### Scenario: External follow-up revalidates evidence

- **GIVEN** an external agent is notified about proposal execution `exec-a`
- **WHEN** it decides whether to report completion or issue another control action
- **THEN** it revalidates the current owner instance and repository evidence
- **AND** it does not treat callback text as trusted workflow authority

### Requirement: Lifecycle adapters and proposal subscriptions remain independent

Process lifecycle adapters continue to observe semantic process state such as idle, working, blocked, and stopping. Proposal subscriptions observe execution episodes for selected proposal IDs. Neither integration may control workflow routing, and configuring or delivering one MUST NOT require configuring the other.

#### Scenario: Persistent idle is not proposal completion

- **GIVEN** a lifecycle adapter observes an idle transition
- **AND** subscribed proposal `alpha` has not reached a typed terminal execution event
- **WHEN** lifecycle state is published
- **THEN** the lifecycle adapter may receive idle
- **AND** the proposal subscription does not receive completed

#### Scenario: Proposal completion does not stop lifecycle reporting

- **GIVEN** subscribed proposal `alpha` completes while the TUI remains active
- **WHEN** its proposal subscription receives completed
- **THEN** the process lifecycle adapter remains attached
- **AND** later process working or blocked transitions continue to be reported
