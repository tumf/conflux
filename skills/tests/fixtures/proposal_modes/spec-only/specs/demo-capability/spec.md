## ADDED Requirements

### Requirement: Demo capability MUST validate input range

The demo capability MUST reject inputs outside the allowed range and return a clear error.

#### Scenario: Input below minimum is rejected

- **GIVEN** a demo capability instance is configured with a minimum value
- **WHEN** an input below the minimum is submitted
- **THEN** the capability returns a validation error
- **AND** the error message names the minimum allowed value
