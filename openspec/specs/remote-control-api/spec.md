### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. `/api/v2` is the only versioned remote-control namespace; the removed multi-project `/api/v1` namespace MUST NOT be reintroduced.

#### Scenario: Client discovers and snapshots one process

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities, instance, and state
**Then**: The client receives supported commands/transports, a process-incarnation ID, and a coherent reducer-derived snapshot

#### Scenario: Removed multi-project namespace is not served

**Given**: A single cflx process has web monitoring enabled
**When**: A client requests a `/api/v1` resource
**Then**: The request is not served

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, `resolve_merge`, and `set_all_execution_marks` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted operator commands MUST execute through the same process-local application transaction used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution, derive command admission from a stale frontend mode, or maintain an independent workflow state machine.

The API MUST settle a command as succeeded, no-op, or failed from the actual shared-service outcome. An ordinary failed command MUST NOT retain partial reducer, execution-mark, queue, retry-edge, resolve-reservation, stop-flag, process-mode, scheduler, hook, event, or projection effects. A two-phase termination command that fails after cancellation issuance MUST commit no dequeue decision state or outcome event and MUST NOT repeat that intentional runtime request.

#### Scenario: Accepted command uses shared behavior

**Given**: A command variant is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared application transaction revalidates and executes it
**And**: TUI-equivalent reducer, scheduler, cancellation, side-effect, process-mode, resolve, and event semantics apply
**And**: the command settles from the actual changed, no-op, or failed outcome

#### Scenario: Empty Start is not successful

**Given**: No eligible execution-marked change exists at final admission
**When**: Start is submitted
**Then**: No scheduler is prepared, activated, or notified
**And**: no state or projection changes
**And**: the command settles as no-op or failed with actionable detail

#### Scenario: Failed dispatch does not certify partial intent

**Given**: Start, retry, or active resolve passes target validation
**And**: scheduler preparation fails before commit
**When**: the command record settles
**Then**: the command is failed
**And**: the snapshot at `result_revision` equals the pre-command decision state
**And**: no staged queue, mark, retry, reservation, mode, hook, or scheduler effect survives

#### Scenario: Retired set_parallel_mode command is rejected

**Given**: A command envelope names `set_parallel_mode`
**When**: It is submitted
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: No service call occurs and the state revision does not change

#### Scenario: Bulk mark classifies one revision

**Given**: Eligible and excluded changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from eligible rows only
**And**: It updates eligible marks and Running queue intent atomically
**And**: It returns changed IDs and stable exclusion reasons

### Requirement: Serialized optimistic revision control

One process-local projection/application owner MUST serialize exact idempotency lookup, ordinary new-command admission, final lifecycle and target revalidation, staged service mutation plus synchronous outcome dispatch, snapshot mutation, `state_revision`, `event_sequence`, event storage, command settlement, and publication. For an ordinary command, the serialization boundary MUST remain held until the accepted outcome or unchanged no-op/failure revision is stored, and the infallible prepared scheduler permit MUST be activated, or its wake issued, after that outcome dispatch and revision capture but still under the held boundary, so no other command is admitted between an accepted run decision and the activation it authorized. The boundary is released only after that activation.

A command whose accepted effect requires awaiting confirmed runtime termination MUST hold the serialization boundary only for admission, final validation, command-record reservation, and cancellation issuance. Confirmation MUST wait outside the serialization boundary, authoritative dispatch transaction, and TUI event loop. The command remains Running during that wait. It MUST reacquire the boundary, revalidate the target's current runtime state rather than the original `expected_revision`, and only then commit, dispatch, and settle. Exact replay during either phase returns the original record without a second cancellation or waiter. Force stop and unrelated operator commands remain admissible while confirmation is pending.

For each state-affecting input, the owner MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; after exact replay lookup, a new stale command MUST fail without service execution. Two ordinary new commands submitted with the same expected revision MUST NOT both execute after one consumes that revision. Snapshot mutations MUST publish all related decision fields coherently at the same resulting revision.

A changed command's `result_revision` MUST be the revision returned by its synchronous outcome dispatch. An ordinary no-op or failure MUST store the unchanged admitted revision, while a two-phase no-op or failure after the wait MUST store the explicit unchanged revision returned under the reacquired settlement boundary. No command MAY sample mutable global state after releasing its boundary, and later asynchronous progress MUST NOT rewrite the settled record.

#### Scenario: State event and snapshot share one revision

**Given**: A state-affecting execution or operator-outcome event changes the current snapshot
**When**: The projection owner processes it
**Then**: It increments revision once
**And**: The stored snapshot and published event contain the same resulting revision

#### Scenario: No-op does not advance revision

