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

TUI and non-interactive frontends MUST publish lifecycle state from typed runtime state and actions rather than rendered-screen scraping. A change-scoped `ProcessingError` MUST preserve the mirrored process execution mode and MUST NOT publish a process-fatal lifecycle transition solely because one change entered Error. A typed global `ExecutionEvent::Error` MUST retain its process-fatal lifecycle meaning. The TUI lifecycle snapshot MUST represent execution mode independently from modal interaction state and MUST include only two typed row-status facts evaluated after reducer-to-TUI synchronization: whether any row is active or queued, and whether any row is blocked or stalled. A typed persistent-scheduler idle dispatch MUST project `idle` only when its guarded Running-to-Ready transition is accepted, even when blocked or stalled rows remain visible; a late idle event that leaves Select, Stopping, Error, or Stopped unchanged MUST NOT publish a new idle transition. Without an accepted persistent-idle transition, a Running blocked/stalled-only snapshot MUST continue to report `blocked`. This lifecycle publication MUST preserve the existing `EventSink` and `ReducerCommand` ownership boundaries and MUST remain observability-only.

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

#### Scenario: Admitted work ends lifecycle idle

- **GIVEN** the lifecycle adapter last received `idle` for a persistent scheduler
- **WHEN** a typed admitted-work event starts workspace preparation or scheduler-owned base-lane work
- **THEN** the adapter receives `working`
- **AND** a Start notification, queue notification, or analysis attempt without admitted work does not produce that transition

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

<!-- Expected canonical result after archive: typed lifecycle projection will preserve process mode for change-local ProcessingError and reserve fatal lifecycle Error for the global Error event. -->

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

### Requirement: Reference OpenCode completion callback is loopback-confined and recoverably deduplicated

The reference OpenCode completion callback MUST validate its configured base URL as loopback HTTP, MUST resolve the callback path against that base, and MUST verify that the resolved URL retains the base's origin before sending. Any path that changes origin, including absolute, protocol-relative, or backslash variants, MUST be rejected. The callback MUST NOT follow redirects.

It MUST use an atomic local in-flight claim so concurrent invocations for the same execution event produce at most one POST during normal operation. A successful-delivery marker MUST be distinct from the in-flight claim. Failed POST MUST release the claim so a later external invocation may retry. A fresh in-flight claim MUST return a distinct non-success outcome. An existing successful-delivery marker MUST return success without posting. A claim older than five minutes MAY be atomically taken over so a crashed process cannot suppress delivery permanently.

Normal operation is at-most-once. If a process crashes after a successful POST but before atomic promotion to the success marker, stale takeover MAY redeliver and crash recovery is at-least-once. Exactly-once delivery is not promised.

These adapter records are observability and delivery state only. They MUST NOT alter Conflux workflow routing or change repository-verifiable completion.

#### Scenario: Absolute path cannot replace loopback base

- **GIVEN** the callback is configured with a loopback base URL
- **WHEN** its path argument is absolute, protocol-relative, a backslash origin variant, or resolves to a different origin
- **THEN** the callback rejects before sending HTTP
- **AND** no successful-delivery marker is written

#### Scenario: Redirect cannot leave loopback

- **GIVEN** the callback sends to a loopback endpoint
- **WHEN** the endpoint returns an HTTP redirect
- **THEN** the callback treats it as failure without following the redirect
- **AND** no request is sent to the redirect destination

#### Scenario: Concurrent callbacks deliver at most once

- **GIVEN** two callback processes receive the same execution event concurrently
- **WHEN** both attempt to claim delivery
- **THEN** atomic claim creation permits at most one process to POST
- **AND** the other process reports a distinct non-success in-flight outcome

#### Scenario: Failed delivery remains retryable

- **GIVEN** a callback owns the in-flight claim but its POST fails
- **WHEN** the process settles
- **THEN** it does not write a successful-delivery marker
- **AND** it releases the in-flight claim
- **AND** a later invocation may claim and attempt delivery

#### Scenario: Successful delivery remains deduplicated

- **GIVEN** the OpenCode POST succeeds
- **WHEN** a later callback receives the same execution event
- **THEN** it observes the successful-delivery marker and does not POST again
- **AND** it exits successfully

#### Scenario: Stale in-flight claim does not permanently suppress delivery

- **GIVEN** an in-flight claim whose owning process died without settling
- **WHEN** a later invocation finds the claim older than five minutes
- **THEN** it atomically takes over the claim and attempts delivery
- **AND** a fresh claim remains refused with a non-success in-flight outcome

#### Scenario: Crash after POST can redeliver observably

- **GIVEN** a process crashes after POST succeeds but before success-marker promotion
- **WHEN** its claim becomes stale and a later invocation takes it over
- **THEN** the later invocation may POST again
- **AND** automation-marker evidence makes the duplicate resume observable
- **AND** Conflux workflow completion remains unchanged
