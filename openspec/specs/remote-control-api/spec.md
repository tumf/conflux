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

`POST /api/v2/commands` MUST continue to accept only the closed command set, including `set_execution_mark`, `set_all_execution_marks`, explicit queue commands, start/retry, stop, dequeue, and resolve controls. Accepted commands MUST execute through the same process-local application transaction used by the TUI.

`set_execution_mark` and `set_all_execution_marks` MUST represent process-local next-run target intent only. They MUST accept visible non-terminal targets for which the reducer has not recorded archive completion, independent of app mode, active/retry/wait status, Apply iteration-limit evidence, and current parallel eligibility, and MUST NOT mutate queue intent, active execution, cancellation, retry/resolve state, hooks, scheduler state, or process mode. Targets with terminal display status (`archived`, `merged`, `pushed`, or `rejected`) or reducer-recorded archive completion MUST settle as unchanged no-op outcomes with a stable non-markable-target reason and no effects.

Start/retry MUST perform current reducer and worktree eligibility checks at final admission. A worktree-ineligible marked target MUST reject the complete request before ordinary or retry-class Start selection. Other non-startable statuses MUST be excluded with target-specific detail, and zero runnable targets MUST reject. In Select and Stopped, Start MUST admit ordinary startable marks when any exist and otherwise MAY select marked retry routes. In Running and Error, Start MUST select marked retry routes only; Stopping MUST reject Start. Failed admission MUST NOT produce partial queue, scheduler, retry-edge, or projection effects.

#### Scenario: Single mark is lifecycle-independent and side-effect free

**Given**: A visible non-terminal change exists in any app mode or lifecycle status
**And**: The reducer has not recorded archive completion for that change
**When**: `set_execution_mark` changes its mark
**Then**: the shared mark store and coherent snapshot reflect the new value
**And**: queue intent, runtime, cancellation, retry, resolve, hooks, scheduler, and mode remain unchanged

#### Scenario: Non-markable single mark is a reasoned unchanged no-op

**Given**: A target has terminal display status or reducer-recorded archive completion
**When**: `set_execution_mark` is submitted
**Then**: the command settles successfully as unchanged
**And**: the outcome identifies a stable non-markable-target reason
**And**: no mark, queue, runtime, revision, or scheduler effect occurs

#### Scenario: Bulk mark changes only markable rows

**Given**: Visible markable and non-markable changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from rows without terminal display status or reducer-recorded archive completion only
**And**: It updates only execution marks atomically
**And**: It returns changed IDs without Running queue-intent effects

#### Scenario: Worktree-invalid Start is rejected atomically

**Given**: Marks include a worktree-ineligible target, including a retry-eligible target
**When**: Start is submitted
**Then**: the complete request is rejected before ordinary or retry-class selection
**And**: No scheduler is prepared, activated, or notified
**And**: no queue, mark, retry-edge, reservation, mode, hook, or projection effect survives
**And**: the command identifies the target and reason

<!-- replaces-scenario: Mixed status Start admits runnable subset -->

#### Scenario: Mixed status Start admits the route permitted by mode and evidence

**Given**: Marks include at least one target permitted by the current Start route and another currently non-startable status
**And**: no marked target violates the worktree eligibility fence
**When**: Start is submitted
**Then**: permitted targets are admitted
**And**: non-startable statuses are reported as excluded with target-specific detail

#### Scenario: Zero runnable targets is rejected

**Given**: Marks exist but no runnable target remains after status and retry classification
**When**: Start or Retry is submitted
**Then**: no scheduler or queue effect occurs
**And**: the command rejects with actionable exclusion detail

#### Scenario: Remote Start uses ordinary priority before retry fallback

**Given**: Select or Stopped has ordinary startable marks and retry-only marks
**When**: remote `start` is submitted
**Then**: ordinary targets are admitted and retry-only targets are excluded
**And**: exclusion detail explains that ordinary marks must be removed before retry-class Start can select the retry-only targets
**And**: remote behavior matches TUI F5 for the same authoritative state

#### Scenario: Remote Running Start can retry a change-local error

**Given**: a scheduler is live in Running mode
**And**: a marked target carries retry-eligible change-local terminal Error evidence
**When**: remote `start` is submitted
**Then**: the target is admitted through the same explicit retry transaction as TUI F5
**And**: the accepted result records the same targets, exclusions, retry semantics, scheduler wake, and revision

<!-- Expected canonical result after archive: `/api/v2` preserves lifecycle-independent mark commands and routes remote Start through the same mode-aware ordinary-or-retry transaction and complete-request worktree fence as TUI F5. -->

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

The state resource MUST be a coherent reducer-derived operator snapshot that includes every server-authoritative field needed to determine current change presentation and permitted operator actions without replaying prior events or parsing logs. A change-scoped `ProcessingError` MUST update the failed change's display status, bounded sanitized error detail, activity, attention, action eligibility, and reconciled execution mark without changing `app_mode` or setting `process_error`. Only a typed process-fatal event MAY set process-wide Error state. A settled terminal-error change that retains typed Apply iteration-limit evidence MUST expose ordinary retry eligibility when its evidence otherwise supports retry, even while a persistent scheduler task remains live. Projection and command admission MUST use the same retry classifier. The retained iteration-limit record MAY remain visible as diagnostic evidence but MUST NOT be an operator-action block. A headless `cflx run` process with no bound command executor continues to expose read-only state while command submission remains unavailable through the existing unbound-runtime lifecycle contract.

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

#### Scenario: Settled Apply limit remains diagnostic rather than blocking

**Given**: A command-capable owner retains `ApplyIterationLimit` for terminal-error change `alpha`
**And**: The persistent scheduler task remains live
**When**: A client reads `/api/v2/state`
**Then**: `alpha.actions.retry_change.allowed` is true when terminal-error evidence otherwise supports retry
**And**: the retained attempts/max and error detail remain observable
**And**: no `apply_iteration_limit_active` blocked reason is projected
**And**: no client must parse the diagnostic to determine retry eligibility

#### Scenario: Snapshot and command admission agree

**Given**: `/api/v2/state` exposes Retry for a settled Apply-limit error
**When**: A client submits `retry_change` using that state revision
**Then**: command admission accepts the same retry classification
**And**: one explicit retry edge is published
**And**: a generic refresh or scheduler notification without that command does not retry the target

#### Scenario: Headless read-only projection remains non-actionable

**Given**: `cflx run` serves `/api/v2` without a bound command executor
**And**: Its shared state retains typed iteration-limit evidence after the run
**When**: A client reads the subsequent snapshot
**Then**: The diagnostic may remain observable
**And**: A submitted command is refused by the existing unbound-runtime lifecycle contract

### Requirement: Shared lifecycle scheduling semantics

Start, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification. `retry_change`, `retry_errors`, and a terminal-error `set_queue_intent=true` alias MUST share the same explicit-retry classification. A settled terminal error that retains typed Apply iteration-limit evidence MUST classify through the ordinary terminal-error retry route; no scheduler-liveness limit guard may refuse it. A command whose candidates are all currently ineligible MUST settle truthfully without notifying or starting a scheduler. For a non-resumable Acceptance deadline hold, shared admission and mark settlement MAY refuse unchanged retry only from the current live owner's typed reducer state. External Acceptance manifests, gate evidence, or diagnostics MUST NOT recreate retry refusal after owner restart or otherwise control queue intent and scheduler dispatch.

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

#### Scenario: V2 retry accepts a settled Apply-limit error

**Given**: The authoritative snapshot exposes `retry_change` for settled terminal-error `alpha`, which retains typed Apply iteration-limit evidence
**And**: The persistent scheduler task remains live
**When**: A client submits `retry_change` or terminal-error `set_queue_intent=true` for `alpha` at the current revision
**Then**: The shared service accepts the ordinary terminal-error retry route exactly once
**And**: One target-specific explicit-retry edge is published
**And**: The later invocation receives fresh Apply budget

#### Scenario: V2 bulk retry includes a settled Apply-limit target

**Given**: `alpha` retains settled Apply iteration-limit evidence and `beta` is ordinarily retryable
**When**: A client submits `retry_errors` for both at the current revision
**Then**: Both targets are retried and dispatched exactly once
**And**: A genuinely unsupported target in the same request remains excluded with its evidence intact

#### Scenario: Retry after run closure starts a later boundary

**Given**: The scheduler boundary that limited `alpha` has completed finish-hook ownership and closed
**When**: A current-revision retry for `alpha` is accepted
**Then**: It cannot notify the closed scheduler
**And**: It may start a new scheduler boundary with workspace-derived state and a fresh budget

<!-- Expected canonical result after archive: shared lifecycle commands keep retry authority in live typed state, external Acceptance cache never owns admission, and restart permits one fresh bounded explicit attempt. -->

#### Scenario: Acceptance deadline hold has terminal operator projection

**Given**: `alpha` exhausted its bounded semantic review deadline and cleanup succeeded
**When**: state, subscriptions, or `cflx client wait` project the result
**Then**: `alpha` has display status `stalled`, blocker kind `none`, `resumable: false`, category `acceptance_review_deadline_exhausted`, and retry is disallowed while the same live owner retains its unchanged typed hold
**And**: `cflx client wait` releases with `change_requires_action` rather than continuing to wait

#### Scenario: Unchanged retry is refused consistently within one owner lifetime

**Given**: the current live owner's typed state retains an unchanged non-resumable Acceptance deadline hold for `alpha`
**When**: `retry_change`, `retry_errors`, Start/F5, terminal-error queue intent, or automatic mark settlement evaluates `alpha`
**Then**: shared admission refuses `alpha` with `unchanged_acceptance_input`
**And**: detail lists the fingerprint components whose repository-visible change can restore eligibility
**And**: no scheduler notification or work dispatch occurs

#### Scenario: Restart does not reconstruct refusal from external state

**Given**: a prior owner persisted Acceptance cache or diagnostics for `alpha`
**And**: the new owner has no live typed refusal state
**When**: an operator explicitly retries `alpha` or automatic mark settlement evaluates it
**Then**: shared admission permits one fresh bounded attempt without requiring repository mutation
**And**: valid external gate evidence may be reused but cannot refuse the command

#### Scenario: Repository change restores live-owner retry eligibility

**Given**: the current owner retains a prior Acceptance deadline hold
**And**: repository evidence now produces a different fingerprint
**When**: state and command admission evaluate the current revision
**Then**: retry is allowed and exactly one target-specific edge may be published
**And**: valid declared gate evidence remains reusable

#### Scenario: Bulk retry reports mixed live eligibility

**Given**: `alpha` has an unchanged live-owner Acceptance hold and `beta` is eligible
**When**: `retry_errors` evaluates both
**Then**: its settled result reports `alpha` with `unchanged_acceptance_input` and admits `beta` exactly once
**And**: neither target's repository evidence is discarded

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

When an existing typed failure, rejection, rejected or parallel-ineligible refresh, dequeue, target-scoped stop, or first `on_merged` hook-recovery event revokes an execution mark, `/api/v2` MUST publish the reconciled `execution_marked` value in the same authoritative state revision as that event's reducer/frontend transition. A `ChangeArchived` transition MUST preserve its target mark in that archive revision. The projection MUST read the shared `ExecutionMarkStore` after pre/post event reconciliation and MUST NOT wait for an unrelated refresh or create a second mark-only revision.

Duplicate or late delivery that changes neither reducer state nor execution marks MUST NOT advance another state revision. Event reconciliation MUST preserve unrelated marks in the same snapshot. A duplicate failure delivered after an explicit re-mark MUST preserve that fresh mark when it creates no new reducer transition. Process-level Stopped transitions MUST retain marked resume targets.

#### Scenario: failure event and cleared mark are coherent

- **GIVEN** `alpha` and `beta` are marked in the authoritative operator snapshot
- **WHEN** a typed event transitions `alpha` into change-level Error
- **THEN** the event revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true
- **AND** no intermediate revision exposes Error with the stale mark

#### Scenario: rejected or ineligible refresh clears mark in its refresh revision

- **GIVEN** `alpha` and `beta` are marked active changes
- **WHEN** one authoritative refresh introduces `alpha` as rejected or parallel-ineligible
- **THEN** that refresh revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true

#### Scenario: archive transition preserves target and unrelated marks

- **GIVEN** `alpha` and `beta` are marked
- **WHEN** the first effective `ChangeArchived(alpha)` transition is projected
- **THEN** that archive revision reports `alpha.execution_marked` as true
- **AND** `beta.execution_marked` remains true
- **AND** later merged or pushed projection continues to retain both marks

#### Scenario: on_merged recovery and cleared mark are coherent

- **GIVEN** a marked change is in active merge handling
- **WHEN** its first `on_merged` hook failure enters reducer merge-wait recovery
- **THEN** that event revision reports the recovery row and `execution_marked: false` together

#### Scenario: duplicate revocation is revision-idempotent

- **GIVEN** the target mark is already false after a revoking event
- **WHEN** duplicate delivery produces no reducer or mark change
- **THEN** `/api/v2` does not advance another state revision
- **AND** unrelated marks remain unchanged

#### Scenario: duplicate failure preserves a fresh re-mark

- **GIVEN** a revoking event cleared a target and an operator explicitly re-marked its non-terminal steady recovery row
- **WHEN** the same failure event is delivered again without a reducer transition
- **THEN** the event revision retains `execution_marked: true`
- **AND** no mark-only correction revision is needed

#### Scenario: process stop retains marked resume targets

- **GIVEN** the snapshot contains execution-marked changes
- **WHEN** a process-level `Stopped` transition is published
- **THEN** the stopped revision retains those `execution_marked` values
- **AND** queue intent and reducer stop reconciliation remain separate from mark ownership

<!-- Expected canonical result after archive: `remote-control-api` will publish archive, merge, and push transitions without revoking lifecycle-independent execution marks. -->

### Requirement: Persistent-scheduler idle is explicit in the operator snapshot

`InstanceSnapshot` MUST include a boolean `persistent_scheduler_idle` field that distinguishes Ready/`app_mode: select` backed by a live persistent-scheduler idle episode from ordinary pre-run Select. The field MUST default to false for a new process and MUST become true in the same authoritative revision where a typed persistent-idle transition performs its guarded Running-to-Ready projection. A late event against Select, Stopping, Error, or Stopped MUST NOT set it. Once true, it MUST become false in the same authoritative projection where an accepted Start commits one or more targets, where typed admitted work begins, or where Error or Stopped ends the episode. An idle-origin graceful-stop request MUST retain it unless accepted Start or admitted work already cleared it.

When shared run control accepts Start with one or more committed targets against persistent-idle Ready, the accepted command outcome's authoritative revision MUST report `app_mode: running`, `persistent_scheduler_idle: false`, and the admitted reducer-derived queue intent. A refused, targetless, stale, or no-op Start MUST leave the idle snapshot unchanged. A subsequent typed persistent-idle transition after a no-work evaluation MUST restore `app_mode: select` and `persistent_scheduler_idle: true` in one revision.