**Given**: an admitted command produces no service or snapshot change
**When**: its command record settles
**Then**: `state_revision` is unchanged
**And**: `result_revision` equals the unchanged admitted revision

#### Scenario: Ordinary failure is revision-idempotent

**Given**: an ordinary command fails validation or preparation after reservation
**When**: its record settles
**Then**: no command side effect remains
**And**: `result_revision` equals the unchanged admitted revision
**And**: no state event is published for a mutation that did not commit

#### Scenario: Two-phase failure does not certify dequeue

**Given**: stop-and-dequeue issued cancellation and later times out or fails post-wait revalidation
**When**: its record settles
**Then**: no dequeue reducer mutation or outcome event is committed
**And**: `result_revision` is the explicit unchanged revision returned under the reacquired settlement boundary
**And**: replay does not issue cancellation again

#### Scenario: Termination wait does not block other commands

**Given**: stop-and-dequeue is Running while it awaits confirmed termination
**When**: a valid force-stop or unrelated operator command is submitted
**Then**: that command executes and settles without waiting for the dequeue timeout
**And**: event fan-out and TUI rendering remain live
**And**: exact replay returns the original Running stop-and-dequeue record without another cancellation request

#### Scenario: Stale new command is rejected

**Given**: The current state revision is 12
**When**: A new command supplies expected revision 11
**Then**: The server returns HTTP 409 with `stale_revision` and current revision 12
**And**: No command side effect occurs

#### Scenario: Concurrent commands cannot consume one revision twice

**Given**: commands A and B are new identities carrying expected revision 12
**And**: A changes the authoritative snapshot
**When**: A and B are submitted concurrently
**Then**: only A executes against revision 12
**And**: B is rejected stale before service execution
**And**: B cannot be reported successful from a later projection refresh

#### Scenario: Mark mutation reads back coherently

**Given**: An accepted command changes an execution mark without changing queue intent
**When**: The resulting state revision is read
**Then**: The snapshot reports the new execution mark and unchanged queue intent together
**And**: the live TUI receives the same target delta
**And**: No client-side inference is required

#### Scenario: Command result revision includes admission effect

**Given**: A lifecycle command changes marks, queue intent, process mode, or resolve reservation during admission
**When**: Its command record settles
**Then**: `result_revision` identifies the one snapshot containing all of that command's synchronous accepted decision fields
**And**: Later asynchronous progress may advance revision separately
**And**: that later progress does not rewrite `result_revision`

#### Scenario: Two-phase command records its commit revision

**Given**: stop-and-dequeue was admitted at revision 12
**And**: unrelated commands or lifecycle events advance projection while termination is pending
**When**: confirmed termination is revalidated and `ChangeDequeued` commits
**Then**: `result_revision` is the revision returned by that dequeue outcome dispatch
**And**: it is not revision 12 or a later sampled revision unrelated to the outcome
**And**: timeout or failed revalidation stores the unchanged revision observed at settlement without publishing a dequeue event

### Requirement: Structurally idempotent side-effect commands

Within one process incarnation, command identity MUST be the typed tuple `(type, target, params, expected_revision)` after schema defaults are applied. JSON member order and whitespace MUST NOT affect identity. `idempotency_key` and `correlation_id` MUST NOT be identity inputs. Idempotency lookup MUST precede current-revision validation so exact replay returns the original record after state advances; a new key MUST pass revision validation before atomic command/idempotency reservation and service execution.

Exact replay of a running or completed record MUST NOT enter the application transaction again. Replay MUST retain the original command ID, state, detail, error code, and `result_revision`, and MUST NOT repeat reducer, scheduler, cancellation, queue hook, retry edge, resolve reservation, event, or projection effects.

#### Scenario: Same typed request is replayed

**Given**: An idempotent command completed and state revision later advanced
**When**: The same key and structurally equivalent typed command are submitted again
**Then**: The original command ID, state, detail, and `result_revision` are returned
**And**: The side effect, scheduler action, and authoritative outcome event are not repeated

#### Scenario: In-progress replay joins the original command

**Given**: an admitted command is still running inside the serialized application transaction
**When**: the same key and typed identity are submitted again
**Then**: the original running record is returned
**And**: no second service execution is started

#### Scenario: Same key with different expected revision conflicts

**Given**: A key is bound to one typed command identity
**When**: The same key is submitted with a different expected revision
**Then**: The server returns HTTP 409 with `idempotency_mismatch`
**And**: No side effect occurs

### Requirement: Bounded fail-closed command and observation history

The process MUST retain the latest 1000 events and 1000 logs. Command and idempotency registries MUST each admit at most 1000 records and MUST reserve corresponding records atomically before executing a side effect. Expired or oldest completed records MAY be evicted; in-progress records MUST NOT be evicted. If capacity cannot be made available without evicting in-progress work, admission MUST return HTTP 503 with `registry_capacity` and MUST NOT execute the command. Completed records MUST expire after 24 hours or process termination.

