## MODIFIED Requirements

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, use queue intent for ordinary Running changes, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Parallel mode changes MUST classify one coherent state, clear marks and queue presentation for newly ineligible changes, and report stable exclusion reasons. Bulk execution-mark mutation MUST choose one target state from eligible rows only and update eligible marks plus Running queue intent atomically.

#### Scenario: Dependency-blocked addition preserves queue intent

**Given**: A Running change has unresolved dependencies
**When**: The operator adds it to the queue
**Then**: Queue intent is retained
**And**: Its display status is `blocked`
**And**: The service does not expose `gated`

#### Scenario: Error mode requires retry

**Given**: The application is in Error mode
**When**: The operator requests execution-mark mutation
**Then**: The request is rejected without state change
**And**: `retry_change` or `retry_errors` remains the supported action

#### Scenario: Parallel mode change cleans ineligible intent

**Given**: The application is in Select or Stopped mode and marked changes become ineligible in parallel mode
**When**: The operator enables parallel mode
**Then**: The service clears those execution marks and queue presentation atomically
**And**: The outcome identifies each excluded change and reason

#### Scenario: Bulk mark updates one coherent target set

**Given**: Eligible and excluded changes exist in one admitted state
**When**: The operator requests bulk execution-mark mutation
**Then**: The service derives one target mark from eligible changes only
**And**: It updates eligible marks and Running queue intent atomically
**And**: Excluded changes retain coherent intent and receive stable reasons
