## ADDED Requirements

### Requirement: Acceptance CONTINUE retry configuration
The orchestrator SHALL support configuring the maximum number of acceptance CONTINUE retries via `acceptance_max_continues`.

#### Scenario: Default CONTINUE retry limit
- **WHEN** `acceptance_max_continues` is not set in config
- **THEN** the system uses a default limit of 2

#### Scenario: Configured CONTINUE retry limit
- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "acceptance_max_continues": 4
  }
  ```
- **WHEN** acceptance output indicates CONTINUE repeatedly
- **THEN** the orchestrator retries acceptance up to 4 times before treating it as FAIL