#### Scenario: In-progress capacity does not permit duplicate execution

**Given**: Registry capacity is occupied by in-progress commands
**When**: A new side-effect command is submitted
**Then**: The server returns `registry_capacity`
**And**: No in-progress record is evicted
**And**: The new side effect is not executed

#### Scenario: Event replay gap requires snapshot

**Given**: A client requests an event sequence older than the retained ring
**When**: It connects to SSE or WebSocket
**Then**: The server signals a replay gap
**And**: The client can recover with `GET /api/v2/state`

### Requirement: Process incarnation and validated correlation

The process MUST generate a new random 128-bit hexadecimal `instance_id` at startup. Events MUST have monotonic process-local `event_sequence` values and include the associated state revision. A caller correlation ID MUST be 1-64 ASCII characters matching `[A-Za-z0-9._:-]+`; it MUST remain an opaque trace label and MUST NOT be used for authorization, resource lookup, uniqueness, or idempotency.

#### Scenario: Restart invalidates replay cursor

**Given**: A client stored an instance ID and event cursor
**When**: The process restarts
**Then**: The instance ID changes
**And**: The old cursor is not treated as valid for the new process

#### Scenario: Invalid correlation ID is rejected

**Given**: A command supplies a correlation ID containing a newline or exceeding 64 characters
**When**: It is submitted
**Then**: The server returns HTTP 422 with `validation_failed`

### Requirement: Safe web authentication and binding

The process-scoped v2 server MUST bind a repository-scoped Unix domain socket by default in web-enabled local orchestration. The socket MUST be created with owner-only mode `0600`. Token-free UDS and loopback TCP binding are permitted; non-loopback TCP binding MUST be refused unless a non-empty bearer token is configured. Direct token and token-environment options MUST be mutually exclusive. One configured authentication policy MUST apply to every active listener: `/api/v2/health` remains unauthenticated and every other v2 HTTP, SSE, and WebSocket resource requires `Authorization: Bearer` authentication.

Before UDS bind, a non-socket entry or connectable live socket at the target path MUST be preserved and startup MUST fail. Only an unreachable stale socket entry may be removed. Shutdown MUST remove only the socket entry created by the current process. Tokens in URLs, query parameters, logs, correlation IDs, WebSocket subprotocols, durable browser storage, or Unix endpoint metadata MUST remain forbidden.

#### Scenario: Default UDS is locally accessible without token

**Given**: A web-enabled local orchestration process uses its default socket and no bearer token is configured
**When**: A local client connects through that socket
**Then**: `/api/v2/health` and other v2 resources follow the token-free local policy
**And**: The socket file mode is `0600`

#### Scenario: Configured token protects both transports

**Given**: Bearer authentication and both UDS and TCP listeners are configured
**When**: A client requests a protected v2 resource over either listener without the token
**Then**: Both requests are rejected as unauthorized
**And**: `/api/v2/health` remains available over both listeners

#### Scenario: Unsafe non-loopback startup is rejected

**Given**: Web monitoring is configured on a non-loopback TCP address without a token
**When**: The process starts its listeners
**Then**: Startup fails before any requested listener is published or orchestration begins

#### Scenario: Live socket is preserved

**Given**: The selected Unix path contains a socket accepting connections
**When**: Conflux attempts startup
**Then**: Startup fails without unlinking the socket

#### Scenario: Non-socket target is preserved

**Given**: The selected Unix path contains a regular file or directory
**When**: Conflux attempts startup
**Then**: Startup fails without modifying that entry

#### Scenario: Unreachable stale socket is replaced

**Given**: The selected Unix path contains a socket entry that cannot accept a connection
**When**: Conflux starts the listener
**Then**: It removes the stale socket entry and binds the new listener

#### Scenario: Shutdown does not delete a replacement

**Given**: The process-bound socket path was externally unlinked and replaced after startup
**When**: Conflux shuts down
**Then**: It does not remove the replacement entry

### Requirement: Exact-origin V2 CORS

The `/api/v2` router MUST allow requests without an Origin header, direct same-origin requests, and explicitly configured exact origins. Direct same-origin MUST compare parsed Origin scheme, host, and port with the direct request origin. Wildcards and forwarded-header-derived origin expansion MUST be forbidden. Reverse proxies that change external origin MUST configure an exact allowed origin.

#### Scenario: Foreign origin is denied

**Given**: No matching exact origin is configured
**When**: A browser sends a v2 request from a different origin
**Then**: The server does not grant cross-origin access

