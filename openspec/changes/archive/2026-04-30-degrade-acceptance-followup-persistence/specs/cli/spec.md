## ADDED Requirements

### Requirement: CLI acceptance failure reporting distinguishes verdict failure from follow-up persistence degradation

When the CLI acceptance loop receives a non-pass acceptance verdict, it SHALL distinguish the acceptance diagnosis from any later persistence problem while recording follow-up tasks.

A failure to append findings into `tasks.md` MAY be reported as warning or supplemental execution context, but it MUST NOT replace the acceptance verdict as the primary reported reason unless the verdict itself could not be determined.

#### Scenario: CLI keeps acceptance fail as primary reason when persistence degrades
- **GIVEN** the acceptance command returns `FAIL` with concrete findings
- **AND** follow-up persistence into `tasks.md` later fails
- **WHEN** the CLI reports the acceptance result
- **THEN** the primary reported outcome is still acceptance `FAIL`
- **AND** the `tasks.md` persistence problem is reported separately as supplemental context
