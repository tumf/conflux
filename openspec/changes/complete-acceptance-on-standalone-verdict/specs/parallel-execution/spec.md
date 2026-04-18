## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.

When a canonical standalone acceptance verdict line has already been observed for the current acceptance execution, the runtime MAY complete the acceptance operation based on that verdict without waiting for additional trailing output or eventual inactivity timeout. The runtime MUST NOT allow malformed trailing-text verdict strings such as `ACCEPTANCE: PASSAll ...` or `ACCEPTANCE: PASS## ...` to satisfy the canonical PASS condition.

#### Scenario: standalone PASS completes acceptance before process stall timeout

- **GIVEN** an acceptance command emits a standalone line exactly equal to `ACCEPTANCE: PASS`
- **AND** the child process continues running without producing further useful output
- **WHEN** the runtime processes streaming stdout for that acceptance execution
- **THEN** the acceptance result is finalized as PASS for the current revision
- **AND** archive handoff may proceed without waiting for inactivity timeout

#### Scenario: trailing-text PASS does not satisfy canonical verdict

- **GIVEN** an acceptance command emits `ACCEPTANCE: PASSAll checks completed`
- **WHEN** the runtime evaluates canonical acceptance verdicts
- **THEN** that line is not treated as a canonical PASS verdict
- **AND** the runtime requires a valid standalone verdict or another terminal outcome before completing acceptance

#### Scenario: heading-concatenated PASS does not satisfy canonical verdict

- **GIVEN** an acceptance command emits `ACCEPTANCE: PASS## Acceptance Review Summary`
- **WHEN** the runtime evaluates canonical acceptance verdicts
- **THEN** that line is not treated as a canonical PASS verdict
- **AND** the runtime requires a valid standalone verdict or another terminal outcome before completing acceptance