#### Scenario: Exact proxy origin is allowed

**Given**: An external reverse-proxy origin is explicitly configured
**When**: A browser sends a request with that exact Origin
**Then**: The server grants cross-origin access without consulting forwarded headers

### Requirement: Stable typed remote-control errors

Every v2 error MUST include `error_code`, sanitized `message`, `correlation_id`, and `current_revision` when applicable. Initial error codes MUST include `unauthorized`, `forbidden`, `not_found`, `stale_revision`, `lifecycle_conflict`, `target_ineligible`, `root_busy`, `idempotency_mismatch`, `registry_capacity`, `validation_failed`, and `internal_error`.

#### Scenario: Client distinguishes stale revision from idempotency mismatch

**Given**: Two requests both map to HTTP 409 for different causes
**When**: One is stale and one reuses an idempotency key with different identity
**Then**: Their error codes are `stale_revision` and `idempotency_mismatch` respectively

### Requirement: Legacy web contract compatibility

The single-instance web server MUST serve the embedded static operator console and the `/api/v2` contract. The legacy single-instance `/api/*` and `/ws` compatibility surface MUST be removed after the console migrates to v2. Removal MUST NOT affect `/api/v2`, OpenAPI resources, or static asset delivery, and requests to removed mutation routes MUST have no side effect. The retained surface MUST NOT depend on the removed multi-project `/api/v1` namespace.

#### Scenario: Legacy monitoring route is absent

**Given**: The v2 operator console is mounted
**When**: A client requests a legacy single-instance `/api/*` route or `/ws`
**Then**: The server returns not found
**And**: No command, Git, or orchestration side effect occurs

#### Scenario: Versioned console remains available

**Given**: Legacy monitoring compatibility is removed
**When**: A user opens the console and it accesses `/api/v2`
**Then**: Static assets and versioned resources remain available
**And**: The console can monitor and control the single process through v2

### Requirement: Authoritative operator snapshot

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs. A change-scoped `ProcessingError` MUST update the failed change's display status, bounded sanitized error detail, activity, attention, action eligibility, and reconciled execution mark without changing `app_mode` or setting `process_error`. Only a typed process-fatal event MAY set process-wide Error state. For each change whose command-capable run owns typed Apply iteration-limit evidence and whose scheduler task reports live, the snapshot MUST block `retry_change` with `apply_iteration_limit_active` at the same state revision. Projection and command admission MUST consult the same scheduler-liveness authority. A live-to-exited scheduler transition MUST publish the changed authoritative action snapshot without waiting for unrelated repository activity. Record presence without live ownership MUST NOT remain an action blocker. A headless `cflx run` process with no bound command executor or scheduler-liveness authority MUST omit this process-local blocked reason; command submission remains unavailable through the existing unbound-runtime lifecycle contract.

#### Scenario: Change-local processing error preserves process snapshot mode

**Given**: `/api/v2/state` reports `app_mode: running` for a multi-change run
**When**: `ProcessingError` is authoritatively dispatched for change `alpha`
**Then**: `alpha.display_status` is `error` and its sanitized `error_detail` is present
**And**: `alpha.execution_marked` reflects the same-revision mark reconciliation
**And**: `app_mode` remains `running`
**And**: `process_error` remains null
**And**: unrelated changes retain their current marks and action eligibility

#### Scenario: Fatal process error remains distinct

**Given**: a snapshot may already contain one or more change-level errors
**When**: a typed fatal `ExecutionEvent::Error` is authoritatively dispatched
**Then**: `app_mode` becomes `error`
**And**: `process_error` contains the sanitized fatal detail
**And**: change-level error details remain distinguishable from the process failure

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

#### Scenario: Active iteration limit is projected as typed eligibility

**Given**: A command-capable run owns `ApplyIterationLimit` for change `alpha` with attempts 50 and max 50
**And**: The owning scheduler task reports live
**When**: A client reads `/api/v2/state`
**Then**: `alpha.actions.retry_change.allowed` is false
**And**: Its blocked reason is `apply_iteration_limit_active`
**And**: No client must parse the error detail, display status, iteration count, or logs

#### Scenario: Scheduler-task exit removes the active action block

**Given**: The finish-hook owner observed `alpha`'s typed iteration-limit evidence
**When**: The owning scheduler task exits while the old record remains in shared state
**Then**: The liveness transition publishes a new authoritative revision
**And**: That snapshot does not block `alpha` with `apply_iteration_limit_active`
**And**: Retry eligibility is derived from `alpha`'s remaining current evidence

#### Scenario: Headless read-only projection does not retain an actionable block

