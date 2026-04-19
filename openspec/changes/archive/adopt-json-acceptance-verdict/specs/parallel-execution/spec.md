## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse machine-readable acceptance output to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.

When a strict JSON acceptance verdict object has already been observed for the current acceptance execution, the runtime MAY complete the acceptance operation based on that verdict without waiting for additional trailing output or eventual inactivity timeout. Legacy plain-text standalone verdict lines such as `ACCEPTANCE: PASS` MAY remain supported as a fallback only when no JSON verdict object is present.

#### Scenario: strict JSON verdict completes acceptance

- **GIVEN** an acceptance command emits a strict JSON verdict object indicating `pass`
- **WHEN** the runtime processes streaming stdout for that acceptance execution
- **THEN** the acceptance result is finalized as PASS for the current revision
- **AND** archive handoff may proceed without requiring a legacy text verdict line

#### Scenario: JSON event stream text payload is normalized into verdict

- **GIVEN** an acceptance command emits JSON event lines where the final assistant text payload contains a strict JSON verdict object
- **WHEN** the runtime evaluates the streaming output
- **THEN** it extracts and normalizes that payload into the canonical acceptance verdict
- **AND** the verdict result matches the non-event-stream JSON contract

#### Scenario: legacy standalone marker remains fallback only

- **GIVEN** an acceptance command emits no strict JSON verdict object
- **AND** it emits a standalone line exactly equal to `ACCEPTANCE: PASS`
- **WHEN** the runtime evaluates canonical acceptance verdicts
- **THEN** that legacy marker is still accepted as PASS
- **AND** it is treated as fallback behavior rather than the primary contract
