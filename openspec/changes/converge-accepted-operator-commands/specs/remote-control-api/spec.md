## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, `resolve_merge`, and `set_all_execution_marks` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted operator commands MUST execute through the same process-local application transaction used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution, derive command admission from a stale frontend mode, or maintain an independent workflow state machine.

The API MUST settle a command as succeeded, no-op, or failed from the actual shared-service outcome. A failed command MUST NOT retain partial reducer, execution-mark, queue, retry-edge, resolve-reservation, stop-flag, process-mode, scheduler, hook, event, or projection effects.

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
**And**: scheduler preparation or activation fails
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

One process-local projection/application owner MUST serialize exact idempotency lookup, new-command admission, final lifecycle and target revalidation, service mutation, synchronous outcome dispatch, snapshot mutation, `state_revision`, `event_sequence`, event storage, command settlement, and publication. The serialization boundary MUST remain held until a state-changing command stores the revision produced by its own accepted outcome or a no-op/failed command stores the unchanged admitted revision.

For each state-affecting input, the owner MUST increment revision exactly once if and only if the snapshot changes and MUST attach that resulting revision to the event. Log-only inputs MUST retain the current revision. Every command MUST supply `expected_revision`; after exact replay lookup, a new stale command MUST fail without service execution. Two new commands submitted with the same expected revision MUST NOT both execute after one consumes that revision. Snapshot mutations MUST publish all related decision fields coherently at the same resulting revision.

A command's `result_revision` MUST be the revision returned by its synchronous outcome dispatch and MUST NOT be sampled from mutable global state after unrelated work can advance it. Later asynchronous scheduler progress MUST NOT rewrite the settled record.

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

#### Scenario: Failure is revision-idempotent

**Given**: a command fails validation, preparation, cancellation, or scheduler activation after reservation
**When**: its record settles
**Then**: no command side effect remains
**And**: `result_revision` equals the unchanged admitted revision
**And**: no state event is published for a mutation that did not commit

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