**Given**: `cflx run` serves `/api/v2` without a bound command executor
**And**: Its old shared state retains typed iteration-limit evidence after the run
**When**: A client reads the subsequent snapshot
**Then**: The snapshot does not expose `apply_iteration_limit_active` as a current action block
**And**: A submitted command is refused by the existing unbound-runtime lifecycle contract

<!-- Expected canonical result after archive: the authoritative snapshot requirement will explicitly separate change-local ProcessingError fields from process-wide app_mode/process_error. -->

### Requirement: Shared lifecycle scheduling semantics

Start, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, refuse an active-run Apply iteration limit before mutation, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification. `retry_change`, `retry_errors`, and a terminal-error `set_queue_intent=true` alias MUST share the same typed limit guard. An all-limited command MUST settle truthfully without notifying or starting a scheduler.

#### Scenario: Retry dispatches reconciled work

**Given**: A marked error, stalled acceptance hold, or resumable external blocker is valid for retry
**When**: Retry is accepted
**Then**: The shared service applies the correct retry route
**And**: The scheduler is notified or started
**And**: Unsupported holds retain their blocker evidence

#### Scenario: Resolve queues behind an active resolver

**Given**: One merge resolution is active
**When**: Another valid merge-wait change is submitted for resolve
**Then**: It is reserved once in FIFO order
**And**: Duplicate submission does not create another queue entry

#### Scenario: V2 individual retry reports active limit refusal

**Given**: The authoritative snapshot blocks `alpha` retry with `apply_iteration_limit_active`
**When**: A client submits `retry_change` or terminal-error `set_queue_intent=true` for `alpha` at the current revision
**Then**: The shared service rejects the command with a typed target-ineligible result
**And**: The command record does not claim a scheduler effect
**And**: Reducer, mark, queue, hook, explicit-retry, and scheduler state remain unchanged

#### Scenario: V2 bulk retry remains partial

**Given**: `alpha` is active-run limited and `beta` is ordinarily retryable
**When**: A client submits `retry_errors` for both at the current revision
**Then**: `beta` is retried and dispatched exactly once
**And**: The result does not claim that `alpha` was accepted
**And**: `alpha.actions.retry_change.blocked_reason` remains `apply_iteration_limit_active` in the authoritative snapshot at the result revision

#### Scenario: Retry after run closure starts a later boundary

**Given**: The scheduler boundary that limited `alpha` has completed finish-hook ownership and closed
**When**: A current-revision retry for `alpha` is accepted
**Then**: It cannot notify the closed scheduler
**And**: It may start a new scheduler boundary with workspace-derived state and a fresh budget

### Requirement: Remote parallel execution discovery

Capabilities and state MUST expose maximum concurrency, VCS backend, and per-change worktree eligibility with machine-readable blocked reasons. They MUST NOT expose an active execution-mode dimension or distinguish serial from parallel modes.

#### Scenario: Client discovers worktree execution state

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities and state
**Then**: It can read concurrency, VCS, and eligibility without an execution-mode field
**And**: It can explain why each non-final change is or is not eligible without inspecting Git itself

### Requirement: Atomic parallel start eligibility

Parallel start MUST validate the complete marked target set at the admitted revision. If any marked target is ineligible, start MUST reject the complete operation without spawning a scheduler or partially changing queue state.

#### Scenario: One ineligible mark rejects start

**Given**: Two changes are marked and one is not parallel-eligible
**When**: Parallel start is submitted
**Then**: Neither change starts
**And**: The response identifies the ineligible target and reason
**And**: Marks and queue intent remain coherent

### Requirement: Canonical OpenAPI ownership

The generated OpenAPI document produced from the source declarations MUST be the canonical contract of `/api/v2`; the repository MUST NOT track a generated OpenAPI YAML or JSON artifact. Every supported v2 route and schema MUST appear in the deterministic document exposed by both `cflx openapi` and `GET /api/v2/openapi.yaml`. Stale legacy routes MUST NOT appear as supported API paths.

#### Scenario: CLI and live endpoint share the canonical contract

**Given**: one `cflx` build with web monitoring enabled
**When**: a client captures `cflx openapi` and `GET /api/v2/openapi.yaml`
**Then**: both outputs contain the same deterministic OpenAPI document
**And**: all supported v2 routes and schemas are present

#### Scenario: Contract completeness fails validation

**Given**: a route, DTO field, command variant, error code, event envelope, or security declaration is absent from the generated contract
**When**: repository-local OpenAPI contract verification runs
**Then**: verification fails with an assertion identifying the missing contract element
**And**: verification does not write generated artifacts into the working tree

#### Scenario: Consumer exports the canonical contract

**Given**: a generated client or schema assertion needs the v2 contract
**When**: it invokes `cflx openapi` or reads the live OpenAPI endpoint
**Then**: it receives the canonical generated document
**And**: it can validate every current command and authoritative snapshot field

