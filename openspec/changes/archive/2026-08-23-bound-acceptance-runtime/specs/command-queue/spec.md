## ADDED Requirements

### Requirement: Acceptance commands have a dedicated absolute runtime limit

The common AI command runner MUST accept an Acceptance-specific absolute runtime limit supplied by orchestration. `acceptance_max_runtime_secs` MUST default to 1,800 seconds, accept values from 60 through 10,800 seconds, and reject zero. When `command_max_runtime_secs` is positive, Acceptance MUST use the minimum of the common and dedicated limits. When `command_max_runtime_secs` is zero, Acceptance MUST remain bounded by the dedicated limit. Acceptance output activity MUST NOT extend the deadline. Expiry MUST close retry admission for that invocation, terminate and prove quiescence for the owned process group through the existing cleanup path, and return a typed non-retryable Acceptance runtime failure. That failure MUST NOT enter no-verdict protocol continuation, corrective command-recovery retry, or inactivity-timeout classification. Other command classes MUST retain `command_max_runtime_secs` semantics.

#### Scenario: Acceptance uses the shorter default

**Given**: no configuration layer sets `acceptance_max_runtime_secs`
**And**: the common command runtime default is 10,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline is 1,800 seconds after successful child spawn

#### Scenario: Disabled common limit does not unbound Acceptance

**Given**: `command_max_runtime_secs` is zero
**And**: `acceptance_max_runtime_secs` is 1,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline remains 1,800 seconds after successful child spawn

#### Scenario: Shorter common safety limit still applies

**Given**: `command_max_runtime_secs` is 300 seconds
**And**: `acceptance_max_runtime_secs` is 1,800 seconds
**When**: Acceptance starts an owned command
**Then**: its absolute runtime deadline is 300 seconds after successful child spawn

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

#### Scenario: Runtime expiry does not enter other retry protocols

**Given**: Acceptance reaches its absolute runtime limit before producing a canonical verdict
**When**: the owned process group is terminated and reaped
**Then**: Conflux returns the typed Acceptance runtime failure
**And**: it does not enter no-verdict protocol continuation
**And**: it does not enter corrective command-recovery retry
**And**: it is not classified as inactivity timeout
