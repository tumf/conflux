## ADDED Requirements
### Requirement: Merge Stall Monitoring Configuration

The orchestrator SHALL provide `merge_stall_detection` configuration for merge stall monitoring and allow configuration of threshold and monitoring interval.

#### Scenario: Specify monitoring configuration
- **GIVEN** `.cflx.jsonc` contains the following configuration:
  ```jsonc
  {
    "merge_stall_detection": {
      "enabled": true,
      "threshold_minutes": 30,
      "check_interval_seconds": 60
    }
  }
  ```
- **WHEN** the orchestrator is executed
- **THEN** merge stall monitoring is enabled
- **AND** the threshold is treated as 30 minutes
- **AND** the monitoring interval is treated as 60 seconds

#### Scenario: Use defaults when monitoring configuration is unspecified
- **GIVEN** `merge_stall_detection` is not configured
- **WHEN** the orchestrator is executed
- **THEN** merge stall monitoring is evaluated with default values
