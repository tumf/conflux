## ADDED Requirements

### Requirement: Owner-only Windows Named Pipe transport

On Windows, a web-monitoring-enabled owner MUST serve the existing HTTP `/api/v2` router over a repository-scoped Named Pipe by default, without requiring the browser-facing TCP listener. The pipe name MUST be derived deterministically from the current user identity and normalized canonical Git common directory without embedding the raw SID or repository path. Linked worktrees of one repository MUST derive the same name, while distinct repositories or users MUST derive different names.

Every pipe instance MUST be created with an owner-only DACL at creation time and MUST reject remote clients. The initial server instance MUST request exclusive first-instance ownership and fail owner startup when the name is already owned. Named Pipe startup and shutdown MUST participate in the same all-or-nothing listener lifecycle as Unix and TCP listeners.

#### Scenario: Windows owner serves the shared API without TCP

**Given**: a Windows owner starts in a Git repository with web monitoring support and without `--web`
**When**: local API listeners are created
**Then**: the owner binds its deterministic Named Pipe
**And**: the pipe serves the same `/api/v2` router, DTOs, state, commands, subscriptions, authentication, and revision rules as the Unix listener
**And**: no TCP listener is required or inferred

#### Scenario: Repository identity determines one endpoint

**Given**: a repository has a main worktree and linked worktrees on Windows
**When**: each worktree resolves the default local API endpoint for the same current user
**Then**: every worktree derives the same pipe name
**And**: a different repository or current user derives a different pipe name
**And**: no durable endpoint registry is written

#### Scenario: Pipe creation is owner-only and race-free

**Given**: a Windows owner prepares its first Named Pipe instance
**When**: it creates the pipe
**Then**: a DACL granting access only to the current user is supplied at handle creation
**And**: remote clients are rejected
**And**: first-instance ownership is requested before orchestration starts
**And**: an existing owner or endpoint collision causes hard startup refusal instead of replacement or TCP fallback

#### Scenario: Graceful shutdown closes the Windows listener

**Given**: a Windows owner has a pending accept and connected local API requests
**When**: its local API handle is shut down
**Then**: new accepts stop
**And**: pending and active listener-owned tasks are awaited or cancelled under the existing shutdown contract
**And**: a later owner may acquire the first pipe instance cleanly

### Requirement: Owner-only local transport equivalence

The owner MUST classify both its mode-`0600` Unix-domain socket and its owner-only Windows Named Pipe as trusted local API transports. Any `/api/v2` operation whose security contract requires the owner-only local transport, including proposal-subscription or completion-sink argv storage and disclosure, MUST permit either trusted local transport and MUST continue refusing TCP. The classification MUST be centralized so security-sensitive handlers cannot diverge by matching one platform directly.

#### Scenario: Sensitive callback argv is allowed over Named Pipe

**Given**: a request arrives over the owner's secured Windows Named Pipe
**When**: a client sets, gets, or clears a proposal subscription or completion sink whose contract permits owner-only local access
**Then**: the handler applies the same authorization and disclosure behavior as over the Unix socket
**And**: it uses the same DTO, revision, validation, and settlement logic

#### Scenario: TCP remains untrusted

**Given**: the same request arrives over TCP, including loopback TCP with valid bearer authentication
**When**: it attempts an operation restricted to owner-only local transport
**Then**: it is refused with `transport_not_permitted`
**And**: no stored callback argv is disclosed or mutated

## MODIFIED Requirements

### Requirement: Client observation does not alter API semantics

Serving the local client over Unix-domain socket or Windows Named Pipe MUST reuse the existing `/api/v2` router, DTOs, optimistic revision rules, idempotent command records, shared operator application transaction, authentication, response bounds, and event stream. The API MUST NOT add a second orchestration path, platform-specific command semantics, client-specific hidden mutation, or durable run record solely for `cflx client`.

#### Scenario: Client controls use ordinary typed commands

**Given**: the client changes marks or explicitly requests Start, Stop, or ForceStop over either supported platform-local transport
**When**: it submits a mutation to the owner
**Then**: every mutation appears as an ordinary v2 command record
**And**: each command uses the same shared operator intent as the equivalent TUI control
**And**: mark/unmark never submits queue intent or a lifecycle command
**And**: TUI and API projections observe the same shared operator outcomes

#### Scenario: Status and wait are read only

**Given**: a client invokes client status or wait over Unix-domain socket or Windows Named Pipe
**When**: it reads snapshots, execution status, events, and repository evidence
**Then**: no v2 command record is created by those operations
**And**: no process-local mark, queue, scheduler, resolver, cancellation, or mode state changes because of observation

