## MODIFIED Requirements

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

## ADDED Requirements

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
