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

### Requirement: Reference OpenCode callback enforces a local operating-system trust boundary

The reference OpenCode auto-resume integration MUST accept only literal IPv4 or IPv6 loopback HTTP destinations and MUST reject hostnames, including `localhost`, before opening a connection. Callback deduplication state MUST be private to the invoking operating-system user. A configured state path MUST be a real directory rather than a symlink, MUST be owned by the current user where the platform exposes ownership, and MUST have owner-only permissions. The plugin MUST register a sink only from a supported versioned enqueue envelope with a successful admission outcome and non-empty string binding identifiers.

#### Scenario: Hostname spelling cannot assert loopback

- **GIVEN** the OpenCode server is configured as `http://localhost:<port>` or another hostname
- **WHEN** the callback or plugin validates the destination
- **THEN** it rejects before opening a connection
- **AND** literal `127.0.0.1` and `[::1]` remain valid

#### Scenario: Pre-created state cannot control delivery

- **GIVEN** the configured or default callback state path is a symlink, a non-directory, is owned by another user where ownership is available, or permits group or world access
- **WHEN** the callback attempts to claim an execution event
- **THEN** it rejects before reading or creating `.inflight` or `.done` records
- **AND** it does not send an OpenCode request

#### Scenario: Private state supports normal deduplication

- **GIVEN** the callback state directory is a real owner-owned directory with mode `0700`
- **WHEN** a valid completion event is delivered
- **THEN** claim and successful-delivery records operate with the existing retry and deduplication semantics

#### Scenario: Incompatible enqueue envelope fails closed

- **GIVEN** an enqueue tool result has an unsupported schema version, a non-admission outcome, or a missing, empty, or non-string `change_id`, `execution_id`, or `instance_id`
- **WHEN** the OpenCode plugin extracts the execution binding
- **THEN** it returns no binding
- **AND** it does not register a completion sink

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

The reference Hermes auto-resume integration MUST extract the enqueue envelope from Hermes host tool-result representation, including its `structuredContent` and textual `result` wrapper fields. It MUST register an execution-scoped completion sink only from a supported versioned enqueue envelope with a successful admitted outcome and non-empty string `change_id`, `execution_id`, and `instance_id`. It MUST bind that execution to the originating messaging platform, chat ID, optional thread ID, and the Conflux owner route selected by the qualifying tool call.

The normal public route selector MUST be a project directory. The post-tool hook MUST preserve a non-empty string `project_dir` from the qualifying enqueue tool arguments and use it for callback registration. A non-empty string `unix_socket` MAY be preserved as a low-level alternative when that was the call's only route selector. The two selectors MUST NOT be accepted together. A process-global `CFLX_UNIX_SOCKET` MAY be used only as a backward-compatible fallback when the host does not expose post-tool arguments at all. If the host exposes the arguments object but the qualifying enqueue used the MCP server's current-directory or namespace default route, the hook MUST NOT guess from the environment; it MUST fail closed unless an admitted result provides an authoritative resolved route. Any call-scoped selector MUST take precedence over environment fallback.

Every registration MUST include the admitted envelope's `instance_id`, `execution_id`, and `change_id`. A fallback or stale route naming another owner MUST therefore receive the existing typed `owner_restarted` or execution-binding refusal rather than silently registering against the wrong owner.

The integration MUST NOT infer a project from `change_id`, retain a mutable project-to-socket map, require the Hermes MCP server registration to be fixed to one project, or start an owner. A malformed, conflicting, or unresolved call-scoped route MUST fail closed rather than silently register against an environment-selected different owner.

The callback MUST invoke an absolute Hermes executable as a fixed argv equivalent to `hermes send --quiet --to <platform>:<chat-id>[:<thread-id>] <message>`. `--to` MUST be the complete delivery destination. It MUST set explicit `HOME`, `PATH`, and `HERMES_HOME` values because Conflux scrubs the callback environment. It MUST NOT use the Hermes API Server, native wake, a webhook, shell evaluation, polling, or a watcher. Secret values MUST NOT appear in callback argv or diagnostics.

The callback MUST validate `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`, and MUST treat event contents only as data. Callback failure MUST remain observability only and MUST NOT change Conflux workflow outcome.

The generated bot message MUST identify itself as an automation event rather than user-authored instruction, MUST contain a typed `event:` line and the exact `instance_id`, `execution_id`, and `change_id` binding, and MUST require verification of current owner and repository evidence before success is reported. Callback delivery remains observability and a responder-compatible trigger only; it MUST NOT itself run an agent loop or alter Conflux workflow routing or terminal classification. A deployment that wants automatic continuation MUST separately provide a responder capable of observing the delivered bot post; this integration does not establish that observation path.

