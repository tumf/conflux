## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept only `start`, `stop`, `cancel_stop`, `force_stop`, `set_execution_mark`, `set_queue_intent`, `retry_change`, `retry_errors`, `stop_and_dequeue`, `resolve_merge`, `set_parallel_mode`, and `set_all_execution_marks` until a later spec delta extends the enum. Every command MUST include `expected_revision` and `idempotency_key`. Accepted lifecycle commands MUST execute through the same process-local application services used by the TUI; the API MUST NOT equate internal channel enqueue with successful command execution and MUST NOT maintain an independent workflow state machine.

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

#### Scenario: Parallel toggle uses shared behavior

**Given**: The application is in Select or Stopped mode and parallel execution is available
**When**: `set_parallel_mode` is accepted
**Then**: The shared service changes the mode
**And**: It clears marks and queue presentation for changes that are ineligible in parallel mode
**And**: The outcome identifies excluded changes and reasons

#### Scenario: Bulk mark classifies one revision

**Given**: Eligible and excluded changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from eligible rows only
**And**: It updates eligible marks and Running queue intent atomically
**And**: It returns changed IDs and stable exclusion reasons

## ADDED Requirements

### Requirement: Remote parallel execution discovery

Capabilities and state MUST expose parallel execution availability, active mode, maximum concurrency, VCS backend, and per-change eligibility with machine-readable blocked reasons.

#### Scenario: Client discovers parallel execution state

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities and state
**Then**: It can distinguish sequential, available parallel, unavailable parallel, and active parallel modes
**And**: It can explain why each non-final change is or is not parallel-eligible without inspecting Git itself

### Requirement: Atomic parallel start eligibility

Parallel start MUST validate the complete marked target set at the admitted revision. If any marked target is ineligible, start MUST reject the complete operation without spawning a scheduler or partially changing queue state.

#### Scenario: One ineligible mark rejects start

**Given**: Two changes are marked and one is not parallel-eligible
**When**: Parallel start is submitted
**Then**: Neither change starts
**And**: The response identifies the ineligible target and reason
**And**: Marks and queue intent remain coherent
