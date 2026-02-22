## ADDED Requirements

### Requirement: Service Start Enforces Server Security Validation

Service operations that start or restart the server MUST validate the effective server configuration using the same policy as `cflx server`.

#### Scenario: Non-loopback bind without bearer token fails before start
- **GIVEN** the effective `server.bind` is non-loopback
- **AND** the effective authentication mode is not a valid bearer-token configuration
- **WHEN** a user runs `cflx service start`
- **THEN** the command fails with an error
- **AND** the service is not started