#### Scenario: Security and recovery semantics are documented

**Given**: a client reads the generated canonical API contract
**When**: it inspects authentication, events, commands, and worktree schemas
**Then**: it can identify bearer-header authentication, fetch-streamed SSE, process incarnation, replay-gap resnapshot, revision and idempotency rules, and opaque worktree safety

### Requirement: Event mark changes share the authoritative state revision

When a typed failure, rejection, rejected or parallel-ineligible refresh, dequeue, legacy stop, or first `on_merged` hook-recovery event revokes an execution mark, `/api/v2` MUST publish the reconciled `execution_marked` value in the same authoritative state revision as that event's reducer/frontend transition. The projection MUST read the shared `ExecutionMarkStore` after pre/post event reconciliation and MUST NOT wait for an unrelated refresh or create a second mark-only revision.

Duplicate or late delivery that changes neither reducer state nor execution marks MUST NOT advance another state revision. Event reconciliation MUST preserve unrelated marks in the same snapshot. A duplicate failure delivered after an explicit re-mark MUST preserve that fresh mark when it creates no new reducer transition.

#### Scenario: failure event and cleared mark are coherent

- **GIVEN** `alpha` and `beta` are marked in the authoritative operator snapshot
- **WHEN** a typed event transitions `alpha` into change-level Error
- **THEN** the event envelope's state revision identifies a snapshot where `alpha.execution_marked` is false
- **AND** `beta.execution_marked` remains true
- **AND** no intermediate revision exposes Error with the stale `alpha` mark

#### Scenario: rejected or ineligible refresh clears mark in its refresh revision

- **GIVEN** `alpha` and `beta` are marked active changes
- **WHEN** one authoritative refresh introduces `alpha` as a rejected marker or classifies it parallel-ineligible while `beta` remains eligible
- **THEN** that refresh revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true
- **AND** the client does not need prior-event replay or log parsing to reconcile the decision

#### Scenario: on_merged recovery and cleared mark are coherent

- **GIVEN** a marked change is in active merge handling
- **WHEN** its first `on_merged` hook failure enters reducer merge-wait recovery
- **THEN** the hook-failure event revision reports the recovery row and `execution_marked: false` together
- **AND** no intermediate revision exposes the recovery state with the stale mark

#### Scenario: duplicate revocation is revision-idempotent

- **GIVEN** the target mark is already false after a revoking event
- **WHEN** duplicate delivery produces no reducer or mark change
- **THEN** `/api/v2` does not advance another state revision
- **AND** unrelated execution marks remain unchanged

#### Scenario: duplicate failure preserves a fresh re-mark

- **GIVEN** a revoking event cleared a target and an operator explicitly re-marked its steady recovery row
- **WHEN** the same failure event is delivered again without creating a new reducer transition
- **THEN** the event revision retains `execution_marked: true`
- **AND** no mark-only correction revision is needed

#### Scenario: process stop retains marked resume targets

- **GIVEN** the snapshot contains execution-marked changes
- **WHEN** a process-level `Stopped` transition is published
- **THEN** the stopped revision retains those `execution_marked` values
- **AND** queue intent and reducer stop reconciliation remain separate from mark ownership

### Requirement: Persistent-scheduler idle is explicit in the operator snapshot

`InstanceSnapshot` MUST include a boolean `persistent_scheduler_idle` field that distinguishes Ready/`app_mode: select` backed by a live persistent-scheduler idle episode from ordinary pre-run Select. The field MUST default to false for a new process and MUST become true in the same authoritative revision where the typed persistent-idle transition performs its guarded Running-to-Ready projection; a late event against Select, Stopping, Error, or Stopped MUST NOT set it. Once true, it MUST remain true through a Start notification and an idle-origin graceful-stop request, and become false in the same authoritative projection that begins admitted work or enters Error or Stopped. It MUST remain process-local presentation state, MUST reset on restart, and MUST NOT authorize a command or influence workspace-derived workflow routing; shared run control MUST independently validate scheduler liveness.

The generated OpenAPI schema MUST include the field. A client that replaces local state after a replay gap MUST be able to derive idle Ready lifecycle controls from the snapshot without replaying prior events or parsing logs.

#### Scenario: idle event publishes one coherent snapshot

- **GIVEN** a persistent scheduler enters its first idle edge
- **WHEN** the authoritative dispatcher projects the typed persistent-idle event
- **THEN** the event revision identifies a snapshot with `app_mode: select`
- **AND** that snapshot has `persistent_scheduler_idle: true`
- **AND** duplicate or no-op idle observation creates no additional revision

#### Scenario: replay-gap snapshot restores idle controls

