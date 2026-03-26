## ADDED Requirements

### Requirement: Demo capability MUST validate input range at runtime

The demo capability MUST enforce range validation both in specification and at runtime.

#### Scenario: Runtime and spec agree on minimum value rejection

- **GIVEN** the canonical spec defines a minimum value of 1
- **WHEN** the runtime receives an input of 0
- **THEN** a `ValidationError` is raised consistent with the spec scenario
