## MODIFIED Requirements

### Requirement: Demo capability MUST validate input range

Updated wording: The demo capability MUST reject inputs outside the allowed range, providing a structured error response.

#### Scenario: Input below minimum is rejected with structured error

- **GIVEN** a demo capability instance is configured with a minimum value
- **WHEN** an input below the minimum is submitted
- **THEN** the capability returns a structured validation error with error code and human-readable message