#### Scenario: Proposal subscriptions are observability only

**Given**: a client sets, reads, or clears proposal subscriptions over an owner-only local transport
**When**: the subscription operation settles
**Then**: no workflow command record is created
**And**: state revision, marks, queue intent, scheduler state, mode, and workflow routing remain unchanged

<!-- Expected canonical result after archive: Unix and Windows local clients share one /api/v2 semantic contract; only endpoint creation and stream connection vary by platform. -->

### Requirement: Execution-sink remote resources

The owner MUST expose authenticated `GET`, `PUT`, and `DELETE /api/v2/executions/{execution_id}/sink` resources outside the closed workflow command registry. Every request, including inspection, MUST require and validate the exact `(instance_id, execution_id, change_id)` binding. These identifiers are readable through authenticated resources, so the binding is a coherence check rather than access control. Registered argv MUST be returned only to a request that arrived on the owner-only local transport. Sink operations MUST create no workflow command record, require no `expected_revision` or idempotency key, and MUST NOT advance `state_revision` or mutate orchestration state.

Capabilities MUST advertise execution-sink support, execution status MUST expose the current execution ID, and the generated OpenAPI contract MUST describe these resources and schemas. Clients that reach an older owner without this capability MUST fail with a typed unsupported-owner outcome. Sink mutation MUST be accepted only over the owner-only local transport; TCP mutation MUST fail with a typed refusal even when bearer authentication succeeds.

#### Scenario: Sink registration is not a workflow command

- **GIVEN** an admitted execution and a coherent owner revision
- **WHEN** an owner-only local client registers a sink with its exact binding
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

### Requirement: Proposal-scoped explicit subscriptions

The owner MUST provide process-local proposal-scoped subscriptions separate from workflow commands and execution-scoped sink resources. The wire identifier remains `change_id`; user-facing documentation MAY call the addressed change a proposal. One subscription record is keyed by `change_id` and stores one bounded argv callback plus optional blocked-edge delivery. A request MUST address one through 64 distinct change IDs. Multi-proposal `set` and `clear` MUST be atomic: validation failure for any requested proposal or callback causes no requested mutation. `get` MUST require named IDs and MUST NOT provide an unbounded list-all operation.

A subscription MAY be registered before a proposal is admitted. Whenever that proposal enters a new execution episode, the owner MUST bind the episode to the current proposal subscription. Setting a subscription while the latest episode is admitted and not terminal MUST bind that live episode immediately. Each bound episode MUST independently deliver the first typed terminal classification among `completed`, `failed`, and `stopped`; optional `blocked` delivery remains edge-triggered per episode. Event data and callback environment MUST retain the existing `change_id` / `CFLX_CHANGE_ID` naming and contain the actual `instance_id`, `execution_id`, and `change_id`. No `CFLX_PROPOSAL_ID` alias is added. Re-admission with a new execution ID creates a distinct delivery episode while retaining the proposal subscription until explicit clear or owner exit.

Replacing a subscription MUST apply the new argv and blocked setting to the current undelivered live or terminal episode and to future episodes. Clearing MUST cancel pending delivery for named proposals but MUST NOT terminate a callback process already started. Delivery dedupe is keyed by execution episode and event edge, not subscription generation: replacing, clearing, or clearing then setting MUST NOT redeliver a terminal event already delivered by this owner for that execution. Setting after typed terminal settlement MUST immediately attempt the latest terminal event only when this owner has not already delivered that event. The owner MUST retain at most the latest terminal episode and its delivery history per proposal and discard them on owner exit. Registration before any execution MUST NOT synthesize an execution ID, start work, mutate marks or queue intent, or promise admission. Owner restart invalidates all proposal subscriptions and does not promise delivery from the lost process.

The owner MUST expose authenticated `GET`, `PUT`, and `DELETE /api/v2/proposals/{change_id}/subscription` outside the closed workflow command registry. Every request, inspection included, MUST carry and validate the complete `(instance_id, change_id)` binding; partial binding is refused and a mismatched instance returns typed `owner_restarted`. Capabilities MUST advertise proposal-subscription support, clients reaching an older owner MUST return typed `unsupported_owner`, and generated OpenAPI MUST describe the resources and schemas. Set and clear are accepted only over the owner-only local transport. Get over TCP MAY report presence, current execution state, and delivery history but MUST omit argv; argv is returned only over an owner-only local transport. Subscription operations create no workflow command record and do not advance `state_revision`.

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
