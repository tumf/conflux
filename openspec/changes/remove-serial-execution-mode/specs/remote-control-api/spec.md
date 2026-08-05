## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, `resolve_merge`, and `set_all_execution_marks` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted lifecycle commands MUST execute through the same process-local application services used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution and MUST NOT maintain an independent workflow state machine.

#### Scenario: Accepted command uses shared behavior

**Given**: A command variant is valid for the current lifecycle state
**When**: The API accepts it
**Then**: The shared application service revalidates and executes it
**And**: TUI-equivalent reducer, scheduler, cancellation, side-effect, and event semantics apply

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
