## ADDED Requirements

### Requirement: UTC Build Number Generation

The release workflow SHALL generate a UTC build number in `YYYYMMDDHHmmss` format during build packaging.

#### Scenario: Build number generation format
- **WHEN** a release build is executed
- **THEN** the build number is generated from the current UTC time
- **AND** the build number format matches `YYYYMMDDHHmmss`
