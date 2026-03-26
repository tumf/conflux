## ADDED Requirements

### Requirement: Demo capability runtime MUST enforce range validation

The demo capability runtime implementation MUST reject inputs outside the allowed range.

#### Scenario: Runtime rejects below-minimum input

- **GIVEN** the demo capability is initialized with a minimum value of 1
- **WHEN** an input of 0 is passed at runtime
- **THEN** a `ValidationError` is raised
