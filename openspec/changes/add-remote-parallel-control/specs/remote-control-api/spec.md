## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST accept the current lifecycle and change commands plus `set_parallel_mode` and `set_all_execution_marks`. Every command MUST include `expected_revision` and `idempotency_key`. Accepted commands MUST delegate to the shared operator command service; the API MUST NOT maintain an independent workflow state machine.

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

### Requirement: Versioned single-instance remote-control resources

Single-instance web monitoring MUST expose `/api/v2` health, capabilities, instance, state, changes, logs, command, event, and WebSocket resources. Capabilities and state MUST expose parallel execution availability, active mode, maximum concurrency, VCS backend, and per-change eligibility with machine-readable blocked reasons.

#### Scenario: Client discovers parallel execution state

**Given**: A single cflx process has web monitoring enabled
**When**: A client reads capabilities and state
**Then**: It can distinguish sequential, available parallel, unavailable parallel, and active parallel modes
**And**: It can explain why each non-final change is or is not parallel-eligible without inspecting Git itself

## ADDED Requirements

### Requirement: Atomic parallel start eligibility

Parallel start MUST validate the complete marked target set at the admitted revision. If any marked target is ineligible, start MUST reject the complete operation without spawning a scheduler or partially changing queue state.

#### Scenario: One ineligible mark rejects start

**Given**: Two changes are marked and one is not parallel-eligible
**When**: Parallel start is submitted
**Then**: Neither change starts
**And**: The response identifies the ineligible target and reason
**And**: Marks and queue intent remain coherent
