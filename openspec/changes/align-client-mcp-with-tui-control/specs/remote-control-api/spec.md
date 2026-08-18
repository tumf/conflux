## MODIFIED Requirements

### Requirement: Client observation does not alter API semantics

Serving the local client MUST reuse the existing `/api/v2` router, DTOs, optimistic revision rules, idempotent command records, and shared operator application transaction. The API MUST NOT add a second orchestration path, client-specific hidden mutation, or durable run record solely for `cflx client`.

#### Scenario: Client controls use ordinary typed commands

**Given**: the client changes marks or explicitly requests Start, Stop, or ForceStop
**When**: it submits a mutation to the owner
**Then**: every mutation appears as an ordinary v2 command record
**And**: each command uses the same shared operator intent as the equivalent TUI control
**And**: mark/unmark never submits queue intent or a lifecycle command

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

The owner MUST provide process-local proposal-scoped subscriptions separate from workflow commands and execution-scoped sink resources. One subscription record is keyed by proposal ID and stores one bounded argv callback plus optional blocked-edge delivery. A bounded multi-proposal request MUST support atomic `set`, `get`, and `clear`: validation failure for any requested proposal or callback MUST cause no requested subscription mutation.

A subscription MAY be registered before a proposal is admitted. Whenever that proposal enters a new execution episode, the owner MUST bind the episode to the current proposal subscription. Each bound episode MUST independently deliver the first typed terminal classification among `completed`, `failed`, and `stopped`; optional `blocked` delivery remains edge-triggered per episode. Event data MUST contain the actual `instance_id`, `execution_id`, and proposal ID. Re-admission with a new execution ID MUST create a distinct delivery episode while retaining the proposal subscription until explicit clear or owner exit.

Registering or replacing a subscription after the proposal's latest execution has already reached typed terminal settlement MUST immediately attempt that latest terminal delivery once. Registration before any execution MUST NOT synthesize an execution ID, start work, mutate marks or queue intent, or promise that the proposal will be admitted. Owner restart invalidates all proposal subscriptions and does not promise delivery from the lost process.

Subscription mutation MUST be accepted only over the owner Unix socket. Callback argv MUST be executed directly without shell interpretation and MUST retain existing completion-sink limits, scrubbed environment, private artifact ownership, bounded output and duration, delivery dedupe, failure isolation, and secret-free diagnostics. Callback success or failure MUST NOT alter workflow outcome.

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

#### Scenario: Clear removes only named proposal subscriptions

- **GIVEN** subscriptions exist for `alpha`, `beta`, and `gamma`
- **WHEN** one request clears `alpha` and `gamma`
- **THEN** those subscriptions are absent and `beta` remains unchanged
- **AND** already-running callback processes are not interpreted as workflow control

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

#### Scenario: Owner restart invalidates proposal subscriptions

- **GIVEN** proposal subscriptions belong to owner instance `owner-a`
- **WHEN** that owner exits and a new owner starts as `owner-b`
- **THEN** the old subscriptions are absent
- **AND** no delivery from `owner-a` is promised
