## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, and `resolve_merge` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted lifecycle commands MUST execute through the same process-local application services used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution and MUST NOT maintain an independent workflow state machine.

#### Scenario: Accepted command uses shared behavior

**Given**: A command variant is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared application service revalidates and executes it
**And**: TUI-equivalent reducer, scheduler, cancellation, side-effect, and event semantics apply
**And**: The command settles as succeeded, no-op, or failed according to the actual service outcome

#### Scenario: Unknown command type is rejected

**Given**: A command envelope names a type outside the closed enum
**When**: It is submitted
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: No service call occurs

#### Scenario: Empty start target is not successful

**Given**: No eligible execution-marked change exists at the admitted revision
**When**: Start is submitted
**Then**: No scheduler is started
**And**: The command settles as no-op or failed with actionable detail

### Requirement: Serialized optimistic revision control

One process-local projection owner MUST serialize command admission, snapshot mutation, `state_revision`, `event_sequence`, event storage, and publication. For each state-affecting input it MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; a new stale command MUST fail without service execution. Snapshot mutations MUST publish all related decision fields coherently at the same resulting revision. A command's `result_revision` MUST include its synchronously accepted decision-state effect and MUST NOT merely capture the revision observed after enqueueing deferred work.

#### Scenario: State event and snapshot share one revision

**Given**: A state-affecting execution event changes the current snapshot
**When**: The projection owner processes it
**Then**: It increments revision once
**And**: The stored snapshot and published event contain the same resulting revision

#### Scenario: No-op does not advance revision

**Given**: An input produces no snapshot change
**When**: The projection owner processes it
**Then**: `state_revision` is unchanged

#### Scenario: Stale new command is rejected

**Given**: The current state revision is 12
**When**: A new command supplies expected revision 11
**Then**: The server returns HTTP 409 with `stale_revision` and current revision 12
**And**: No command side effect occurs

#### Scenario: Mark mutation reads back coherently

**Given**: An accepted command changes an execution mark without changing queue intent
**When**: The resulting state revision is read
**Then**: The snapshot reports the new execution mark and unchanged queue intent together
**And**: No client-side inference is required

#### Scenario: Command result revision includes admission effect

**Given**: A lifecycle command changes marks, queue intent, mode, or resolve reservation during admission
**When**: Its command record settles
**Then**: `result_revision` identifies a snapshot containing that accepted change
**And**: Later asynchronous progress may advance revision separately

## ADDED Requirements

### Requirement: Shared lifecycle scheduling semantics

Start, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification.

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
