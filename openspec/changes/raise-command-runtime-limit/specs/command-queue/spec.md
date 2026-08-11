## MODIFIED Requirements

### Requirement: AI command invocations have an absolute runtime limit

The common AI command runner MUST enforce `command_max_runtime_secs` as an absolute deadline measured from successful child spawn. The default MUST be 10,800 seconds, `0` MUST disable the deadline, and stdout or stderr activity MUST NOT extend it. Runtime-limit expiry MUST close retry admission for the invocation, terminate the owned process group through the existing graceful-then-forceful cleanup path, and return a typed non-retryable runtime-limit outcome.

#### Scenario: Default runtime limit is three hours

**Given**: no configuration layer sets `command_max_runtime_secs`
**When**: the common AI command runner starts an owned command
**Then**: its absolute runtime deadline is 10,800 seconds after successful child spawn

#### Scenario: Continuous output does not extend the absolute deadline

**Given**: `command_max_runtime_secs` is enabled
**And**: an owned AI command emits output continuously
**When**: elapsed time from child spawn reaches the configured limit
**Then**: Conflux closes retry admission for the invocation
**And**: Conflux terminates and proves quiescence for the owned process group
**And**: the command is not automatically retried in the same run

#### Scenario: Zero disables the absolute deadline

**Given**: `command_max_runtime_secs` is `0`
**When**: an owned AI command remains active while satisfying all other lifecycle constraints
**Then**: Conflux does not terminate it solely because of total elapsed runtime
**And**: inactivity timeout and explicit cancellation remain independently enforceable

#### Scenario: Cleanup proof is required after runtime expiry

**Given**: an AI command exceeds its absolute runtime limit
**When**: bounded process-group cleanup cannot prove quiescence
**Then**: Conflux returns actionable cleanup diagnostics
**And**: it does not acknowledge successful termination
**And**: no later retry is admitted for that invocation
