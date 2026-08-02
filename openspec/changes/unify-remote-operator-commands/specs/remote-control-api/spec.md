## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only the command variants declared by the current v2 contract. Every command MUST include `expected_revision` and `idempotency_key`. Accepted lifecycle commands MUST execute through the same process-local application services used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution and MUST NOT maintain an independent workflow state machine.

#### Scenario: Accepted command uses shared behavior

**Given**: A command variant is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared application service revalidates and executes it
**And**: TUI-equivalent reducer, scheduler, cancellation, side-effect, and event semantics apply
**And**: The command settles as succeeded, no-op, or failed according to the actual service outcome

#### Scenario: Empty start target is not successful

**Given**: No eligible execution-marked change exists at the admitted revision
**When**: Start or resume is submitted
**Then**: No scheduler is started
**And**: The command settles as no-op or failed with actionable detail

### Requirement: Serialized optimistic revision control

One process-local projection owner MUST serialize command admission, snapshot mutation, `state_revision`, `event_sequence`, event storage, and publication. For each state-affecting input it MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; a new stale command MUST fail without service execution. A command's `result_revision` MUST include its synchronously accepted decision-state effect and MUST NOT merely capture the revision observed after enqueueing deferred work.

#### Scenario: Command result revision includes admission effect

**Given**: A lifecycle command changes marks, queue intent, mode, or resolve reservation during admission
**When**: Its command record settles
**Then**: `result_revision` identifies a snapshot containing that accepted change
**And**: Later asynchronous progress may advance revision separately

## ADDED Requirements

### Requirement: Shared lifecycle scheduling semantics

Start, resume, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification.

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
