## ADDED Requirements

### Requirement: Version endpoint

The server API SHALL expose a version endpoint that returns the backend version string.

#### Scenario: Fetch version

- **WHEN** client sends `GET /api/v1/version`
- **THEN** response status is 200
- **AND** response body is JSON with a `version` field containing the backend version string (e.g. `"v0.5.24 (20260328)"`)

### Requirement: Dashboard version display

The server dashboard Header SHALL display the backend version string next to the logo text in a small, subdued style.

#### Scenario: Version displayed on load

- **GIVEN** the dashboard is loaded in a browser
- **WHEN** the Header component mounts
- **THEN** the backend version string is fetched and displayed next to "Conflux" in small muted text

#### Scenario: Version fetch failure

- **GIVEN** the version endpoint is unreachable
- **WHEN** the Header component mounts
- **THEN** no version text is displayed (no error shown to user)