- **GIVEN** a client missed the persistent-idle event
- **WHEN** it replaces local state with `GET /api/v2/state`
- **THEN** `persistent_scheduler_idle: true` distinguishes live-idle Ready from pre-run Select
- **AND** the client can expose Start, graceful stop, and force stop without parsing logs
- **AND** shared run control still rejects the command if scheduler liveness no longer validates

#### Scenario: admitted work clears idle in one revision

- **GIVEN** the snapshot reports `persistent_scheduler_idle: true`
- **WHEN** a typed workspace or base-lane work-start event projects Running
- **THEN** the same resulting revision reports `app_mode: running`
- **AND** `persistent_scheduler_idle` is false

#### Scenario: idle-origin graceful stop retains episode identity

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **WHEN** graceful stop is accepted
- **THEN** the result revision reports `app_mode: stopping`
- **AND** `persistent_scheduler_idle` remains true
- **AND** accepted cancel-stop returns both fields to `app_mode: select` and `persistent_scheduler_idle: true`

#### Scenario: work start during stopping clears episode identity

- **GIVEN** the snapshot reports `app_mode: stopping` and `persistent_scheduler_idle: true`
- **WHEN** a typed work-start event is projected before cancel-stop
- **THEN** the same resulting revision retains `app_mode: stopping`
- **AND** reports `persistent_scheduler_idle: false`
- **AND** accepted cancel-stop subsequently projects `app_mode: running`

#### Scenario: generated contract owns the idle field

- **GIVEN** a consumer reads the canonical generated OpenAPI document
- **WHEN** it inspects `InstanceSnapshot`
- **THEN** the schema includes boolean `persistent_scheduler_idle`
- **AND** no tracked generated schema artifact is required

### Requirement: Agent-readable execution observation

The authenticated `GET /api/v2/execution-status` resource MUST provide a coherent machine-readable observation of scheduler liveness, actual active lifecycle work, typed current and last-completed phases, iteration and timing boundaries, latest lifecycle activity, and the latest retained structured log for the process and each exactly associated change. It MUST include the process incarnation, `state_revision`, `event_sequence`, and an `observed_at` instant.

Scheduler liveness MUST read the same process-local `RunBoundaryLiveness` authority used by operator snapshot eligibility and command admission, and MUST remain distinct from `has_active_work`. `has_active_work` MUST be true while at least one change has an active reducer `ActivityState`, or while a closed process-level dependency-analysis, base-branch-merge, conflict-resolution, branch-merge, or workspace-cleanup activity has received its typed start event without its typed terminal event. Persistent-idle scheduler liveness alone MUST NOT be active work. Command acceptance, `app_mode: running`, an execution mark, queue intent, task completion percentage, or a display status alone MUST NOT certify active work or a lifecycle phase.

Per-change phase MUST project the existing reducer activity authority as `preparing`, `apply`, `acceptance`, `rejection_review`, `archive`, `resolve`, typed `push`, `none`, or `unknown`. Analysis MUST remain process-level. `merge` MAY appear as a last-completed phase only from a typed per-change merge-completion fact. `hook` MUST NOT be advertised until production emits typed hook start/completion facts. Phase facts MUST update synchronously from the same source event under the authoritative dispatch boundary and MUST NOT form a second lifecycle state machine. Unknown evidence MUST remain explicitly unknown.

Every execution-status time MUST be an absolute UTC RFC 3339 instant. The resource MUST NOT return elapsed seconds, age seconds, localized relative-time text, or advance `state_revision` merely because time passed. A log-only observation MAY change latest-log output and `event_sequence` while retaining the current `state_revision`.

Latest-log selection MUST use bounded sanitized in-memory ring insertion order. A change latest log MUST be selected only by exact structured `change_id` equality. The resource MUST return a new closed projection containing only sanitized message, level, operation, iteration, and RFC 3339 UTC `created_at`; it MUST NOT embed the existing `LogEntry` wire shape, display timestamp, or `workspace_path`. The API MUST NOT read persistent log files to fill this resource, expose a persistent log path or file URL, accept a host path or filename for log access, or expose such a locator differently over UDS and TCP. Complete retained API logs MUST remain readable through authenticated `GET /api/v2/logs` and live observation through the existing event transports.

#### Scenario: Active change exposes absolute observation boundaries

**Given**: A change has completed Apply and is actively running Acceptance
**And**: Typed lifecycle facts and an exactly associated structured log are retained
**When**: A client reads execution status
**Then**: The change reports `current_phase: acceptance` and `last_completed_phase: apply`
**And**: `observed_at`, phase boundaries, activity time, and log creation time are absolute UTC RFC 3339 instants
**And**: No elapsed, age, or relative-time field is returned

#### Scenario: Idle scheduler is not active work

