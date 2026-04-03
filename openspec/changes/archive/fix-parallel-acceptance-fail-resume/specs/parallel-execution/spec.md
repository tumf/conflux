## MODIFIED Requirements

### Requirement: Acceptance failure returns to apply loop

When acceptance returns FAIL, the parallel dispatch loop MUST re-enter the apply step on the next cycle, regardless of how the workspace was initially routed (fresh start or resume).

#### Scenario: Resumed workspace acceptance failure triggers apply retry

- **GIVEN** a parallel workspace resumed with state `Applied` (routed to acceptance-only on first cycle)
- **WHEN** the acceptance step returns `ACCEPTANCE: FAIL`
- **THEN** the next cycle of the apply+acceptance loop MUST execute the apply step before running acceptance again