The post-tool hook MUST be observational: refusal, malformed output, missing routing, or registration failure MUST NOT replace the tool result or fail the Hermes turn, and diagnostics MUST remain bounded and secret-free. Request-scoped messaging and project routing context MUST be authoritative; process-global environment mirrors MAY be used only as compatibility fallbacks when no corresponding request-scoped value was supplied.

#### Scenario: Admitted enqueue registers the originating messaging thread

- **GIVEN** Hermes completes `cflx_enqueue` or an MCP name such as `mcp__cflx__cflx_enqueue`
- **AND** the result is a supported successful admitted envelope with complete binding identifiers
- **AND** Hermes request context identifies a messaging platform and chat, with an optional thread
- **AND** the tool arguments identify the Conflux project directory
- **WHEN** the post-tool hook runs
- **THEN** it registers one callback for that exact execution binding through the same call-scoped project route
- **AND** the callback argv contains the fixed messaging target and explicit profile environment paths but no secret value

#### Scenario: Two project calls retain independent owner routing

- **GIVEN** one Hermes process calls enqueue for project A using project directory A
- **AND** it calls enqueue for project B using project directory B
- **WHEN** each post-tool hook registers its completion sink
- **THEN** project A registration reaches only project A's owner
- **AND** project B registration reaches only project B's owner
- **AND** neither call mutates process-global routing for the other

#### Scenario: Call-scoped project overrides compatibility fallback

- **GIVEN** `CFLX_UNIX_SOCKET` names project A's owner
- **AND** a qualifying enqueue call names project directory B
- **WHEN** the post-tool hook registers the sink
- **THEN** it resolves and uses project B's route
- **AND** it does not contact project A's owner

#### Scenario: Low-level socket route remains supported

- **GIVEN** a qualifying enqueue call supplies `unix_socket` and omits `project_dir`
- **WHEN** the post-tool hook registers the sink
- **THEN** it preserves that socket route for `notify set`

#### Scenario: Conflicting call-scoped routes fail closed

- **GIVEN** a qualifying enqueue call contains both `project_dir` and `unix_socket`
- **WHEN** the post-tool hook runs
- **THEN** it registers no completion sink
- **AND** it does not fall back to process-global routing
- **AND** it leaves the original tool result and Hermes turn unchanged

#### Scenario: Legacy host may use environment fallback

- **GIVEN** a qualifying admitted enqueue result
- **AND** the legacy host does not expose a post-tool arguments object
- **AND** `CFLX_UNIX_SOCKET` contains a non-empty socket path
- **WHEN** the post-tool hook runs
- **THEN** it MAY register through that fallback socket

#### Scenario: Unsupported enqueue result fails closed

- **GIVEN** an enqueue result has an unsupported schema, unsuccessful or non-admitted outcome, malformed operation, or missing binding identifier
- **WHEN** the post-tool hook runs
- **THEN** it registers no callback
- **AND** it starts no wait or polling process

#### Scenario: Callback posts a responder-compatible Slack bot message

- **GIVEN** Conflux invokes the callback for a terminal execution event
- **AND** the selected Hermes profile can deliver to the bound messaging target
- **WHEN** callback delivery succeeds
- **THEN** the callback invokes `hermes send --quiet --to` with the bound Slack channel/thread target
- **AND** the body contains an explicit non-user-authored automation marker
- **AND** the body contains a typed event the responder can classify
- **AND** the callback does not directly start an agent loop

#### Scenario: Hermes host wrapper yields the admitted envelope

- **GIVEN** Hermes wraps the admitted enqueue envelope in `structuredContent` and textual `result` fields
- **WHEN** the post-tool hook runs
- **THEN** it extracts the typed envelope from the host wrapper
- **AND** it registers the exact execution binding once

#### Scenario: Concurrent turns retain request-scoped routing

- **GIVEN** concurrent Hermes turns have different messaging targets
- **WHEN** their post-tool hooks register callbacks
- **THEN** each callback uses its request-scoped target
- **AND** neither callback reads another turns process-global environment mirror

#### Scenario: Missing or non-messaging routing context registers nothing

- **GIVEN** Hermes request context has no supported messaging platform or no chat ID
- **WHEN** the post-tool hook runs
- **THEN** it registers no callback
- **AND** it starts no API self-POST, webhook, wait, watcher, or polling process

#### Scenario: Scrubbed callback environment is reconstructed explicitly

- **GIVEN** Conflux invokes the callback with only the five documented `CFLX_*` variables
- **WHEN** the callback delivers the event
- **THEN** it sets the configured `HOME`, `PATH`, and `HERMES_HOME`
- **AND** it invokes the absolute Hermes executable without a shell
- **AND** a non-zero Hermes exit remains a callback delivery failure without changing workflow state

<!-- Expected canonical result after archive: Hermes callback registration follows each enqueue call's project directory or explicit low-level socket, while process environment remains only a compatibility fallback. -->