**Given**: A persistent scheduler task is alive but parked without admitted lifecycle work
**When**: A client reads execution status
**Then**: `scheduler_running` is true
**And**: `has_active_work` is false
**And**: application mode, marks, queue intent, or prior command success do not change that answer

#### Scenario: Log-only activity preserves state revision

**Given**: An execution-status response identifies state revision 12 and event sequence 30
**When**: A retained structured log arrives without changing the authoritative snapshot
**Then**: A later execution-status response may expose that latest log and a later event sequence
**And**: Its state revision remains 12

#### Scenario: Change latest log requires exact association

**Given**: The retained ring contains process logs, logs for `alpha`, and text mentioning `alpha` without structured association
**When**: A client reads execution status for `alpha`
**Then**: `alpha.latest_log` is the newest entry whose structured `change_id` exactly equals `alpha`
**And**: Message substring, operation, iteration, or workspace path is not used as fallback association

#### Scenario: Log path remains private on every transport

**Given**: The same process serves UDS and TCP listeners
**When**: A client reads capabilities, state, execution status, logs, events, commands, or OpenAPI over either listener
**Then**: No response exposes the persistent log path, repository root, workspace path as a log locator, or file URL
**And**: The client can still read the bounded structured log ring through `/api/v2/logs`

<!-- Expected canonical result after archive: v2 includes an agent-readable execution-status resource with absolute timestamps, exact structured log association, and a transport-independent private-log-path boundary. -->

### Requirement: Phase-aware immutable stop command result

A settled successful `stop_and_dequeue` command record MUST contain a closed typed result captured after confirmed termination and final lifecycle revalidation. The result MUST identify the cancelled phase, the last completed phase, nullable proof of the final managed-worktree Apply commit and its commit OID, and `effects_rolled_back: false`. The presentation detail MUST state that dequeue does not roll back previously completed worktree effects.

Apply commit evidence MUST retain each non-empty typed `ApplyCompleted.revision` OID as a per-change, per-process-incarnation fact. At settlement the server MUST identify the managed worktree through its own change mapping and prove the retained OID equals or is an ancestor of the quiescent worktree HEAD before reporting presence true and returning that retained OID. Missing or empty completion facts, restart-empty facts, absent worktree, Git failure, or non-ancestor evidence MUST be unknown; the server MUST NOT reconstruct evidence from task count, display status, log text, or commit subject. No result may expose branch, worktree, repository, or log paths.

The command registry MUST store the typed result with the settled command record. Exact idempotent replay MUST return the original result unchanged and MUST NOT re-read Git, reclassify phase, or repeat cancellation or dequeue effects after later state changes.

#### Scenario: Apply completes before acceptance cancellation settles

**Given**: The final managed-worktree Apply commit is created and Apply completion is published
**And**: Acceptance starts before stop-and-dequeue termination is confirmed
**When**: stop-and-dequeue settles successfully after cancelling Acceptance
**Then**: The typed result reports `cancelled_phase: acceptance` and `last_completed_phase: apply`
**And**: Apply commit presence is true and its OID equals the retained final Apply commit
**And**: `effects_rolled_back` is false
**And**: The final Apply commit remains in the managed worktree

#### Scenario: Already-terminated target has no cancelled phase

**Given**: The target task already terminated and no typed phase is active at settlement
**When**: stop-and-dequeue settles successfully through its existing already-terminated path
**Then**: The typed result reports `cancelled_phase: none`
**And**: It does not invent a phase from prior display or log evidence

#### Scenario: Stop settles before final Apply commit

**Given**: Apply is active and no final managed-worktree Apply commit has been created
**When**: stop-and-dequeue confirms termination and settles
**Then**: The typed result reports Apply as the cancelled phase
**And**: It does not claim that the final Apply commit is present
**And**: It states that no prior worktree effect was rolled back

#### Scenario: Unavailable evidence remains unknown

**Given**: Cancellation and dequeue succeed but lifecycle or managed-worktree Git evidence cannot be read safely
**When**: The command record settles
**Then**: Unavailable phase or Apply commit fields are explicitly unknown or null
**And**: The result does not infer them from display status, task progress, logs, or commit subject

#### Scenario: Exact replay preserves settlement evidence

**Given**: A stop-and-dequeue command settled with typed phase and Apply commit evidence
**And**: Later lifecycle or Git state changed
**When**: The same idempotency key and typed command identity are replayed
**Then**: The original command ID, state, detail, result, and result revision are returned unchanged
**And**: The server does not re-read Git or issue another cancellation

<!-- Expected canonical result after archive: stop-and-dequeue command records explain the actual cancelled phase and retained Apply effect without implying rollback or requiring clients to inspect Git independently. -->
