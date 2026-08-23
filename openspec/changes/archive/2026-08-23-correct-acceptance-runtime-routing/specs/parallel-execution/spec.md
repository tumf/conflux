## MODIFIED Requirements

### Requirement: Acceptance command failures MUST use bounded Acceptance-only recovery

Managed-worktree execution MUST treat configured Acceptance command launch or execution failure as Acceptance-only recovery on the same applied clean workspace. It MUST allow at most two retries after the initial failure and return terminal error after the third consecutive failure. This counter MUST remain independent from protocol correction, explicit CONTINUE, FAIL-to-Apply cycles, and the outer cycle ceiling; retry MUST NOT rerun Apply or cleanup-review.

That bounded recovery MUST NOT apply when process termination is classified as the dedicated Acceptance runtime limit. Runtime-limit expiry MUST produce a typed terminal run outcome after owned-process cleanup is proven. It MUST NOT increment the consecutive Acceptance command-failure counter, set corrective command-recovery context, enter missing-verdict or Acceptance retry cycles, or return to the Apply loop. Operator-triggered recovery remains explicit. Owner restart MAY recompute Acceptance from repository state with a fresh runtime budget.

#### Scenario: Acceptance command recovers without rerunning Apply

- **GIVEN** Acceptance command fails after command-queue retry on an applied clean managed workspace
- **AND** the dedicated command-failure budget remains
- **WHEN** runtime handles the failure
- **THEN** it passes bounded diagnostics to the next Acceptance invocation
- **AND** it reruns only the configured Acceptance command

#### Scenario: Runtime limit bypasses command recovery

**Given**: an Acceptance command reaches its effective absolute runtime limit
**And**: owned-process cleanup is proven
**When**: Acceptance classifies the termination
**Then**: the consecutive command-failure count remains unchanged
**And**: no corrective command-recovery context is created

#### Scenario: Runtime limit does not return to Apply

**Given**: Acceptance terminates with the typed runtime-limit outcome
**When**: parallel dispatch handles the outcome
**Then**: the run reaches a terminal actionable state
**And**: dispatch does not schedule Apply, missing-verdict continuation, or another Acceptance invocation

#### Scenario: Shorter common limit remains authoritative

**Given**: `command_max_runtime_secs` is 30 seconds
**And**: `acceptance_max_runtime_secs` is 1,800 seconds
**When**: normal Acceptance starts
**Then**: its effective absolute runtime limit is 30 seconds
**And**: the dedicated configuration floor is not treated as a clamp

#### Scenario: Cleanup review retains common semantics

**Given**: cleanup-review uses the configured Acceptance agent command
**When**: the runner selects a runtime limit for cleanup-review
**Then**: it uses `command_max_runtime_secs`
**And**: it does not receive the dedicated Acceptance limit solely because the same agent command is used