The field remains process-local presentation state, resets on restart, and MUST NOT authorize commands or influence workspace-derived workflow routing. Shared run control MUST independently validate scheduler liveness. The generated OpenAPI schema MUST include the field. A client that replaces local state after a replay gap MUST be able to derive idle Ready lifecycle controls from the snapshot without replaying prior events or parsing logs.

`GET /api/v2/execution-status` MUST keep scheduler liveness, application mode, and actual active work distinct: accepted Start, `app_mode: running`, queue intent, and execution marks alone MUST NOT set `has_active_work` or create a current phase; typed process-activity or lifecycle events remain required.

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
- **AND** no accepted Start outcome already cleared it
- **WHEN** a typed workspace or base-lane work-start event projects Running
- **THEN** the same resulting revision reports `app_mode: running`
- **AND** `persistent_scheduler_idle` is false

#### Scenario: idle-origin graceful stop retains episode identity

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **WHEN** graceful stop is accepted without a preceding accepted Start
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

#### Scenario: Accepted idle Start publishes one coherent Running snapshot

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **AND** the same live scheduler accepts Start for an eligible marked target
- **WHEN** the accepted command outcome is projected
- **THEN** its result revision reports `app_mode: running`
- **AND** reports `persistent_scheduler_idle: false`
- **AND** reports reducer-derived queue intent for the admitted target
- **AND** no replacement scheduler is spawned

#### Scenario: Refused Start preserves idle snapshot

- **GIVEN** the snapshot reports persistent-idle Ready
- **WHEN** Start is refused because no target is eligible or the scheduler is no longer live
- **THEN** no accepted-outcome revision projects Running
- **AND** the latest snapshot remains `app_mode: select` with `persistent_scheduler_idle: true`

#### Scenario: Start feedback remains distinct from active work

- **GIVEN** an accepted Start revision reports `app_mode: running`
- **AND** no dependency-analysis or lifecycle start event has been dispatched
- **WHEN** a client reads `GET /api/v2/execution-status`
- **THEN** `scheduler_running` reflects the existing scheduler
- **AND** `has_active_work` remains false
- **AND** no current phase is inferred from mode, marks, queue intent, or command success

#### Scenario: No-work park restores idle snapshot

- **GIVEN** accepted Start projected Running and woke the existing scheduler
- **WHEN** no execution is admitted and the scheduler emits its next persistent-idle transition
- **THEN** the resulting revision reports `app_mode: select`
- **AND** reports `persistent_scheduler_idle: true`
- **AND** duplicate/no-op idle observation creates no additional state revision

<!-- Expected canonical result after archive: snapshots acknowledge accepted persistent-idle Start immediately while preserving replay recovery, generated contract ownership, non-Start admitted-work clearing, stop races, and exact separation from active-work evidence. -->

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

### Requirement: Client observation does not alter API semantics

Serving the local client MUST reuse the existing `/api/v2` router, DTOs, optimistic revision rules, idempotent command records, and shared operator application transaction. The API MUST NOT add a second orchestration path, client-specific hidden mutation, or durable run record solely for `cflx client`.

#### Scenario: Client controls use ordinary typed commands

**Given**: the client changes marks or explicitly requests Start, Stop, or ForceStop
**When**: it submits a mutation to the owner
**Then**: every mutation appears as an ordinary v2 command record
**And**: each command uses the same shared operator intent as the equivalent TUI control
**And**: mark/unmark never submits queue intent or a lifecycle command
**And**: TUI and API projections observe the same shared operator outcomes

#### Scenario: Status and wait are read only

**Given**: a client invokes client status or wait
**When**: it reads snapshots, execution status, events, and repository evidence
**Then**: no v2 command record is created by those operations
**And**: no process-local mark, queue, scheduler, resolver, cancellation, or mode state changes because of observation

#### Scenario: Proposal subscriptions are observability only

**Given**: a client sets, reads, or clears proposal subscriptions
**When**: the subscription operation settles
**Then**: no workflow command record is created
**And**: state revision, marks, queue intent, scheduler state, mode, and workflow routing remain unchanged

### Requirement: Owner-scoped execution identity

The owner MUST assign one random process-local `execution_id` whenever any admission source moves a change from non-admitted into queued or active work. Admission sources include the shared TUI/API Start transaction, scheduler-owned mark settlement, explicit lower-level queue control, and retry. The ID MUST bind to the current `instance_id` and proposal ID and MUST be visible in execution status. A mark write alone MUST NOT create or claim an execution ID.

Typed terminal settlement or dequeue ends the episode. Dequeue followed by admission or a later retry MUST receive a new execution ID; iterations within one admitted run MUST retain the same ID. Concurrent observations of one already-admitted episode MUST report the same current ID.

Execution identity is observability-only. It MUST NOT authorize commands or affect scheduler eligibility, workflow routing, acceptance, archive, merge, retry, or completion classification. Owner restart MUST invalidate every prior execution binding rather than silently rebinding it.

#### Scenario: Every admission source creates an identity

- **GIVEN** proposal `alpha` is not admitted
- **WHEN** a shared Start transaction, scheduler settlement, explicit queue control, or retry successfully admits it
- **THEN** the owner creates one execution ID for that episode
- **AND** later observations report that same ID

#### Scenario: Mark write creates no identity

- **GIVEN** proposal `alpha` is not admitted
- **WHEN** TUI or client marks it
- **THEN** no execution ID is created by the mark write
- **AND** a later owner-side admission creates the ID

#### Scenario: Retry receives a new execution identity

- **GIVEN** execution `exec-a` for proposal `alpha` reached terminal settlement or was dequeued
- **WHEN** the owner later admits `alpha` again
- **THEN** it creates execution `exec-b`
- **AND** `exec-b` differs from `exec-a`
- **AND** execution-scoped delivery for `exec-a` cannot observe or control `exec-b`

#### Scenario: Restart invalidates process-local identity

- **GIVEN** an execution binding belongs to owner instance `owner-a`
- **WHEN** the owner restarts as `owner-b`
- **THEN** later operations using the old binding fail with typed `owner_restarted`
- **AND** no delivery from the lost process is promised
- **AND** workspace-derived workflow routing is unchanged

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

