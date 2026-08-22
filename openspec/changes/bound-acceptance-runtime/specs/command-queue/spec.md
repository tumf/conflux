## ADDED Requirements

### Requirement: Acceptance commands have a dedicated absolute runtime limit

The common AI command runner MUST accept an Acceptance-specific absolute runtime limit supplied by orchestration. `acceptance_max_runtime_secs` MUST default to 1,800 seconds, accept values from 60 through 10,800 seconds, and reject zero. Acceptance output activity MUST NOT extend the deadline. Expiry MUST close retry admission for that invocation, terminate and prove quiescence for the owned process group through the existing cleanup path, and return a typed non-retryable Acceptance runtime failure. Other command classes MUST retain `command_max_runtime_secs` semantics.

#### Scenario: Acceptance uses the shorter default

**Given**: no configuration layer sets `acceptance_max_runtime_secs`
**And**: the common command runtime default is 10,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline is 1,800 seconds after successful child spawn

#### Scenario: Output does not extend Acceptance runtime

**Given**: an Acceptance command continuously emits output
**When**: elapsed time reaches `acceptance_max_runtime_secs`
**Then**: retry admission closes
**And**: the owned process group is terminated and reaped
**And**: the invocation returns a typed non-retryable Acceptance runtime failure

#### Scenario: Acceptance deadline cannot be disabled

**Given**: configuration sets `acceptance_max_runtime_secs` to zero or outside 60 through 10,800 seconds
**When**: configuration is loaded
**Then**: loading fails with an actionable range diagnostic

#### Scenario: Other command limits remain unchanged

**Given**: Acceptance and common command runtime limits differ
**When**: Apply and Acceptance commands start
**Then**: Acceptance receives the dedicated limit
**And**: Apply retains `command_max_runtime_secs`

#### Scenario: Cleanup failure is not hidden

**Given**: Acceptance exceeds its absolute runtime limit
**When**: process-group quiescence cannot be proven
**Then**: Conflux reports actionable cleanup failure diagnostics
**And**: it does not acknowledge termination or retry the invocation
