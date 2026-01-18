## MODIFIED Requirements

### Requirement: REQ-OBS-003 Unified Log Format

The orchestrator MUST ensure error messages include actionable context such as operation type, change ID, and workspace or working directory when available.

#### Scenario: Error message includes execution context
- **GIVEN** an apply operation fails for change `alpha`
- **WHEN** the orchestrator records the error
- **THEN** the error message includes the operation type (`apply`) and change ID (`alpha`)
- **AND** the message includes the workspace or working directory when available