For each delivery the owner MUST create a versioned bounded event file and provide only fixed metadata through `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Payloads MUST exclude prompts, terminal screen contents, environment dumps, credential values, and unrestricted error bodies. The event file MUST be created inside an owner-private directory with owner-read-only permissions (`0400` inside a `0700` directory), so an ordinary callback cannot open it for writing. The owner MUST NOT re-read or trust the event file after writing it, MUST remove it only after its callback is reaped, and MUST remove it on owner shutdown only after every callback is positively acknowledged as reaped. A callback runs under the owner's UID and can defeat file permissions; this is default mutation refusal, not an integrity guarantee against a hostile callback, and no owner decision may depend on the file contents. An event artifact MUST NOT be overwritten or removed while a different callback still holds it.

Callback runtime and stdout/stderr capture MUST be bounded during collection, not merely truncated after collection, and the owner MUST continue draining both streams past the retention limit so a callback is never blocked by a full pipe. Spawn failure, timeout, non-zero exit, malformed callback behavior, and output overflow MUST produce bounded diagnostics only. Output overflow alone MUST NOT terminate a callback. Timeout and shutdown cancellation MUST terminate and explicitly reap the callback. One terminal delivery attempt is permitted per execution; failures MUST NOT retry forever, alter orchestration state, roll back completion, or change the repository-verifiable result.

Graceful owner shutdown MUST stop admission and apply one finite shutdown deadline across all queued or running callbacks. Delivery MUST remain serialized. Shutdown MUST start no new delivery and create or recreate no event directory or artifact after it begins. When the deadline expires, the owner MUST cancel unfinished delivery and MUST wait for dispatcher acknowledgement that every active callback has been terminated and reaped before event artifact cleanup and registry destruction. Only positive acknowledgement MUST authorize cleanup. A secondary timeout, task-send failure, or acknowledgement sender drop MUST retain the owner-private directory and artifacts while a callback may remain alive, and MUST emit a bounded path-only diagnostic.

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
- **WHEN** graceful shutdown receives the dropped acknowledgement
- **THEN** the owner-private directory and artifact remain
- **AND** no code path or drop-cleanup type other than positive acknowledgement performs removal
- **AND** a bounded warning identifies only the retained directory path

### Requirement: Proposal-scoped explicit subscriptions

The owner MUST provide process-local proposal-scoped subscriptions separate from workflow commands and execution-scoped sink resources. The wire identifier remains `change_id`; user-facing documentation MAY call the addressed change a proposal. One subscription record is keyed by `change_id` and stores one bounded argv callback plus optional blocked-edge delivery. A request MUST address one through 64 distinct change IDs. Multi-proposal `set` and `clear` MUST be atomic: validation failure for any requested proposal or callback causes no requested mutation. `get` MUST require named IDs and MUST NOT provide an unbounded list-all operation.

A subscription MAY be registered before a proposal is admitted. Whenever that proposal enters a new execution episode, the owner MUST bind the episode to the current proposal subscription. Setting a subscription while the latest episode is admitted and not terminal MUST bind that live episode immediately. Each bound episode MUST independently deliver the first typed terminal classification among `completed`, `failed`, and `stopped`; optional `blocked` delivery remains edge-triggered per episode. Event data and callback environment MUST retain the existing `change_id` / `CFLX_CHANGE_ID` naming and contain the actual `instance_id`, `execution_id`, and `change_id`. No `CFLX_PROPOSAL_ID` alias is added. Re-admission with a new execution ID creates a distinct delivery episode while retaining the proposal subscription until explicit clear or owner exit.

Replacing a subscription MUST apply the new argv and blocked setting to the current undelivered live or terminal episode and to future episodes. Clearing MUST cancel pending delivery for named proposals but MUST NOT terminate a callback process already started. Delivery dedupe is keyed by execution episode and event edge, not subscription generation: replacing, clearing, or clearing then setting MUST NOT redeliver a terminal event already delivered by this owner for that execution. Setting after typed terminal settlement MUST immediately attempt the latest terminal event only when this owner has not already delivered that event. The owner MUST retain at most the latest terminal episode and its delivery history per proposal and discard them on owner exit. Registration before any execution MUST NOT synthesize an execution ID, start work, mutate marks or queue intent, or promise admission. Owner restart invalidates all proposal subscriptions and does not promise delivery from the lost process.

The owner MUST expose authenticated `GET`, `PUT`, and `DELETE /api/v2/proposals/{change_id}/subscription` outside the closed workflow command registry. Every request, inspection included, MUST carry and validate the complete `(instance_id, change_id)` binding; partial binding is refused and a mismatched instance returns typed `owner_restarted`. Capabilities MUST advertise proposal-subscription support, clients reaching an older owner MUST return typed `unsupported_owner`, and generated OpenAPI MUST describe the resources and schemas. Set and clear are accepted only over the owner Unix socket. Get over TCP MAY report presence, current execution state, and delivery history but MUST omit argv; argv is returned only over the Unix socket. Subscription operations create no workflow command record and do not advance `state_revision`.

Callback argv MUST be executed directly without shell interpretation and MUST retain existing completion-sink limits, scrubbed environment, private artifact ownership, bounded output and duration, delivery dedupe, failure isolation, and secret-free diagnostics. Callback success or failure MUST NOT alter workflow outcome.

#### Scenario: Multiple proposal subscriptions are set atomically

- **GIVEN** proposals `alpha` and `beta` are visible and callback argv is valid
- **WHEN** one local client sets subscriptions for both proposals
- **THEN** both subscriptions are installed with the same requested callback contract
- **AND** either all are installed or none are installed
- **AND** no workflow command record or state revision is created

#### Scenario: Subscription precedes execution

- **GIVEN** proposal `alpha` has no execution episode
- **AND** a subscription is registered for `alpha`
- **WHEN** the owner later admits `alpha` as execution `exec-a`
- **THEN** `exec-a` is bound to the subscription
- **AND** terminal or requested blocked notification carries `(instance_id, exec-a, alpha)`

#### Scenario: Re-admission creates a distinct notification episode

- **GIVEN** subscribed proposal `alpha` completed execution `exec-a`
- **WHEN** the owner later admits `alpha` as `exec-b`
- **THEN** the existing proposal subscription binds to `exec-b`
- **AND** delivery dedupe for `exec-a` does not suppress delivery for `exec-b`

#### Scenario: Late subscription delivers latest terminal episode

- **GIVEN** proposal `alpha` most recently settled execution `exec-a` terminally
- **WHEN** a subscription is set after settlement
- **THEN** the owner immediately attempts that terminal delivery once
- **AND** the event carries the retained exact execution binding

#### Scenario: Replacing a subscription does not replay delivered terminal state

- **GIVEN** execution `exec-a` for `alpha` has already delivered its terminal event
- **WHEN** the subscription is replaced with different callback argv
- **THEN** `exec-a` is not delivered again
- **AND** the replacement applies to the next undelivered event or execution episode

#### Scenario: Clear then set does not reset episode dedupe

- **GIVEN** execution `exec-a` for `alpha` has already delivered its terminal event
- **WHEN** the subscription is cleared and then set again before a new execution episode
- **THEN** `exec-a` is not delivered again
- **AND** a later `exec-b` remains independently deliverable

#### Scenario: Clear removes only named proposal subscriptions

- **GIVEN** subscriptions exist for `alpha`, `beta`, and `gamma`
- **WHEN** one request clears `alpha` and `gamma`
- **THEN** those subscriptions are absent and `beta` remains unchanged
- **AND** already-running callback processes are not interpreted as workflow control

#### Scenario: Clear races safely with callback launch

- **GIVEN** a terminal callback for `alpha` is pending or has just started
- **WHEN** the subscription is cleared
- **THEN** pending unstarted delivery is cancelled
- **AND** a callback process already started is allowed to finish under its existing bounds
- **AND** neither outcome changes workflow state

#### Scenario: Subscription registration never resumes an agent

- **GIVEN** an agent explicitly registers a proposal callback
- **WHEN** an event is delivered
- **THEN** Conflux executes only the registered bounded argv
- **AND** Conflux does not infer, start, or resume an agent or messaging session
- **AND** any later agent action is external and explicit

#### Scenario: TCP cannot mutate proposal subscriptions

- **GIVEN** an authenticated TCP client can read owner resources
- **WHEN** it attempts to set or clear proposal subscriptions
- **THEN** the owner returns a typed transport refusal
- **AND** no callback argv is stored

#### Scenario: TCP inspection does not disclose subscription argv

- **GIVEN** an authenticated TCP client inspects an existing proposal subscription
- **WHEN** the owner returns subscription status
- **THEN** presence, execution state, and delivery history MAY be returned
- **AND** callback argv is omitted

#### Scenario: Owner restart invalidates proposal subscriptions

- **GIVEN** proposal subscriptions belong to owner instance `owner-a`
- **WHEN** that owner exits and a new owner starts as `owner-b`
- **THEN** the old subscriptions are absent
- **AND** no delivery from `owner-a` is promised

### Requirement: Target-scoped force-stop API and clients

The v2 command registry MUST accept `force_stop_change` with exactly one non-empty `change_id`, ordinary expected-revision fencing, and idempotency identity. Each projected change MUST publish `actions.force_stop_change` eligibility. A successful settled command result MUST identify the target, cancelled execution ID when one exists, cancelled phase, last completed phase, confirmed termination, and `effects_rolled_back: false`.

The CLI MUST expose `cflx client force-stop-change <change-id> --json` with operation `control_force_stop_change` and successful outcome `stopped`. MCP `cflx_control` MUST accept action `force_stop_change` using the existing `change_ids` array with exactly one non-empty element. Both clients MUST delegate through the ordinary v2 command and shared operator transaction; they MUST NOT invoke process-wide `force_stop`, rewrite unrelated marks, perform PID lookup, or reimplement cancellation policy.

#### Scenario: API addresses one change

- **GIVEN** the owner publishes `force_stop_change` eligibility for `alpha`
- **WHEN** the caller submits `force_stop_change` for `alpha` at the current revision
- **THEN** one ordinary command record is created for `alpha`
- **AND** settlement returns the target-specific termination result
- **AND** no process-wide ForceStop command is created

#### Scenario: Per-change eligibility is authoritative

- **GIVEN** `alpha` can be individually force-stopped and `beta` cannot
- **WHEN** clients inspect state
- **THEN** `alpha.actions.force_stop_change` is allowed
- **AND** `beta.actions.force_stop_change` carries a typed blocked reason

#### Scenario: CLI requires exactly one target

- **WHEN** `cflx client force-stop-change` receives zero or multiple change IDs
- **THEN** it returns a typed usage error before contacting the owner
- **AND** no command record is created

#### Scenario: MCP requires exactly one target

- **WHEN** `cflx_control` action `force_stop_change` receives zero, multiple, duplicate, or blank change IDs
- **THEN** MCP validation refuses the call before mutation
- **AND** no lifecycle command is submitted

#### Scenario: Process-wide force-stop remains distinct

- **WHEN** a caller invokes existing `force_stop`
- **THEN** its process-wide semantics and target-list refusal remain unchanged
- **AND** `force_stop_change` remains the only target-scoped control that bypasses the graceful SIGTERM escalation window
- **AND** `stop_and_dequeue` retains its existing cancel-confirm-dequeue contract

### Requirement: Candidate proposal refresh API and clients

The v2 command registry MUST accept `refresh_proposal` with exactly one non-empty `change_id`, a non-empty `source_ref`, full-length `expected_source` and `expected_head` commit IDs, ordinary expected-revision fencing, idempotency identity, and `deny_unknown_fields`. The preview MUST be the read resource `GET /api/v2/proposals/{change_id}/refresh-preview?source_ref=<ref>`, served on both listeners, creating no command record and advancing no revision, and returning `instance_id`, the observed `state_revision`, `eligible`, a `refusal` reason when any precondition fails, `source_ref`, `source_commit`, `candidate_head`, `path`, `source_blob`, `candidate_blob`, `no_op`, and a unified `patch`. Each projected change MUST publish `actions.refresh_proposal` eligibility with the lifecycle reasons defined by `operator-command-execution`. A settled result of kind `refresh_proposal` MUST carry `outcome` (`refreshed` or `unchanged`), `source_commit`, `source_blob`, `candidate_head_before`, `candidate_head_after`, `candidate_blob_before`, `path`, `effects`, and a bounded hook output tail; a refused command MUST carry the stable reason and the same `effects` object. A lifecycle refusal of the command MUST settle with error code `target_ineligible`; a command-time refusal MUST settle with the distinct error code `refresh_refused`, and its record MUST carry the `refresh_proposal` result so the effects remain readable. Capabilities MUST advertise the command and the preview resource, and the generated OpenAPI document MUST include the command variant, the preview route, the result kind, and the action field.

The surface is additive, and compatibility with owners that predate it MUST follow the same rule as execution sinks and proposal subscriptions: `actions.refresh_proposal` and the `proposal_refresh` capability block MUST decode as absent when an older owner omits them, and absence MUST never be read as `allowed`. Every client operation that does not consult refresh eligibility — `status`, `wait`, `mark`, `unmark`, `start`, `stop`, `force-stop`, `force-stop-change`, and the subscribe group — MUST keep working against such an owner from its unchanged `/api/v2` responses. A refresh preview or apply against an owner whose capabilities do not advertise the surface MUST return typed `unsupported_owner` without requesting the preview route and without creating a command record; a typed `not_found` from an owner that does advertise it MUST remain `change_not_found`. Omitting any pre-existing required member of `actions` or of capabilities MUST still be rejected as an incompatible owner. The server MUST always publish `actions.refresh_proposal`.

The CLI MUST expose `cflx client refresh-proposal <change-id> --source-ref REF` with `--dry-run` (operation `refresh_proposal_preview`, success outcome `previewed`) or `--expected-source COMMIT --expected-head COMMIT` (operation `control_refresh_proposal`, success outcomes `refreshed` and `unchanged`), each with `--json` printing one envelope. Lifecycle refusals MUST map to `target_ineligible`; command-time refusals MUST map to the outcome `refresh_refused` with its own exit status `28` and `detail.reason` plus `detail.effects`. The client MUST delegate through the ordinary v2 command and the preview resource, MUST NOT run Git itself, and MUST NOT expose the command over `cflx client mcp` in this change.

#### Scenario: Preview is a read

- **GIVEN** an eligible candidate
- **WHEN** a client reads `/api/v2/proposals/alpha/refresh-preview?source_ref=develop`
- **THEN** the response carries the preview facts and `eligible: true`
- **AND** no command record exists and the state revision is unchanged

#### Scenario: Apply is an ordinary fenced command

- **GIVEN** a preview reported `source_commit` S and `candidate_head` H
- **WHEN** `refresh_proposal` is submitted at the current revision with `expected_source: S` and `expected_head: H`
- **THEN** one command record is created and settles with kind `refresh_proposal` and `outcome: refreshed`
- **AND** a stale `expected_revision` is refused with the ordinary revision conflict and no Git effect

#### Scenario: Per-change eligibility is authoritative

- **GIVEN** `alpha` is an idle `error` candidate with a worktree and `beta` is `applying`
- **WHEN** clients inspect state
- **THEN** `alpha.actions.refresh_proposal` is allowed
- **AND** `beta.actions.refresh_proposal` carries `target_active`

#### Scenario: CLI validates the invocation before contacting the owner

- **WHEN** `cflx client refresh-proposal alpha` is invoked without `--source-ref`, with `--dry-run` and an expectation together, with only one expectation, or with a non-40-hex expectation
- **THEN** it returns `usage_error` before any owner request
- **AND** no command record is created

#### Scenario: An owner built before the surface stays usable

- **GIVEN** a running owner whose `/api/v2/state` publishes every `actions` member except `refresh_proposal` and whose capabilities carry no `proposal_refresh` block
- **WHEN** a client built with the surface runs `status`, `mark alpha`, and `wait beta` against it
- **THEN** each decodes the owner's unchanged responses and returns `observed`, `marked`, and the row's own classification
- **AND** `refresh-proposal alpha --source-ref develop --dry-run` returns `unsupported_owner` with exit status `26` without requesting the preview route
- **AND** `refresh-proposal alpha --source-ref develop --expected-source S --expected-head H` returns `unsupported_owner` and creates no command record
- **AND** a `state` payload that omits a pre-existing required member such as `actions.force_stop_change` is still refused as `incompatible_owner`

#### Scenario: CLI reports typed refusals and effects

- **GIVEN** an apply whose candidate `pre-commit` hook rejects the commit
- **WHEN** `cflx client refresh-proposal alpha --source-ref develop --expected-source S --expected-head H --json` runs
- **THEN** the envelope outcome is `refresh_refused` with exit status `28`
- **AND** `detail.reason` is `hook_rejected` and `detail.effects` reports `file_written: true, staged: true, committed: false`

#### Scenario: Contract completeness

- **GIVEN** a client captures `cflx openapi`
- **WHEN** it inspects the document
- **THEN** it contains the `refresh_proposal` command variant, the refresh-preview route, the `refresh_proposal` result kind, and `actions.refresh_proposal`

### Requirement: Acceptance decision provenance is published per change

Each projected change MUST carry an additive nullable `acceptance_provenance` object describing the latest Acceptance settlement observed by this owner incarnation, built from typed manifest and verdict facts rather than from prose. It MUST contain `decision_source` (`native_verification` | `reviewer`), `reviewer_started`, a closed `reason_code`, `defect_code`, `verification_id`, `declaration_path`, `candidate_commit`, `proposal_content_id`, `change_input_digest`, `program`, `argument_count`, `env_names`, a closed `remediation` (`fix_declaration` | `satisfy_environment` | `repair_candidate` | `retry` | `refresh_candidate` | `none`), and `candidate_divergence` (`status` `identical` | `diverged` | `unknown`, with `source_ref`, `source_commit`, `source_blob`, `candidate_blob`, `reason`). It MUST NOT contain command argument values, environment values, prompt text, or process output; `argument_count` and `env_names` stand in for argv. It MUST be `null` for a change with no settlement in this incarnation and after owner restart. Existing fields MUST keep their meaning, and the generated OpenAPI document MUST document the field and every token. The field is observability only: no command admission, retry classification, archive, merge, or next-action decision MAY read it.

#### Scenario: Native pre-reviewer stop is attributable

- **GIVEN** Acceptance settled on a defective required declaration before any reviewer started
- **WHEN** a client reads the change row
- **THEN** `acceptance_provenance.decision_source` is `native_verification`, `reviewer_started` is `false`, `reason_code` is `declaration_invalid`, `defect_code` is the planner's code, and `verification_id` and `declaration_path` name the declaration
- **AND** `candidate_commit` equals the manifest candidate and `proposal_content_id` equals the blob of the candidate `proposal.md` at that commit

#### Scenario: Reviewer verdict is distinctly labeled

- **GIVEN** a reviewer subprocess ran and returned `fail`
- **WHEN** a client reads the change row
- **THEN** `decision_source` is `reviewer`, `reviewer_started` is `true`, `reason_code` is `reviewer_fail`, `verification_id` is `null`, and `remediation` is `repair_candidate`

#### Scenario: Environment and tool absence map to their remediation

- **GIVEN** one settlement on `tool_unavailable` and one on a failed gate with environment bindings
- **WHEN** the rows are read
- **THEN** the first carries `remediation: satisfy_environment` with `program` naming the unresolved token
- **AND** the second carries `repair_candidate`, `argument_count`, and `env_names`, and no argument or environment value appears anywhere in the row

#### Scenario: Divergence is explicit or unknown, never guessed

- **GIVEN** the manifest bound a review base whose `proposal.md` differs from the candidate's
- **WHEN** the row is read
- **THEN** `candidate_divergence.status` is `diverged` with both blob IDs, the base ref, and the base commit
- **AND** a `declaration_invalid` settlement on such a candidate recommends `refresh_candidate`
- **AND** without a manifest, with an unresolvable base, or with a missing blob the status is `unknown` with a stated reason

#### Scenario: Provenance is not authority

- **GIVEN** two owner states that differ only in `acceptance_provenance`, and a state root that has been deleted
- **WHEN** retry eligibility, admission, and the next action are computed
- **THEN** all three are identical
- **AND** restart publishes `null` without changing the next action

### Requirement: Configured worktree command sessions API

V2 MUST extend the closed command set with `run_worktree_command`
(target opaque `worktree_id`, empty params), `create_session_worktree`
(empty params), and `stop_worktree_session` (target opaque `session_id`,
empty params), and MUST expose
`GET /api/v2/worktree-sessions` and `GET /api/v2/worktree-sessions/{session_id}`.
The command executed MUST be exactly the configured `worktree_command`
template with `{workspace_dir}` and `{repo_root}` expanded server-side; no
request MUST carry a command, argv, environment, path, branch, or base
commit. Launch MUST use the same runner path as the TUI: `sh -c`, `stdin`
from `/dev/null`, an owned process group, stagger admission, and the
configured retry policy inside one session.

`create_session_worktree` MUST create a worktree on a server-generated
`ws-session-<random>` branch at base `HEAD` under the workspace base
directory, MUST validate the cwd after creation, run `.wt/setup` from the
project root, validate after setup and immediately before launch, and MUST
remove the worktree through the same cleanup the TUI uses when setup fails
(forced worktree removal with teardown skipped; the `ws-session-*` branch is
retained exactly as the TUI leaves it), launching nothing and settling
`worktree_session_setup_failed` with the typed cleanup outcome. The creation
steps MUST run under the repository root mutation guard. The running-slot
and worktree-busy reservation MUST be taken before any Git effect and
released on every refusal, so a `capacity` refusal creates no worktree and
no branch.

Each session MUST have a process-local 128-bit `session_id`, a `kind`
(`existing` or `session`), the opaque `worktree_id`, a repository-relative
path, a closed `state` (`running`, `succeeded`, `failed`, `stopped`),
`attempt`, start/finish instants, exit code or signal, setup and cleanup
facts, and an output tail bounded at 65,536 bytes of `sanitize_detail`-ed
lines with a dropped-byte count. Output MUST NOT enter the log ring, the
event stream, or the snapshot. A replay of an identical idempotency key MUST
return the original record and launch nothing; a new request for a worktree
with a running session MUST be refused `worktree_session_busy`; more than 4
concurrently running sessions MUST be refused `worktree_session_busy` with
reason `capacity`. A launch command MUST be refused
`worktree_session_unavailable` when `worktree_command` is not configured or
the project is not a Git repository, and `command_executor_unbound` on a
headless owner.

`stop_worktree_session` MUST cancel the session through the runner's own
handle (`StreamingChildHandle::terminate`), whose cancellation path sends
SIGTERM to the session's process group, escalates to SIGKILL after 5
seconds, verifies the group, and admits no further attempt; it MUST NOT
signal the process group externally, because the configured retry policy
would relaunch inside the same session, and MUST settle only after the
leader is reaped and the cleanup report is read, with `terminated` and
`state_after`. The runner MUST be bound to a `RunCommandScope` owned by the
session registry, and owner shutdown MUST close that scope under a bounded
deadline so every running session is terminated and reaped and no further
attempt can spawn. Scheduler `stop`, `stop_and_dequeue`, and `force_stop`
MUST NOT terminate these independent sessions or close their registry scope;
session stop and owner process shutdown are separate operations.
The guarantee covers the processes that remain in the
session's process group; a descendant that left it (`setsid`,
daemonization) is explicitly outside the stop and shutdown guarantee. The shared worktree service MUST refuse deleting a worktree whose
session is running, for the TUI and the API alike. Sessions MUST be retained
in memory only (running never evicted, at most 32 finished), MUST NOT be
persisted, and MUST NOT influence scheduling, resume, acceptance, or archive.
No owner-generated field (path, branch, result, error detail, events, the
log ring, the snapshot, or the OpenAPI document) MUST disclose the template,
the expanded command, environment values, absolute paths, or credentials.
The output tail is the command's own stdout/stderr: it MUST be retained only
after control-sequence stripping and size bounding, MUST NOT be redacted or
claimed to be, MUST be readable only through the bearer-authenticated
session resource, and MUST never appear as an event, log entry, or snapshot
field. A configured command that prints its arguments, environment, or cwd
discloses them to every authenticated reader; that disclosure is the
operator's responsibility when configuring `worktree_command`, and the
documentation MUST say so.

Capabilities MUST advertise `worktree_sessions` (`available`, `commands`,
`routes`, `max_running`, `max_retained`, `max_output_bytes`, `stdin`,
`reasons`), decoding as unavailable when an older owner omits it; the
generated OpenAPI document MUST include the three commands, both routes, the
`worktree_session` and `worktree_session_stop` result kinds, the
`worktree_session_started`/`worktree_session_finished` event types, and the
error codes. Every existing route and command MUST remain unchanged on an
older owner, and a new command submitted to it MUST fail typed validation
before any effect. `cflx client` and `cflx client mcp` MUST gain no verb.

#### Scenario: Configured command runs in the addressed worktree with null stdin

- **GIVEN** `worktree_command` is configured and a worktree has an opaque ID
- **WHEN** an authenticated client submits `run_worktree_command` for it
- **THEN** one session starts with cwd equal to that worktree and both placeholders expanded
- **AND** the child's stdin is `/dev/null`
- **AND** the session resource publishes exit code, state, and the bounded output tail
- **AND** one `worktree_session_finished` event is published

#### Scenario: Session worktree is created the way the TUI creates it

- **GIVEN** `.wt/setup` exists in the project root
- **WHEN** a client submits `create_session_worktree`
- **THEN** a `ws-session-*` branch and registered worktree exist at base `HEAD`
- **AND** setup ran and the cwd was validated after create, after setup, and before launch
- **AND** the result names the new opaque `worktree_id`

#### Scenario: Setup failure cleans up and launches nothing

- **GIVEN** `.wt/setup` exits non-zero
- **WHEN** `create_session_worktree` runs
- **THEN** the worktree is removed by forced removal without teardown and the `ws-session-*` branch is retained
- **AND** no process is launched
- **AND** the record fails `worktree_session_setup_failed` with `cleanup.removed: true`

#### Scenario: Capacity refusal of creation leaves the repository untouched

- **GIVEN** 4 sessions are running
- **WHEN** a client submits `create_session_worktree`
- **THEN** it is refused `worktree_session_busy` with reason `capacity` before any Git effect
- **AND** no worktree or branch is created and the reservation is released

#### Scenario: Resend and concurrency never duplicate a session

- **GIVEN** a session is running for a worktree
- **WHEN** the same idempotency key is replayed and a new key is submitted
- **THEN** the replay returns the original record and launches nothing
- **AND** the new key is refused `worktree_session_busy`
- **AND** a configured retry inside the session increments `attempt` without a new `session_id`

#### Scenario: Stop terminates the whole process group

- **GIVEN** a running session whose command spawned a background child in the same process group, and a retry policy of at least one retry
- **WHEN** `stop_worktree_session` is submitted
- **THEN** both processes are gone after settlement and `terminated` is true
- **AND** the session reads `stopped` with its signal, `attempt` stays 1, and no new process was started

#### Scenario: Deletion is refused while a session runs

- **GIVEN** a worktree with a running session
- **WHEN** the API submits `delete_worktree` or the TUI confirms deletion
- **THEN** the shared service refuses with a typed ineligible reason
- **AND** nothing is torn down or removed

#### Scenario: Output is bounded and owner metadata discloses no secret

- **GIVEN** a template containing a credential and a command that prints 200 KiB with ANSI codes but never echoes its arguments, environment, or cwd
- **WHEN** the session finishes
- **THEN** the retained tail is at most 65,536 bytes with `truncated_bytes` counted and no escape bytes
- **AND** no owner-generated field, event, log entry, snapshot field, or record contains the credential, the template, or an absolute path

#### Scenario: Output tail is the command's own data

- **GIVEN** a configured command that echoes the expanded `{repo_root}`
- **WHEN** an authenticated client reads the session resource
- **THEN** the absolute path appears in `output.lines` exactly as printed, control sequences stripped and not redacted
- **AND** it appears in no owner-generated field, event, log entry, or snapshot field, and an unauthenticated read is refused

#### Scenario: Older owner stays compatible

- **GIVEN** an owner built before this surface
- **WHEN** a client decodes its capabilities and submits `run_worktree_command`
- **THEN** `worktree_sessions` decodes as unavailable
- **AND** the command is `validation_failed` with no record and no effect

### Requirement: Archive completion and execution clocks are published per revision

Each projected `ChangeResource` MUST publish `archive_complete`, `true`
exactly when the reducer's archive record contains the change in this owner
process, read in the same reducer hold as `display_status`, distinct from
`display_status` and from merge or push success, retained while the row
advances to `resolving`, `resolve pending`, `merge wait`, `merged`, or
`pushed`, and `false` in a new process. Each `ChangeResource` MUST publish
`net_active` with `accumulated_ms` (the sum of closed active intervals),
`active_since` (the RFC 3339 instant of the open interval or `null`), and
`has_history`, driven by one shared clock the TUI rows also use: an interval
opens on a transition into an active display status (`preparing`,
`applying`, `accepting`, `rejecting`, `archiving`, `resolving`), stays open
across repeated or successive active statuses, closes exactly once on a
transition to any other status or on process `Stopped` in either order with
the reducer synchronization, resumes on the next active status with the
earlier total retained, is never reset while the owner runs, and is zero
after restart.

The snapshot MUST publish `run_clock` with `started_at`, `ended_at`, and
`elapsed_ms` as a projection of the existing TUI run clock through one
shared rule, boundary for boundary and without changing the TUI: opened
(`started_at` set, the other two `null`) by an accepted Start dispatch
while the mode is not running; left unchanged by admitted work resuming a
persistent-idle Ready, by `ResolveReserved`, by a persistent-idle park, by a
stop request, by a cancelled stop, by a Start while already running, and by
a fatal process `Error`; frozen, with `ended_at` set and `elapsed_ms` the
monotonic duration since the open in whole milliseconds, by every process
`Stopped` delivery, by every `AllCompleted` delivery whether or not it may
overwrite the mode, and by a resolve-side settlement that finds no
processing row while running, except that `ResolveFailed` for an already
`merged` target MUST be a no-op just as the TUI early-return path is;
re-frozen at the later instant by a repeated
terminal delivery, exactly when the TUI re-freezes; and retained until the
next opening Start. All three members MUST be `null` before the first Start
in the process. Run-clock parity is scoped to event sequences in which the
existing web and TUI modes agree; existing mode-divergence cases are not
redefined or claimed to have universal parity by this metadata extension.

`ChangeTiming` MUST keep its existing per-run meaning: `elapsed_ms` is the
duration of the finished run and is not cumulative. Accumulated time and
the open boundary MUST be published in the same snapshot and revision; no
member MUST change because time passed, a repeated identical status event
MUST NOT change `net_active`, and `run_clock` MUST change only on a real
open or freeze event, so the revision MAY advance on a repeated terminal
event only when the frozen value changes. None of the three members MUST be
persisted, derived from process start time or logs, or read by scheduling,
resume, acceptance, or archive decisions.

The three members MUST be published as required, non-null members of the
generated OpenAPI document and MUST decode as absent (`null`) — never as
`false` or zero — when an older owner omits them, while a payload omitting
any pre-existing required member MUST still be rejected. `cflx client`
verbs MUST keep working against both owners. No new capability block,
route, or command is introduced.

#### Scenario: Archive completion is published independently of status

- **GIVEN** the reducer recorded `ChangeArchived` for `alpha` and the row advanced to `resolving`
- **WHEN** a client reads `/api/v2/state`
- **THEN** `alpha.archive_complete` is `true` while `display_status` is `resolving`
- **AND** a fresh-process retry showing `resolving` with no archive on record reports `false`

#### Scenario: Net-active clock equals the TUI row

- **GIVEN** one event sequence of apply, acceptance, stop, retry, and archive
- **WHEN** the TUI row and the API projection observe it with the same instants
- **THEN** `accumulated_ms`, the open boundary, and `has_history` are equal at every step
- **AND** the stop event and the reducer synchronization close the interval exactly once in either order

#### Scenario: Run clock is retained after stop and reset by the next start

- **GIVEN** a run was started and then stopped
- **WHEN** clients read the snapshot repeatedly and later submit `start`
- **THEN** `run_clock.elapsed_ms` is constant across the reads
- **AND** the next start publishes a new `started_at` with `ended_at` and `elapsed_ms` null
- **AND** a persistent-idle park and a fatal process `Error` leave the clock open

#### Scenario: Run clock keeps the TUI start across re-admission and ResolveReserved

- **GIVEN** a run opened at `t0` was parked by a persistent-idle event, or stopped
- **WHEN** admitted work resumes it, or `ResolveReserved` moves the mode to running
- **THEN** `run_clock.started_at` is still the `t0` sample
- **AND** the TUI `orchestration_started_at` driven at the same instants is still `t0`

#### Scenario: Run clock re-freezes on a repeated terminal event like the TUI

- **GIVEN** a run opened at `t0` was frozen by `Stopped` at `t1`
- **WHEN** a second `Stopped` arrives at `t2`, or a delayed `AllCompleted` arrives at `t3`
- **THEN** `run_clock.elapsed_ms` equals the TUI `orchestration_elapsed` after the same event (`t2 - t0` or `t3 - t0`)
- **AND** `ended_at` moves to the later sample and the revision advances on that event, not on any read

#### Scenario: Time passing does not advance the revision

- **GIVEN** an active change and no new event
- **WHEN** the snapshot is read twice at later instants
- **THEN** both reads return the same stored snapshot with the same `state_revision`
- **AND** `ChangeTiming` boundaries are unchanged from before this contract

#### Scenario: Older owner reads as unpublished

- **GIVEN** a `state` payload from an owner that predates these members
- **WHEN** a client decodes it
- **THEN** `archive_complete`, `net_active`, and `run_clock` are absent, not `false` or zero
- **AND** `status`, `wait`, and `mark` behave as before
- **AND** a payload missing `actions.force_stop_change` is still rejected

### Requirement: Abort-on-conflict remote merge API

V2 MUST extend the closed command set with `merge_worktree_abort_on_conflict`
(target opaque `worktree_id`, empty params, `deny_unknown_fields`, ordinary
revision fencing, idempotency, and the application gate) that runs the
shared base merge under `AbortOnConflict`. `merge_worktree` MUST remain
unchanged with `PreserveConflict`, its `merge_conflict` refusal, retained
intermediate state, and `local_or_tui_required` recovery.

A conflict predicted by the fresh targeted inspection MUST be refused as
`merge_conflict_predicted` before any Git mutation, with the conflict files
in the typed result. A real conflict MUST run `git merge --abort`, record its
exit status, and verify restoration from the repository — `HEAD` equals the
pre-merge commit, no `MERGE_HEAD` exists, and the root working directory is
clean under the read-only status policy — reporting `restored` as
`confirmed` only when all three hold, `unknown` when any verification could
not be read, and `failed` otherwise. A confirmed restoration MUST settle
`merge_conflict_aborted`; a failed or unknown restoration MUST settle
`merge_abort_failed` with `recovery: local_or_tui_required`, MUST NOT claim
that state was preserved, and MUST leave later incompatible mutations
refused as `root_busy` while `MERGE_HEAD` remains. The owner MUST NOT retry
the abort or run any further compensation. `on_merged` MUST run only after a
completed merge, exactly once. After a confirmed abort a following merge of
a mergeable branch MUST be admissible.

The result kind `merge_abort` MUST carry `outcome`, `premerge_head`,
`conflict_files`, `abort` (`exit_ok`, `head_after`, `merge_head_present`,
`clean_after`, `restored`, `detail`), and `recovery`, and MUST be present on
success and on every refusal after eligibility. The same verified report
MUST reach the TUI merge path: the operator-facing text MUST state that the
merge was aborted and the base restored only when restoration was confirmed,
and MUST state that the abort failed or restoration is unconfirmed
otherwise. Capabilities MUST advertise `worktrees.merge_abort` (`available`,
`command`, `predicted_conflict_refused`, `restore_verification`, `reasons`),
MUST keep `merge_conflict_preserves_state: true` and
`merge_conflict_recovery` with descriptions narrowed to `merge_worktree`,
and MUST decode `merge_abort` as unavailable when an older owner omits it.
The generated OpenAPI document MUST include the command variant, the result
kind, and the three error codes. `cflx client` and `cflx client mcp` MUST
gain no verb.

#### Scenario: Clean branch merges and runs on_merged once

- **GIVEN** a worktree branch that merges cleanly into base
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** base contains a `--no-ff` merge commit and `on_merged` ran exactly once
- **AND** the record settles `succeeded` with `outcome: merged`

#### Scenario: Predicted conflict is refused before any mutation

- **GIVEN** the fresh inspection reports conflict files for the branch
- **WHEN** the abort-on-conflict command is submitted
- **THEN** it fails `merge_conflict_predicted` naming those files
- **AND** base `HEAD` is unchanged and no `MERGE_HEAD` exists

#### Scenario: Real conflict is aborted and restoration is verified

- **GIVEN** a merge that conflicts at execution time
- **WHEN** the owner runs `git merge --abort` and it succeeds
- **THEN** `HEAD` equals `premerge_head`, `MERGE_HEAD` is absent, the root is clean
- **AND** the record fails `merge_conflict_aborted` with `restored: confirmed`
- **AND** `on_merged` did not run and a following clean merge succeeds

#### Scenario: Failed abort is reported, not assumed

- **GIVEN** `git merge --abort` exits non-zero
- **WHEN** the owner verifies restoration
- **THEN** the record fails `merge_abort_failed` with `exit_ok: false`, `restored: failed`, and `recovery: local_or_tui_required`
- **AND** the TUI text says the abort failed rather than "Merge aborted"
- **AND** a later incompatible mutation is `root_busy` until local recovery

#### Scenario: Default merge and older owners are unchanged

- **GIVEN** an older owner and a current owner
- **WHEN** `merge_worktree` conflicts on either, and `merge_worktree_abort_on_conflict` is submitted to the older owner
- **THEN** `merge_worktree` fails `merge_conflict` and retains the intermediate state on both
- **AND** the new command is `validation_failed` on the older owner with no effect
- **AND** `merge_conflict_preserves_state` is `true` on both and `merge_abort` decodes as unavailable from the older owner

### Requirement: Confirmed destructive worktree deletion API

V2 MUST expose `GET /api/v2/worktrees/{worktree_id}/delete-preview` as a
read (no command record, no revision change, both listeners) that performs a
fresh targeted observation and returns `observed_at` (top level, outside
`consent`), `branch`, `head`, `is_main`, `dirty`, `has_commits_ahead`,
`inspection`, `content_observation` (`observed`, `status_failed`,
`unbounded`, `nested_repository`), `has_ignored_content`, `ignored_entries`,
`teardown_script_present` (true exactly when the teardown runner would
execute the script: a regular file with an execute bit), per-option
eligibility with a typed `blocked_reason` for `ordinary`, `skip_teardown`,
`discard_known_dirty`, and `discard_commits_ahead` (with
`also_discards_dirty`), and a `consent` object holding exactly `branch`,
`head`, `dirty`, `has_commits_ahead`, `has_ignored_content`, `merge_base`,
`discardable_content_id`, and `ignored_content_id`, which the command MUST
accept verbatim.

Both identities MUST come from one bounded read-only oracle over the sorted
records of `git status --porcelain=v2 -z --untracked-files=normal
--ignored=traditional` combined with the oracle's own fd-relative walk of
every directory record Git collapses (`? dir/` and `! dir/`), so that the
bound set is the on-disk content `git worktree remove --force` deletes
beyond `HEAD`. `discardable_content_id` MUST be `sha256:` over the staged,
unstaged, unmerged, and untracked records combined with the no-follow
on-disk bytes (or symlink target string, or deletion marker) of every path
with a worktree-side difference and of every entry walked beneath an
untracked directory, and MUST change whenever any such byte changes.
`ignored_content_id` MUST be `sha256:` over every ignored path's file type,
size, mtime, mode, and the no-follow bytes of the file itself (or symlink
target string), including every file, symlink, special file, and empty
subdirectory walked beneath a collapsed ignored directory, and MUST change
whenever an ignored path is added, modified, or removed at any depth,
including a same-size byte change with mtime and mode restored; metadata
MUST be bound in addition to bytes, never instead of them. `dirty` keeps its
`--ignored=no` meaning. Every path MUST be resolved component by component
from an open worktree root descriptor with `O_NOFOLLOW`, files MUST be
opened `O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC` and re-checked with `fstat` so that
a FIFO or a path swapped after `fstatat` is recorded by type without being
read or followed, and the walk MUST NOT traverse a symlink. The oracle MUST
have one documented combined budget over tracked, untracked, and ignored
content with each path counted once (500,000 records including walked
entries, 256 MiB of hashed bytes) beyond which the observation is
`unbounded` with no partial, prefix, or metadata-only identity and no
exclusion of ignored content, MUST report an actual `.git` descendant at any
depth or a status-reported submodule as `nested_repository` while a
directory record itself is walked rather than refused, MUST publish both
identities and `has_ignored_content` as `null` with the typed
`content_observation` whenever it is not `observed`, and MUST be computed
through the read-only status policy without writing, staging, running
hooks, or excluding any path. A worktree whose observation is `unbounded`
MUST NOT be deletable through this command; reducing its content remains a
local operation.

V2 MUST extend the closed command set with `delete_worktree_confirmed`
(target opaque `worktree_id`) whose typed params are the three independent
booleans `skip_teardown`, `discard_known_dirty`, `discard_commits_ahead` and
the echoed `consent`, with `deny_unknown_fields`; either discard permission
without both `discardable_content_id` and `ignored_content_id` (regardless of
`consent.dirty`), `discard_commits_ahead` without `merge_base`, or a
non-40-hex commit MUST be `validation_failed`. Under the repository root
mutation guard the shared service MUST re-observe the target, including the
content observation, and refuse with `delete_consent_stale` (reasons
`branch_changed`, `identity_changed`, `head_moved`, `base_moved`,
`dirty_changed`, `ahead_changed`, `ignored_changed`, `content_changed`) when
any consented fact or either recomputed identity differs, MUST refuse
unknown dirty and an unobservable or unbounded content observation as
`worktree_dirty_unknown`, MUST refuse unknown ahead, the main worktree, and
a `nested_repository` observation as `target_ineligible` whatever
permissions were given, MUST refuse known ignored content without
`discard_known_dirty` as `worktree_dirty` with reason `ignored_content`
even when `dirty` is `false`, MUST refuse a held guard or base merge as
`root_busy`, and MUST apply the unchanged eligibility, teardown, drift,
branch-ref, removal, and branch-cleanup steps with the `DeleteOptions` the
permissions map to. The service MUST re-run the content observation after
teardown and refuse `delete_consent_stale` with `teardown_ran: true` without
removing anything when `discardable_content_id` changed or when the ignored
identity is neither the consented one nor the empty-set identity. Nothing
MUST be removed on any refusal.

The settled result kind `delete_worktree_confirmed` MUST report `teardown`
(`ran`, `skipped`, `absent`, `not_reached`), `dirty_discarded`,
`commits_discarded`, `branch_deleted`, `branch_retained_reason`, and, on
refusal, `refusal.reason`, `refusal.teardown_ran`, and the freshly observed
facts including both identities and `content_observation`. The `delete_worktree` command MUST remain unchanged (empty params,
fail-closed options, existing refusals). Capabilities MUST advertise
`worktrees.confirmed_delete` (`available`, `command`, `preview_route`,
`permissions`, `consent_fields`, `content_id_algorithm`, `reasons`), MUST
keep `delete_requires_teardown: true` with its description narrowed to the
`delete_worktree` command, and MUST decode `confirmed_delete` as unavailable
when an older owner omits it. The generated OpenAPI document MUST include the
command variant, the preview route, the result kind, and the error code.
`cflx client` and `cflx client mcp` MUST gain no verb.

#### Scenario: Preview publishes facts, options, and consent

- **GIVEN** a dirty worktree with commits ahead of base and a `.wt/teardown` script
- **WHEN** an authenticated client reads its `delete-preview`
- **THEN** `dirty`, `has_commits_ahead`, `teardown_script_present`, and each option's eligibility and reason are published
- **AND** `consent.discardable_content_id` and `consent.ignored_content_id` are `sha256:` identities and `observed_at` is outside `consent`
- **AND** the `consent` object submitted verbatim as `params.consent` passes validation
- **AND** no command record exists and the revision is unchanged

#### Scenario: Ignored content is disclosed and needs dirty discard

- **GIVEN** a worktree that `git status --ignored=no` reports clean but which holds an ignored file
- **WHEN** an authenticated client reads its `delete-preview` and then submits `delete_worktree_confirmed` without `discard_known_dirty`
- **THEN** the preview publishes `dirty: false`, `has_ignored_content: true`, `ignored_entries`, and `ordinary` blocked with `ignored_content`
- **AND** the command refuses `worktree_dirty` with reason `ignored_content` and removes nothing
- **AND** modifying the ignored file after the preview makes a `discard_known_dirty: true` submission `delete_consent_stale` with `content_changed`
- **AND** rewriting an ignored file to different bytes of the same size with its mtime and mode restored, or changing a file three levels beneath a collapsed ignored directory, is likewise `delete_consent_stale` with `content_changed`
- **AND** fresh consent with `discard_known_dirty: true` removes the worktree

#### Scenario: Oracle limits and nested repositories fail closed

- **GIVEN** a worktree containing an actual nested repository (untracked, or inside an ignored directory), or one whose combined records or hashed bytes exceed the documented budget, including when ignored build output alone exceeds it
- **WHEN** an authenticated client reads its `delete-preview` and submits `delete_worktree_confirmed` with every permission
- **THEN** the preview publishes `content_observation` as `nested_repository` or `unbounded` with `null` identities and every option ineligible
- **AND** the command refuses `target_ineligible` (reason `nested_repository`) or `worktree_dirty_unknown` (reason `content_unbounded`)
- **AND** no metadata-only or partial identity is published in place of the refusal, and a plain ignored directory without a `.git` descendant is walked rather than refused
- **AND** nothing is torn down or removed and `delete_worktree` behaves as before

#### Scenario: Skip-teardown removes without running the script

- **GIVEN** a clean worktree whose `.wt/teardown` writes a marker
- **WHEN** `delete_worktree_confirmed` is submitted with `skip_teardown: true` and valid consent
- **THEN** the worktree is removed, the marker is absent, the result reports `teardown: skipped`
- **AND** the opaque ID is retired

#### Scenario: Same HEAD and same dirty flag but changed content is stale

- **GIVEN** a preview taken while `README` holds `A` with `dirty: true`
- **WHEN** `README` is changed to `B` without committing and the consent is submitted with `discard_known_dirty: true`
- **THEN** the command fails `delete_consent_stale` with reason `content_changed`
- **AND** the worktree, branch, and ID are intact and `refusal.observed` carries the new identity

#### Scenario: Ahead and dirty requires both permissions

- **GIVEN** a worktree that is dirty and ahead of base
- **WHEN** consent is submitted with `discard_commits_ahead: true` and `discard_known_dirty: false`
- **THEN** the command refuses `worktree_dirty` and removes nothing
- **AND** `discard_commits_ahead: true` with `consent.dirty: false` and no identities is `validation_failed` before any observation
- **AND** with both permissions the worktree is removed, the branch is force-deleted at the consented commit, and the result reports both discards

#### Scenario: Cancel submits nothing

- **GIVEN** a client read the preview
- **WHEN** the operator cancels and no command is submitted
- **THEN** the worktree, its ID, the command registry, and the revision are unchanged

#### Scenario: Teardown failure is recoverable by consented skip

- **GIVEN** a worktree whose `.wt/teardown` exits non-zero
- **WHEN** an ordinary delete fails and a `delete_worktree_confirmed` with `skip_teardown: true` and fresh consent follows
- **THEN** the first fails `internal_error` retaining the worktree and its ID
- **AND** the second removes the worktree and reports `teardown: skipped`

#### Scenario: Unknown, main, and busy fail closed

- **GIVEN** a target whose dirty state cannot be observed, the main worktree, or a held root guard
- **WHEN** `delete_worktree_confirmed` is submitted
- **THEN** the refusals are `worktree_dirty_unknown`, `target_ineligible`, and `root_busy` respectively
- **AND** nothing is torn down or removed

#### Scenario: Ordinary delete and older owners are unchanged

- **GIVEN** an owner built before this surface and a current owner
- **WHEN** `delete_worktree` is submitted with a `skip_teardown` field, and `delete_worktree_confirmed` is submitted to the older owner
- **THEN** both are `validation_failed` with no delegation
- **AND** `delete_requires_teardown` is published as `true` by both owners
- **AND** `confirmed_delete` decodes as unavailable from the older owner

### Requirement: Bounded remote text edit API

V2 MUST expose `GET /api/v2/changes/{change_id}/text?path=<relative>` and
`GET /api/v2/worktrees/{worktree_id}/text?path=<relative>` as reads that
create no command record, advance no revision, and are served on both
listeners and by a headless `cflx run`. It MUST extend the closed command set
with `write_change_text` (target `change_id`) and `write_worktree_text`
(target opaque `worktree_id`), each taking exactly `params.path`,
`params.expected` (`{"kind":"content","content_id":"sha256:<64 hex>"}` or
`{"kind":"absent"}`), and `params.content`, with `deny_unknown_fields`,
ordinary revision fencing, idempotency identity, and the application gate.
There MUST be no unconditional write and no client-supplied root, absolute
path, branch, or Git ref.

The scope root MUST be server-derived: `openspec/changes/<change_id>` in the
project repository for a tracked, non-archived change, or the registered
directory bound to the opaque worktree ID. `path` MUST be at most 1024 UTF-8
bytes and MUST be refused as `validation_failed` when it is empty, absolute,
contains `\`, a NUL, or an empty, `.`, or `..` component. Every directory
component and the leaf MUST be opened with `O_NOFOLLOW` from the root
descriptor on every read and every write. The owner MUST refuse with
`text_not_editable` and a stable reason — `git_metadata` (any `.git`
component, file or directory), `conflux_owned_path` (top-level `.wt/` or the
top-level `.conflux-inflight`), `unsafe_path` (a symlink or non-directory on
the walk or a symlink leaf), `nested_repository` (a traversed directory below
the root that contains a `.git` entry of any type; the root's own `.git` is
not examined), `not_regular_file`, `executable` (any execute bit),
`too_large` (over 1,048,576 bytes), `not_text` (invalid UTF-8 or NUL),
`root_missing` — and with `target_ineligible` and reason `change_active`,
`change_admitted`, `managed_process_live`, or `root_busy` when the owning
change is in an active display status, is admitted (`queue_intent` queued or
display status `queued` / `blocked`), when the owner's managed writer
coverage for the target root is not `Clear`, or when the repository root
mutation guard is held or a base merge or resolve is in progress — the last
applying to the main worktree row and every change directory alike. The read
response MUST publish `editable`, `edit_blocked_reason`, and a bounded
diagnostic `edit_blocked_detail` from the same classifier. A nested worktree
MUST be editable only through its own opaque worktree ID, never through a
path from another root.

Managed writer coverage MUST be a tri-state (`Clear`, `Busy`, `Unknown`)
computed from one process-local ledger, never a boolean, and `Busy` and
`Unknown` MUST both refuse as `managed_process_live`. The ledger MUST account
for every owner-managed writer: every run command scope the owner has
installed, retained until each of its identities is proven quiescent and
never dropped because a later run replaced it, a shutdown took it, or a
quiescence budget expired; every spawn that bypasses the scope, including
the shell command path and the configured worktree session, held as a lease
from before spawn until both direct-child reap and managed process-group /
retained-descendant quiescence are positively confirmed (shell exit alone
MUST NOT release coverage); and spawns with no
working directory, bound to the owner's own canonical working directory.
Each writer's working directory MUST be canonicalized at registration and
compared to the canonical root by component-wise path containment, so a
symlink alias binds the real path and a sibling whose name shares a prefix
is not bound. A failed canonicalization, an unresolved completion/quiescence lease, or an owner with
no ledger attached MUST be `Unknown`; the absence of a current scope, of a
termination port, or of a ledger MUST NOT be read as `Clear`. `Clear` MUST be
provable by an owner — a headless `cflx run` included — whose ledger exists
and is empty. A running lease with known cwd MUST be `Busy` for the bound
root and MUST NOT block unrelated siblings as `Unknown` solely because the
child has not exited. Owner restart MUST NOT be presented as quiescence
evidence or performed to bypass an unknown result. The coverage MUST be read once, as part of the eligibility
snapshot at the start of a write, and the contract MUST state that a writer
admitted after that snapshot and before the rename is in the same window as
an external writer. Reaching `Clear` MUST NOT kill, sweep, or reset any
process or ledger entry, and `force_stop_change`, `owns_managed_process`,
and `kill_managed_process` MUST keep their current target and scope.

`content_id` MUST be `sha256:` followed by the lowercase hex SHA-256 of the
exact file bytes. A write MUST compare the current bytes to `expected`
immediately before replacing them, MUST refuse `text_stale` with the current
`content_id` (or `null` when absent) when they differ or when `absent` was
expected and the file exists, MUST settle `no_op` with `outcome: unchanged`
when the new bytes equal the current bytes, and otherwise MUST write through a
same-directory temporary file, `fsync`, and `rename`, preserving the existing
permission bits (`0644` subject to umask for a created file), removing the
temporary file on every failure path, and reporting a `text_write` result
with `outcome`, `content_id_before`, `content_id_after`, `current_content_id`,
`bytes_written`, `mode`, and `effects.file_written`. A refused record MUST
carry the same result. A successful worktree write MUST trigger the ordinary
worktree observation refresh so `dirty` follows, and MUST NOT stage, commit,
or push. The write itself MUST NOT advance the state revision, but the
observation refresh it triggers MAY, and the contract MUST tell clients to
re-read `state_revision` after a save. A temporary file left by a process
crash between create and rename MUST be reported as ordinary untracked dirt
and MUST NOT be ignored or excluded from any cleanliness check.

The contract MUST state that the compare and the rename are not atomic
against external writers: Conflux's own writers are excluded by eligibility,
and uncooperative external editors are not excluded. Capabilities MUST
advertise `text_edit` (`available`, `read_routes`, `commands`, `max_bytes`,
`max_path_bytes`, `content_id_algorithm`, `excluded`, `reasons`), and the
generated OpenAPI document MUST include both routes, both command variants,
the result kind, and both error codes. The block MUST decode as unavailable
when an older owner omits it, every existing route and command MUST remain
unchanged on such an owner, and a new command submitted to it MUST fail
typed validation before any effect. `cflx client` and `cflx client mcp` MUST
gain no verb.

#### Scenario: Read publishes identity and eligibility

- **GIVEN** a live opaque worktree whose `src/lib.rs` is a regular `0644` file
- **WHEN** an authenticated client reads `/api/v2/worktrees/{id}/text?path=src/lib.rs`
- **THEN** it receives `content`, `content_id` equal to `sha256:` of the bytes, `size_bytes`, `mode: "0644"`, `editable: true`, and the limits
- **AND** the state revision is unchanged and no command record exists

#### Scenario: Matching write replaces atomically and dirties Git

- **GIVEN** a prior read returned identity `C` for a `0600` file
- **WHEN** `write_worktree_text` is submitted with `expected.content_id = C` and new content
- **THEN** the file holds the new bytes with mode `0600`, the record settles `succeeded` with `outcome: written`
- **AND** `git status --porcelain` lists only that path and no commit is created
- **AND** no temporary file remains in the directory

#### Scenario: Stale write refuses and preserves client text

- **GIVEN** the file changed after the client read identity `C`
- **WHEN** a write with `expected.content_id = C` is submitted
- **THEN** the record fails with `text_stale` and `current_content_id` names the current bytes
- **AND** the file bytes and directory listing are unchanged
- **AND** no response asks the client to discard its buffer

#### Scenario: Containment refuses symlinks and Git metadata in both forms

- **GIVEN** a linked worktree whose `.git` is a file and a main worktree whose `.git` is a directory
- **WHEN** a client addresses `.git`, a path through a symlinked directory, a symlink leaf, `.wt/teardown`, an executable, a file over 1 MiB, or a file with NUL bytes
- **THEN** each is refused with `text_not_editable` and its stable reason
- **AND** a leaf replaced by a symlink after a successful read is refused at write time
- **AND** no bytes under either root change

#### Scenario: Active writers exclude remote edits

- **GIVEN** change `alpha` is `applying`, or the owner's writer coverage for `alpha`'s worktree is `Busy` (a registered writer with or without a change ID), or the root guard is held, or a resolve is running
- **WHEN** a client reads and then writes a file in `alpha`'s worktree
- **THEN** the read reports `editable: false` with the matching reason
- **AND** the write fails `target_ineligible` with the same reason and writes nothing
- **AND** a writer bound only to a sibling path leaves the target editable

#### Scenario: Retained writers from a previous run exclude remote edits

- **GIVEN** a previous run's scope holds an identity spawned under `alpha`'s worktree that was never proven quiescent, and a new run has since been started with a fresh scope
- **WHEN** a client reads and then writes a file in `alpha`'s worktree
- **THEN** the read reports `editable: false` with `managed_process_live` and a detail naming the retained identity
- **AND** the write fails `target_ineligible` with the same reason and writes nothing
- **AND** a sibling worktree stays editable, nothing is killed, and `actions.force_stop_change` is unchanged
- **AND** once the identity is released with confirmed quiescence the same read reports `editable: true`

#### Scenario: Unknown writer coverage fails closed

- **GIVEN** an owner with no writer ledger attached to its termination port, or a ledger holding a lease whose child was never reaped, or a writer whose working directory could not be canonicalized
- **WHEN** a client reads a file in any worktree, in the main worktree, or in a change directory
- **THEN** the read reports `editable: false` with `managed_process_live` and a detail saying coverage is unknown
- **AND** the write fails `target_ineligible` and writes nothing
- **AND** the change-keyed `managed_process_live` on the state snapshot is unchanged

#### Scenario: Unscoped and session writers are covered

- **GIVEN** a child spawned through the shell command path or through the configured worktree session with `alpha`'s worktree as its working directory and no change ID
- **WHEN** a client reads a file in `alpha`'s worktree while the child is alive, and again after direct-child reap and confirmed process-group quiescence
- **THEN** the first read reports `editable: false` with `managed_process_live` and a sibling directory is `Clear`
- **AND** the second read reports `editable: true`
- **AND** shell exit with a surviving managed descendant does not clear coverage or allow a write

#### Scenario: Canonical containment decides a binding

- **GIVEN** a writer registered through a symlink alias of `alpha`'s worktree, and a sibling worktree whose name begins with `alpha`
- **WHEN** a client reads a file in each
- **THEN** `alpha` reports `editable: false` with `managed_process_live` and the sibling reports `editable: true`
- **AND** a writer whose working directory no longer exists at registration refuses every root

#### Scenario: Admitted targets exclude remote edits

- **GIVEN** change `alpha` is `queued` or `blocked`, or its `queue_intent` is queued
- **WHEN** a client reads and then writes a file in `alpha`'s change directory or in its worktree
- **THEN** the read reports `editable: false` with reason `change_admitted`
- **AND** the write fails `target_ineligible` with `change_admitted` and writes nothing

#### Scenario: Nested repository is not reachable through another root

- **GIVEN** change `alpha`'s worktree lies under the project root, and a directory under the main worktree carries its own `.git` entry
- **WHEN** a client addresses a file inside either through the main worktree's opaque ID
- **THEN** the read and the write are refused with `text_not_editable` and reason `nested_repository`
- **AND** the same file is served through `alpha`'s own worktree ID
- **AND** no bytes under the project root or under `alpha`'s worktree change

#### Scenario: Absent expectation creates exactly once

- **GIVEN** `notes.md` does not exist in the change directory
- **WHEN** `write_change_text` is submitted with `expected.kind = absent`
- **THEN** the file is created and the record reports `content_id_before: null`
- **AND** a second `absent` write is `text_stale`

#### Scenario: Older owner stays compatible

- **GIVEN** an owner built before this surface
- **WHEN** a client decodes its capabilities and submits `write_worktree_text`
- **THEN** `text_edit` decodes as unavailable
- **AND** the command is `validation_failed` with no record and no effect
- **AND** every existing read and command keeps working

#### Scenario: Contract completeness

- **GIVEN** a client captures `cflx openapi`
- **WHEN** it inspects the document
- **THEN** both text routes, both write commands, the `text_write` result kind, `text_stale`, `text_not_editable`, and `capabilities.text_edit` are present
- **AND** removing any of them from a copy fails the completeness check

### Requirement: Landed-merge and pre-merge failure evidence for the abort-on-conflict merge

`merge_worktree_abort_on_conflict` MUST carry the `merge_abort` result on
every settlement after eligibility, including the two paths that today
settle `internal_error` with `result: null`: an `on_merged` hook failure
after the merge commit landed, and a Git failure after eligibility but
before any merge commit landed. The owner MUST distinguish whether the hook ran and failed or Git failed before it from its recorded operation stage, never from error text. Separately, `merge.landed` MUST be derived from repository evidence re-read after the failure, never inferred from that stage. A hook failure with inconclusive or contrary landing evidence MUST publish the observed `unknown` or `not_landed`, not an invented `confirmed`; the owner MUST NOT compensate or run the hook again.

A merge whose commit landed and whose `on_merged` hook then failed MUST
settle the typed error code `merge_hook_failed` (HTTP 409) with
`outcome: merged_hook_failed`, `premerge_head`, a `merge` object whose
`landed` is `confirmed` only when the re-read `HEAD` differs from
`premerge_head`, is a descendant of it, and no `MERGE_HEAD` exists, and a
`hook` object (`ran: true`, `exit_ok: false`, bounded sanitized `detail`),
with `recovery: local_or_tui_required`. The owner MUST NOT re-run the hook,
re-merge, abort, reset, revert, or otherwise compensate; the base keeps the
merge commit, and the reducer's merged transition stays blocked exactly as it
is today. A Git failure after eligibility and before a merge commit landed
(dirty base at the clean check, unreadable `HEAD`, or a `git merge` failure
that is not a conflict) MUST keep the error code `internal_error` and MUST
carry `outcome: merge_failed` with `merge.landed` as `not_landed` only when
the re-read `HEAD` equals `premerge_head` and no `MERGE_HEAD` exists, and
`unknown` whenever `premerge_head` or either verification could not be read;
`hook` MUST be `ran: false`. Refusals before eligibility settled
(`worktree_not_found`, `root_busy`, `target_ineligible`) MAY still carry no
result, and the contract MUST name them as the only such settlements.
`merge_worktree` MUST remain byte-for-byte unchanged, including
`internal_error` with no result on its own hook failure. Capabilities MUST
list `merge_failed` and `merged_hook_failed` in `worktrees.merge_abort.reasons`,
the generated OpenAPI document MUST include the new error code, the two
outcome values, and the `merge` and `hook` schemas, and an older owner that
omits them MUST still decode as before.

#### Scenario: Hook failure after a landed merge is typed and not compensated

- **GIVEN** a worktree branch that merges cleanly and an `on_merged` hook that exits non-zero
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** base `HEAD` is a `--no-ff` merge commit of the branch and no `MERGE_HEAD` exists
- **AND** the record fails `merge_hook_failed` with `outcome: merged_hook_failed`, `merge.landed: confirmed`, `hook.ran: true`, `hook.exit_ok: false`
- **AND** the hook ran exactly once and base `HEAD` is unchanged after settlement
- **AND** `merge_worktree` for a sibling worktree with the same hook still fails `internal_error` with `result: null`

#### Scenario: Pre-merge Git failure reports that nothing landed

- **GIVEN** a base worktree whose clean check fails after eligibility
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** the record fails `internal_error` with `outcome: merge_failed`, `merge.landed: not_landed`, and `hook.ran: false`
- **AND** `merge.head_after` equals `premerge_head` and `merge.merge_head_present` is `false`
- **AND** `on_merged` did not run

#### Scenario: Unreadable landing evidence is unknown, not inferred

- **GIVEN** a Git failure after eligibility whose `HEAD` re-read cannot be performed
- **WHEN** the owner settles the command
- **THEN** `merge.landed` is `unknown` with the verification that failed named in `merge.detail`
- **AND** the record does not claim `not_landed` and the owner attempts no compensation

### Requirement: Session attempt, output provenance, and observed termination signal

A worktree session's `attempt`, its retained `output.lines`, and its `signal`
MUST be owner facts that child output cannot change. The runner MUST publish
its own notices — retry scheduled with the next attempt number and cause,
attempt started after a successful spawn, inactivity termination, runtime-limit
termination, launch failure, shutdown refusal, and process-group cleanup
diagnostics — to the session through a typed channel separate from the child's
stdout/stderr. The session MUST derive later `attempt` values only from
`AttemptStarted` after the corresponding process spawned, never from
`RetryScheduled` or the text of an output line. Before the first successful
spawn, the legacy initial `attempt: 1` remains unchanged; stopping a scheduled
retry during its delay does not increment `attempt` without a second spawn. A child line that begins with `[Retry `, contains a PGID, or imitates
any runner notice MUST remain in `output.lines` verbatim (after
control-sequence stripping) and MUST change no typed member. `output.lines`
MUST contain only bytes the child wrote to its stdout or stderr; runner
notices MUST NOT be interleaved into it and MUST instead be retained in a
bounded typed `runner_notices` list (`kind`, `attempt`, sanitized `detail`)
that discloses no template, expanded command, environment value, or absolute
path. The existing text-emitting runner entry point used by the TUI
terminal, the parallel executor, and the shell command path MUST keep
emitting the same stderr notice lines, so nothing outside the session
resource changes.

`signal` MUST be the signal that actually terminated the leader as reaped by
the runner, named from the wait status (`SIGTERM` when the group honoured
the first signal, `SIGKILL` when the runner escalated after its 5-second
grace), and `exit_code` MUST be the leader's real exit code when it exited
instead of dying from a signal. When the leader's wait status is unavailable
— the escalation timed out without a reap, or the runner never obtained a
status — `signal` and `exit_code` MUST both be `null` and the session MUST
NOT substitute the signal it requested. The configured retry policy inside
one session, the "stop admits no further attempt" guarantee, the 5-second
SIGTERM-to-SIGKILL escalation, verified group cleanup, and settlement after
reap are unchanged. The `worktree_sessions` capability MUST advertise
`max_runner_notices`, the generated OpenAPI document MUST include
`runner_notices` and the corrected `signal` description, and an older owner
that omits both MUST decode as before.

#### Scenario: Child output cannot forge an attempt or a runner notice

- **GIVEN** a configured command that prints `[Retry 1/3] Command crashed, retrying in 0ms...` and `process-group cleanup unconfirmed (pgid=1, force_killed=false)` to stderr and exits `0`
- **WHEN** the session finishes
- **THEN** both lines are present in `output.lines` on stream `stderr` exactly as printed
- **AND** `attempt` is `1`, `runner_notices` is empty, `state` is `succeeded`, and `exit_code` is `0`

#### Scenario: A real retry is a typed fact and the tail stays the child's

- **GIVEN** a retry policy allowing one retry and a command that crashes on its first attempt and succeeds on its second
- **WHEN** the session finishes
- **THEN** `attempt` is `2` and `runner_notices` holds one `retry_scheduled` notice naming attempt `2`
- **AND** no `[Retry ` line and no runner text appears in `output.lines`
- **AND** the TUI terminal path for the same command still prints the `[Retry 1/1]` stderr notice

#### Scenario: Stopping in the retry delay does not invent a new attempt

- **GIVEN** the first process failed and retry attempt `2` was scheduled, but stop cancelled the delay before another spawn
- **WHEN** the session settles
- **THEN** `attempt` remains `1` although `runner_notices` records the scheduled retry
- **AND** no second process was spawned

#### Scenario: Observed signal follows the escalation

- **GIVEN** a running session whose command ignores SIGTERM, and another whose command exits on SIGTERM
- **WHEN** `stop_worktree_session` is submitted to each
- **THEN** the ignoring session settles `stopped` with `signal: SIGKILL` after the escalation and `terminated: true`
- **AND** the honouring session settles `stopped` with `signal: SIGTERM`
- **AND** neither started a new process and `attempt` stays `1` for both

#### Scenario: Unavailable wait status is null, not the requested signal

- **GIVEN** a cancelled session whose leader status the runner could not obtain
- **WHEN** the session settles
- **THEN** `state` is `stopped`, `signal` is `null`, and `exit_code` is `null`
- **AND** the record does not read `SIGTERM`

### Requirement: Bounded directory enumeration in the consent oracle

The confirmed-delete content oracle MUST apply its record budget to each
directory entry as `readdir` returns it, before the entry's name is retained,
and MUST charge the retained name bytes of the listing being read against a
documented listing budget (`max_listing_name_bytes`, 64 MiB in production),
so that a directory with more entries than the remaining record budget or
more name bytes than the listing budget makes the observation `unbounded`
without first materializing the whole listing in memory. The oracle MUST
stop reading a listing at the first entry past either budget and MUST never
retain more names than the budget allows plus the one entry that exceeded
it. Every directory stream MUST be closed on every exit path, including the
`unbounded`, `nested_repository`, and unobservable ones, through an owned
guard rather than a trailing call, and every counter MUST use saturating
arithmetic. The depth limit of 256, the combined 500,000-record and 256 MiB
hashed-byte budget, byte-level identity, no-follow resolution, and the
`nested_repository` rule are unchanged, and no partial, prefix, or
metadata-only identity is substituted. The documented budget MUST name the
new listing bound, and `capabilities.worktrees.confirmed_delete` MUST remain
decodable by older clients.

#### Scenario: A huge single directory fails closed before being materialized

- **GIVEN** injected limits of `max_records: 5` and an untracked directory holding 20 entries
- **WHEN** the oracle walks the directory
- **THEN** the observation is `unbounded`
- **AND** the walker counted at most 6 records and retained at most 6 names before stopping
- **AND** the directory stream was closed and no identity was published

#### Scenario: Listing name bytes are bounded independently of record count

- **GIVEN** injected limits with a large record budget and a `max_listing_name_bytes` smaller than one directory's names
- **WHEN** the oracle walks that directory
- **THEN** the observation is `unbounded`
- **AND** the preview publishes `content_observation: unbounded` with `null` identities and every option ineligible

#### Scenario: Budgets that fit are unchanged

- **GIVEN** production limits and the existing bound-exceeded, depth, FIFO, and nested-repository fixtures
- **WHEN** the existing `remote_confirmed_delete` cases run
- **THEN** every identity, refusal, and `unbounded` outcome is the same as before this change

### Requirement: Detached worktree delete preview is ineligible

`GET /api/v2/worktrees/{worktree_id}/delete-preview` for a worktree whose
`HEAD` is detached or whose branch is empty MUST report every option
(`ordinary`, `skip_teardown`, `discard_known_dirty`,
`discard_commits_ahead`) as ineligible with the closed `blocked_reason`
`detached_head`, decided before dirty, ahead, content, or ignored facts and
independent of whether the inspection ran. The `consent` object MUST still
be published as observed, with `branch` empty, as a disclosure and not as an
admission ticket: the contract MUST state that `delete_worktree_confirmed`
refuses an empty `consent.branch` as `validation_failed` before any
observation, unchanged. A target that was previewed with a branch and became
detached before the command MUST be refused `target_ineligible` with reason
`detached_head` under the root guard with nothing torn down or removed.
`detached_head` MUST be added to `ConfirmedDeleteReason`, to the
`confirmed_delete.reasons` capability list, and to the OpenAPI enum; no
remote detached destructive deletion is introduced, the ordinary
`delete_worktree` path is unchanged, and unknown merge base still blocks
`discard_commits_ahead` with `ahead_unknown` for attached worktrees.

#### Scenario: Detached preview offers nothing and names why

- **GIVEN** a linked worktree checked out at a detached commit
- **WHEN** an authenticated client reads its `delete-preview`
- **THEN** all four options are `eligible: false` with `blocked_reason: detached_head`
- **AND** `consent.branch` is empty and `is_detached` is published
- **AND** submitting that consent is `validation_failed` and no command record changes any worktree

#### Scenario: Detaching after the preview is refused under the guard

- **GIVEN** a preview taken on an attached clean worktree
- **WHEN** the worktree is detached and the consent is submitted with `skip_teardown: true`
- **THEN** the command refuses `target_ineligible` with `refusal.reason: detached_head`
- **AND** nothing was torn down or removed and the worktree keeps its ID

### Requirement: Spawn-outcome-aware writer lease release

A writer lease taken for an unscoped spawn MUST record a typed spawn
outcome — `not_attempted`, `attempted`, `failed`, `spawned` with a bound
process group, or `spawned` without one — and its release on drop or
settlement MUST follow that outcome alone. A lease dropped or settled while
`not_attempted` (the runner positively never called `spawn`: admission or
shutdown refused it before the call) or `failed` (the spawn call returned an
error and produced no child) MUST be released, leaving coverage `Clear`. A
lease whose spawn was `attempted` but whose outcome the runner never
learned, or `spawned` with no process-group identity, MUST stay `Unknown`
with a detail naming the outcome, and no elapsed time, owner restart,
direct-child reap alone, or "missing PGID" reasoning MUST release it. A
lease `spawned` with a bound process group MUST keep today's rules: `Busy`
while the child runs, `Unknown` after an unconfirmed handle drop until a
process-group probe proves the group empty, and retained after reap while
descendants remain. The runner MUST mark `attempted` immediately before the
spawn call and bind the process group immediately after a successful spawn,
before any step that can fail, so a failure between spawn and managed-child
construction leaves a probe-clearable lease rather than a removed one.
Scoped run-command coverage, canonical cwd containment, sibling `Clear`,
`Busy`/`Unknown` precedence, `force_stop_change`, `owns_managed_process`,
and `kill_managed_process` are unchanged, and no kill authority is added.

#### Scenario: Refused-before-spawn lease releases, in-flight lease does not

- **GIVEN** a ledger and two leases: one whose spawn admission was refused before the call, one marked `attempted` whose runner task was aborted before reporting an outcome
- **WHEN** both leases are dropped and the root is read
- **THEN** the first root reads `Clear` and `editable: true`
- **AND** the second root reads `managed_process_live` with a detail naming an unresolved attempted spawn, and stays so after any wait and after reading again

#### Scenario: Spawn failure is released but a spawn without identity is not

- **GIVEN** a spawn whose `sh` cannot be executed, and a real spawn whose process id could not be read
- **WHEN** each lease is settled
- **THEN** the failed spawn's root reads `Clear`
- **AND** the identity-less spawn's root reads `managed_process_live` with a detail naming the missing process-group identity, and no probe or restart clears it

#### Scenario: Failure after a real spawn stays probe-clearable

- **GIVEN** a real spawn whose process group was bound and whose handle was dropped without a reap
- **WHEN** the group is still alive, and later after it is gone
- **THEN** the root reads `managed_process_live` while the group lives
- **AND** it reads `editable: true` once the probe proves the group empty, with nothing killed

### Requirement: Session stop received before the next spawn refuses that attempt

For a configured worktree session, the runner's cancellation path MUST honour
a `stop_worktree_session` that is received while no child process is running
— during the retry delay that follows a crash retry, during the retry delay
that follows an inactivity-timeout retry, during the stagger wait before an
attempt, and at the final spawn admission — by refusing the next attempt
instead of spawning it. The runner MUST race each of those waits against the
handle's cancellation and MUST re-read the cancellation immediately before
the spawn call, so the "admits no further attempt" guarantee holds on the
real process path and not only in an injected fixture. A refused attempt MUST
publish a typed `stop_refused_attempt` runner notice naming the attempt that
was refused and the stage at which it was refused (`retry`, `stagger`, or
`spawn`), MUST NOT publish `AttemptStarted`, and MUST settle the invocation
with `CommandTermination::Cancelled`, the process-group cleanup report of the
attempt that actually ran (or the not-applicable report when none did), the
writer lease settled, and no further loop iteration. An already scheduled
`retry_scheduled` notice MUST remain in `runner_notices`; a stop received
before the retry is scheduled MUST schedule none.

The session MUST then read `stopped`, keep `attempt` at the last attempt that
actually spawned (`1` when the first attempt is the only one that ran, and the
legacy initial `1` when none did), publish `exit_code` and `signal` from the
reaped leader of the attempt that ran (its own exit code with `signal: null`
after a crash, the signal that reaped it after an inactivity termination,
both `null` when no attempt ever spawned), retain only that attempt's bytes in
`output.lines`, and publish `result.terminated` as today's fact — `true` only
when the state is `stopped` and the last attempt's process group was proven
quiescent — with `result.state_after: stopped`. No wire member is added or
removed; the documented `kind` vocabulary gains `stop_refused_attempt`, and an
older owner that never publishes it MUST decode unchanged.

The cancellation signal MUST be read without being consumed while pending,
without being lost once sent, and without misreading a closed channel: a
dropped handle on the registry-scoped runner is a cancellation, exactly as the
running-child cancellation branch already decides, and a pending signal is left
armed for the next checkpoint. A stop that arrives after the final admission
check returned and before the spawn call returned MAY still be honoured
against the attempt it admitted, by the existing running-child cancellation
branch, and the session then truthfully publishes that attempt; the contract
MUST name this as the only remaining way an attempt can start after a stop.

This behaviour MUST be selected by an explicit stop policy on the shared
retry loop that only the session registry's observed entry point opts into.
The text-emitting entry point used by the TUI terminal, the parallel executor,
apply, archive, acceptance, and shell commands MUST keep its default
spawn-then-cancel behaviour and stderr text byte for byte; scope shutdown MUST
keep its own loop-head, pre-delay, and admission checks and its
`shutdown_refused` notice on both policies; session stop and owner shutdown
MUST remain two distinguishable operations; and the SIGTERM-to-SIGKILL
escalation, verified group cleanup, observed signal, retry policy, stagger
semantics for every existing caller, `CommandTermination`, and
`StreamingChildHandle` MUST be unchanged. No kill authority is added and no
other session or the scheduler is affected by one session's stop.

#### Scenario: Stop during a crash retry delay spawns nothing

- **GIVEN** a session with a retry policy admitting a retry, a 60-second retry delay, and a command that records its PID and exits non-zero
- **AND** the session resource already shows a `retry_scheduled` notice for attempt `2`
- **WHEN** `stop_worktree_session` is submitted
- **THEN** the record settles `result.state_after: stopped` well within the hang guard rather than after the delay
- **AND** exactly one PID was ever recorded by the command and `attempt` is `1`
- **AND** `runner_notices` is `retry_scheduled` for attempt `2` followed by `stop_refused_attempt` for attempt `2` with `stage=retry`
- **AND** `exit_code` is the first attempt's own exit code, `signal` is `null`, and `output.lines` holds only the first attempt's output

#### Scenario: Stop during an inactivity retry delay spawns nothing

- **GIVEN** a session whose command prints nothing, an inactivity timeout with a remaining inactivity retry budget, and a 60-second retry delay
- **AND** the session shows `inactivity_terminated` for attempt `1` and `retry_scheduled` for attempt `2` with `cause=inactivity_timeout`
- **WHEN** `stop_worktree_session` is submitted
- **THEN** no second process is spawned and `attempt` is `1`
- **AND** `stop_refused_attempt` for attempt `2` with `stage=retry` follows the scheduled notice
- **AND** `signal` is the signal that reaped the first attempt and the session reads `stopped`

#### Scenario: Stop while parked in the stagger wait or at the spawn boundary spawns nothing

- **GIVEN** a session whose first attempt crashed with no retry delay, and either a long stagger wait before attempt `2` or the shared stagger lock held by the test so attempt `2` is blocked at the spawn boundary
- **WHEN** `stop_worktree_session` is submitted and the barrier is then released
- **THEN** no second process is spawned, `attempt` is `1`, and the session reads `stopped`
- **AND** `stop_refused_attempt` for attempt `2` names `stage=stagger` or `stage=spawn`

#### Scenario: Stop before the first spawn is stopped, not failed

- **GIVEN** a session whose first attempt is blocked at the spawn boundary before any process exists
- **WHEN** `stop_worktree_session` is submitted and the barrier is released
- **THEN** no process is ever spawned and the session reads `stopped` with `attempt: 1`, `exit_code: null`, `signal: null`
- **AND** `runner_notices` holds one `stop_refused_attempt` for attempt `1`

#### Scenario: Stopping a running leader is unchanged

- **GIVEN** a running session whose command ignores SIGTERM, and another whose command honours it, each with a retry policy of at least one retry
- **WHEN** `stop_worktree_session` is submitted to each
- **THEN** the ignoring session settles `stopped` with `signal: SIGKILL` and the honouring one with `signal: SIGTERM`, both with `terminated: true`, `attempt: 1`, and the whole group gone
- **AND** neither session records a `stop_refused_attempt` notice

#### Scenario: Owner shutdown during a retry delay stays a shutdown refusal

- **GIVEN** a session parked in a 60-second retry delay after its first attempt crashed
- **WHEN** the owner closes the session registry scope
- **THEN** no second process is spawned and the session reads `stopped` with `attempt: 1`
- **AND** `runner_notices` records `shutdown_refused` and no `stop_refused_attempt`

#### Scenario: The text entry point keeps spawn-then-cancel

- **GIVEN** the same crashing command run through the text-emitting runner entry point with the shared stagger lock held so attempt `2` is blocked at the spawn boundary
- **WHEN** the handle is terminated and the lock is released
- **THEN** attempt `2` spawns and is then terminated by the existing cancellation branch, so two PIDs were recorded
- **AND** the `[Retry 1/N]` stderr line is printed as before and no refusal text appears

### Requirement: Completed changes stay published for the owner incarnation

The `/api/v2` change projection — `snapshot.changes` of `GET /api/v2/state`, `GET /api/v2/changes`, `GET /api/v2/changes/{change_id}`, and the snapshot at every event's `state_revision` — MUST publish, for the rest of the owner incarnation, every change this incarnation has already published whose reducer-authored display status is `merged` or `pushed`, whether or not the latest successful workspace observation (an event-driven `ChangesRefreshed` or the periodic Web/UDS disk refresh) lists it. This includes a change that an earlier observation stopped publishing while it waited or failed — for example in `merge wait` after an `on_merged` hook failure, or in `error` after a publication failure — and that the reducer later completes: such a change MUST be published again no later than the first reducer event or workspace observation after that completion whose update reads reducer state. A change this incarnation has already published whose reducer-authored display status is `resolving` MUST be published under the same conditions while it is `resolving`.

A previously published row that a workspace observation does not list and whose deciding status is not `merged`, `pushed`, or `resolving` MUST be released by that observation and MUST NOT be published while it stays unlisted in such a status, exactly as before this requirement. The deciding status MUST be the reducer-authored display status from the reducer state the observation holds — the dispatch's authoritative state or the observation's non-blocking read — or, only when that observation cannot read reducer state, the row's last reducer-derived status. Only workspace observations release rows. A kept `resolving` row whose reducer-authored status has settled into another status therefore stays published, with the reducer-authored status each projection that reads the reducer publishes for it (for example `merge wait` or `error`), until the first observation that reads reducer state releases it; an observation that cannot read reducer state keeps it while its last reducer-derived status is still `resolving`. An update that cannot read reducer state MUST NOT publish a released change again. Retention and republication MUST NOT be inferred from absence, task completion, archive-directory presence, or `archive_complete`, and the reducer never authors `archived`.

To publish a released change again, the owner MAY keep, in process memory only, the last published row of every change it released. That memory MUST be private observability data: no row, total, run clock, net-active or status observation, worktree relation, or other resource may expose it or be derived from it; no scheduling, queue, mark, Start, retry, stop, dequeue, targeted force-stop, resolve, mark reconciliation, resume, acceptance, archive, or merge decision may read it; it MUST NOT be persisted or rehydrated; and every new owner process MUST start without it. A released change MUST be published again only when this incarnation published it before and the reducer authors `merged`, `pushed`, or `resolving` for it while the latest observation does not list it. When an observation lists a released change again, the owner MUST discard that memory and publish the change from the observation exactly as a change that was never kept, including new-arrival attention.

A kept or republished row MUST keep its exact `id`, its last observed listing fields (task counts, progress, progress status, dependencies, iteration), and the reducer-authored and process-local facts this owner holds for it: `archive_complete` and acceptance provenance from the reducer; `timing`, `latest_activity`, and attention as recorded from every event this owner applied, including events applied while it was released; and `net_active` as accumulated from the reducer statuses observed while it was published. Workspace-observation facts (`worktree`, `parallel`) MUST keep following the latest observation and MUST NOT be guessed. Rows the latest observation lists come first, in observation order, followed by rows kept from earlier publication in their previously published order, followed by rows published again in the order they were released. An observation that lists a published row's id again MUST replace it with exactly one row carrying the observation's listing fields and the reducer-authored status, without a duplicate and without new-arrival attention.

Retention and republication MUST happen inside the existing snapshot update, so a kept row is published in the same `state_revision` as the observation that kept it, a row published again is published in the same `state_revision` as the event or observation that published it again, and a repeated identical observation MUST NOT advance `state_revision`. A `changes_refreshed` event payload keeps counting the workspace observation; the snapshot at the event's revision remains the authority for rows.

Retention is process-local presentation state. No file, store, TTL, or later incarnation holds it, and no scheduling, queue, mark, Start, retry, stop, dequeue, targeted force-stop, resolve, mark reconciliation, resume, acceptance, archive, or merge decision may read it. Totals, action eligibility, the run clock, and net-active timing MUST classify published rows with the same shared rules as every other projected row and MUST NOT read released rows. TUI-local dismissal MUST NOT remove an API row, the API MUST NOT read TUI presentation state, and hiding or restoring rows in any client requires no Conflux command.

#### Scenario: Merged row survives the archive move and later refreshes

**Given**: this owner published change `alpha` and the reducer reports `alpha` as `merged`
**And**: `alpha`'s proposal directory has moved to `openspec/changes/archive/`, so workspace observations no longer list it
**When**: any number of event-driven `ChangesRefreshed` observations and periodic disk refreshes complete
**Then**: `GET /api/v2/changes`, `GET /api/v2/changes/alpha`, and `GET /api/v2/state` all publish `alpha` with `display_status: merged`
**And**: its `id`, last observed task counts and progress, `archive_complete`, `timing`, `latest_activity`, and `net_active` are unchanged
**And**: repeated identical observations do not advance `state_revision`

#### Scenario: Pushed row is retained like a merged row

**Given**: this owner published change `beta` and the reducer reports `beta` as `pushed`
**When**: later observations no longer list `beta`
**Then**: `beta` stays published with `display_status: pushed` for the rest of the incarnation

#### Scenario: Row integrated after its proposal left the base stays while it resolves

**Given**: `alpha` is `resolving` and its merge already landed on the base branch, so the base no longer lists it
**When**: observations complete before the reducer authors the merge completion
**Then**: `alpha` stays published as `resolving`
**And**: after the reducer authors `merged`, `alpha` stays published for the rest of the incarnation

#### Scenario: Change released while waiting is published again when the reducer completes it

**Given**: this owner published `alpha`, its `on_merged` hook failed after its merge landed, and the reducer reports `merge wait`
**And**: an event-driven observation and a disk refresh that do not list `alpha` released it
**When**: an explicit resolve makes the reducer author `resolving` and then `merged` for `alpha`
**Then**: `alpha` is published again as `resolving` in the snapshot of the first update after that transition that reads reducer state, and as `merged` afterwards
**And**: it stays published for the rest of the incarnation with its last observed task counts, `archive_complete`, and the `timing`, `latest_activity`, `net_active`, and attention an owner that never stopped listing it would publish

#### Scenario: Change released after a publication failure is published again when publication completes

**Given**: this owner published `beta`, its publication failed, and the reducer reports `error`
**And**: observations that do not list `beta` released it
**When**: the resumed publication makes the reducer author `pushed` for `beta`
**Then**: `beta` is published again as `pushed` no later than the first update after that completion that reads reducer state
**And**: it stays published for the rest of the incarnation

#### Scenario: A settled resolving row is released only by an observation that reads the reducer

**Given**: `delta` is kept as `resolving` while the base no longer lists it
**When**: the reducer authors `merge wait` for `delta`
**Then**: until the next observation that reads reducer state, `delta` stays published with the status each projection that reads the reducer publishes for it, here `merge wait`
**And**: an observation that cannot read reducer state while the last reducer-derived status of `delta` is still `resolving` keeps it
**And**: the first observation that reads reducer state and does not list `delta` releases it

#### Scenario: Completion is never inferred

**Given**: absent rows whose reducer-authored status is `not queued` with every task complete, `merge wait` or `resolve pending` with archive completion recorded, `error`, `rejected`, or `stopped`, and a row for which no reducer evidence exists
**When**: an observation no longer lists them
**Then**: they leave the projection exactly as before this requirement
**And**: they stay unpublished for as long as the reducer authors none of `merged`, `pushed`, or `resolving` for them
**And**: no row is published as `merged` or `pushed` unless the reducer authored that status

#### Scenario: Released changes stay private

**Given**: `delta` was released while the reducer reports `merge wait`
**When**: a client reads state, changes, the `delta` row, execution status, the `delta` refresh preview, or the `delta` change text, or an observation reports a worktree path for `delta` without listing it
**Then**: no row, total, execution-status entry, or worktree relation names or counts `delta`
**And**: every `delta` read answers as it does for a change this owner never observed
**And**: no command admission, mark, queue intent, or run-clock decision reads what the owner remembers about `delta`

#### Scenario: Reappearance keeps one row

**Given**: published row `alpha` is `merged` and no longer listed
**When**: a later observation lists a change directory named `alpha` again
**Then**: the projection holds exactly one `alpha` row with the observation's listing fields and the reducer-authored status
**And**: the row is not reported as newly detected

#### Scenario: A released change listed again is observed as before

**Given**: released change `epsilon` is not published and the reducer reports `merge wait`
**When**: a later observation lists `epsilon` again
**Then**: the projection holds exactly one `epsilon` row built from that observation with the reducer-authored status
**And**: it is reported as newly detected with no facts carried from before its release, exactly as before this requirement

#### Scenario: Snapshot, list, get, and stream agree

**Given**: `alpha` is retained
**When**: a client reads state, list, and get and replays the event stream from its cursor
**Then**: every response names the same `instance_id` and `state_revision` and the same `alpha` row
**And**: a `changes_refreshed` event payload counts the workspace observation while the snapshot at that event's revision still publishes `alpha`
**And**: a client that reconnects with `GET /api/v2/state` and its `event_sequence` cursor observes no removal of `alpha`
**And**: a change published again by a reducer event is published in the snapshot at that event's revision

#### Scenario: Failed, contended, or empty refresh does not erase completion

**Given**: `alpha` is retained and `delta` is released
**When**: a refresh fails before producing an observation, the reducer cannot be read while an observation is applied, or the observation lists no change at all
**Then**: `alpha` stays published with its reducer-authored status
**And**: a refresh that fails before producing an observation changes no row and no revision
**And**: `delta` is published again once a later update that reads reducer state finds it `merged`, `pushed`, or `resolving`

#### Scenario: Retention authorizes nothing

**Given**: `alpha` is retained as `merged` and may still carry an execution mark set before it completed
**When**: a client submits mark, unmark, bulk mark, queue intent, Start, retry, retry-errors, stop-and-dequeue, targeted force-stop, or resolve for `alpha`
**Then**: each settles exactly as it settles for a listed `merged` row in the same state
**And**: each settles for a released `merge wait` change exactly as for the same change while listed
**And**: no queue intent, mark, scheduler, retry edge, resolve reservation, or mode effect results from retention
**And**: `alpha.actions` reports every action blocked
**And**: totals count `alpha` as completed and never as pending or in progress
**And**: neither the run clock nor `alpha.net_active` reopens or resets

#### Scenario: Reads have no side effects

**Given**: `alpha` is retained and `delta` is released
**When**: a client repeatedly reads state, changes, the `alpha` and `delta` rows, capabilities, execution status, the refresh previews, and the event replay
**Then**: `state_revision`, `event_sequence`, the command and idempotency registries, reducer state, execution marks, and every repository file and Git ref are unchanged
**And**: `delta` is still published again when the reducer later completes it

#### Scenario: TUI and API retain the same completed rows

**Given**: the TUI and the API observe the same owner
**When**: `alpha` reaches `merged`, `beta` reaches `pushed`, `gamma` reaches `merged` through an explicit resolve after an `on_merged` failure, and `delta` reaches `pushed` through resumed publication after a publication failure, and all of them are later absent from every observation
**Then**: after each completion both frontends keep the completed change with the same `id`, display status, task counts, archive completion, and net-active time
**And**: when explicit resolve is requested with the resolve lane free, the API `run_clock` equals the TUI's Elapsed clock during the absent `resolving` refresh, at its settlement, and afterwards; no parity is guaranteed while a private `resolve pending` row waits for a busy lane

#### Scenario: TUI dismissal leaves the API row

**Given**: the TUI and the API both retain merged row `alpha`
**When**: the operator dismisses `alpha` in the TUI and later observations still do not list it
**Then**: the API `alpha` row and `state_revision` are unchanged by the dismissal
**And**: no Conflux command, reducer change, or Git mutation is involved in hiding or restoring a row in any client

#### Scenario: A new owner starts without retained rows

**Given**: an owner retained `alpha` and is replaced by a new process
**When**: the new owner observes the same workspace
**Then**: it publishes a new `instance_id` and no `alpha` row
**And**: changes still present in the workspace are observed independently
**And**: a reducer completion for a change the new owner never published publishes nothing
**And**: nothing is persisted or rehydrated from the earlier incarnation or from archived history

### Requirement: Completed change retention is discoverable

`GET /api/v2/capabilities` MUST publish `completed_change_retention` with `available: true`, `retained_statuses` exactly `merged` and `pushed`, `settling_statuses` exactly `resolving`, and `scope: instance`, meaning that every change this owner incarnation published is published, as specified by the retention requirement, while the reducer reports one of those statuses — including after it was released while waiting or failing — and that retained rows live exactly as long as the published `instance_id`. The generated OpenAPI document MUST publish the `CompletedChangeRetentionCapability` schema with those four members required, MUST publish `CapabilitiesResponse.completed_change_retention` as an optional member, and MUST describe retention on `InstanceSnapshot.changes` and `ChangesResponse.changes`. A capabilities payload that omits the block MUST decode as `available: false` with empty status lists and an empty scope — never as available — while a payload that omits any pre-existing required member MUST still be rejected. The block adds no route, command, or change-resource member, and the owner's private memory of released rows adds none either. `cflx client` MUST keep working against owners with and without the block; no client may require it to start or to operate. Published documentation MUST tell consumers that locally hidden or dismissed rows are scoped by `instance_id` and must be cleared or rekeyed when it changes.

#### Scenario: Current owner advertises the guarantee

**Given**: an owner built with this requirement serves `/api/v2`
**When**: a client reads `GET /api/v2/capabilities`
**Then**: `completed_change_retention` reports `available: true`, `retained_statuses: [merged, pushed]`, `settling_statuses: [resolving]`, and `scope: instance`
**And**: the closed command set is unchanged

#### Scenario: Older owner decodes as unavailable

**Given**: a capabilities payload from an owner that predates this requirement
**When**: a client built with this requirement decodes it
**Then**: `completed_change_retention.available` is `false` and the status lists and scope are empty
**And**: `status`, `wait`, and `mark` keep working against that owner
**And**: a payload missing a pre-existing required member is still rejected

#### Scenario: Generated contract publishes the block as optional

**Given**: a consumer reads the canonical generated OpenAPI document
**When**: it inspects `CapabilitiesResponse` and `CompletedChangeRetentionCapability`
**Then**: `completed_change_retention` is published and is not required
**And**: `available`, `retained_statuses`, `settling_statuses`, and `scope` are published and required
**And**: the `changes` descriptions of `InstanceSnapshot` and `ChangesResponse` name `completed_change_retention`
