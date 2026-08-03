## MODIFIED Requirements

### Requirement: Web Monitoring Flags

On Linux and macOS web-enabled builds, the CLI SHALL expose a default local Unix API listener and retain `--web` as the browser-facing TCP/Web UI control. Default TUI, `tui`, and `run` SHALL use `${GIT_COMMON_DIR}/cflx-api.sock` unless `--web-unix-socket PATH` overrides it or `--no-web-unix-socket` disables it. The override and opt-out SHALL be mutually exclusive. `--web` SHALL add TCP without disabling UDS. Non-Unix builds SHALL remain TCP-only and SHALL NOT advertise Unix-only flags.

#### Scenario: Enable web monitoring
- **WHEN** user runs with `--web` flag
- **THEN** the retained TCP HTTP/Web UI server starts on the configured bind and actual port
- **AND** the selected Unix listener remains active unless opted out

#### Scenario: Configure web port
- **WHEN** user runs with `--web --web-port 3000`
- **THEN** TCP HTTP server starts on port 3000 instead of the default auto-assigned port

#### Scenario: Configure bind address
- **WHEN** user runs with `--web --web-bind 0.0.0.0` and required non-empty authentication
- **THEN** TCP HTTP server accepts connections from any network interface
- **AND** warning is logged about exposing server to network

#### Scenario: Web flags without --web
- **WHEN** user runs with `--web-port 3000` but without CLI/config TCP web enablement
- **THEN** TCP HTTP server does not start
- **AND** web-port does not alter the default Unix listener

#### Scenario: Invalid port number
- **WHEN** user runs with `--web --web-port 99999`
- **THEN** error message is displayed about invalid port range
- **AND** orchestrator exits with non-zero status before effectful startup

#### Scenario: Web monitoring in TUI mode
- **WHEN** user runs TUI mode with `--web` flag
- **THEN** TCP HTTP server starts in background in addition to selected UDS
- **AND** TUI displays message indicating web server is running
- **AND** TUI shows and QR-encodes the actual TCP Web UI URL

#### Scenario: Web monitoring in run mode
- **WHEN** user runs `cflx run --web`
- **THEN** selected UDS and TCP HTTP server start before effectful orchestration startup
- **AND** actual endpoints are logged/published
- **AND** orchestration proceeds normally

#### Scenario: Default Unix API without web flag
- **GIVEN** a Linux or macOS web-enabled build inside Git
- **WHEN** user starts default TUI, `cflx tui`, or `cflx run` without Unix options or `--web`
- **THEN** `/api/v2` binds `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP Web UI listener starts

#### Scenario: Override Unix API path
- **WHEN** user supplies `--web-unix-socket /trusted/cflx.sock`
- **THEN** the API binds that validated absolute path instead of the repository default

#### Scenario: Disable Unix API
- **WHEN** user supplies `--no-web-unix-socket`
- **THEN** no UDS listener starts
- **AND** local orchestration and optional TCP Web UI may continue

#### Scenario: Unix options conflict
- **WHEN** user supplies both `--web-unix-socket PATH` and `--no-web-unix-socket`
- **THEN** CLI parsing fails with an actionable conflict error

#### Scenario: Non-Git invocation requires explicit choice
- **GIVEN** a Linux or macOS web-enabled orchestration invocation outside Git
- **WHEN** neither explicit Unix path nor opt-out is supplied
- **THEN** startup exits non-zero before effectful work
- **AND** the error explains both choices

#### Scenario: Non-Unix CLI is TCP-only
- **GIVEN** a non-Unix build
- **WHEN** user requests help
- **THEN** Unix-only flags are absent
- **AND** retained TCP flags remain available when web monitoring is compiled

### Requirement: Repository Lock Conflict Diagnostics

A lock owner MUST publish best-effort diagnostic metadata containing its PID, start time, canonical workspace, invocation mode, and successfully started API endpoints. New metadata MUST use ordered `api_endpoints` entries with closed `transport` values `unix|tcp` and an `address`; Unix entries precede TCP entries. The legacy `api_url` field MUST be written only when TCP exists. New readers MUST prefer valid endpoint entries, deduplicate matching legacy data, and fall back to valid legacy `api_url`. A conflicting invocation MUST display all valid available owner metadata, omit unavailable endpoints, and remain safe when metadata is missing or malformed. Metadata MUST NOT determine ownership or workflow routing.

#### Scenario: Conflict reports an active API endpoint
- **GIVEN** a process owns the repository lock
- **AND** one or more API listeners completed the startup transaction
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the diagnostic includes owner PID, mode, start time, canonical workspace, and every valid endpoint
- **AND** an OS-assigned TCP port is reported when requested port was `0`

#### Scenario: Conflict before API bind omits endpoint
- **GIVEN** a process owns the repository lock
- **AND** no API startup transaction completed
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic identifies the owner from valid available metadata
- **AND** the diagnostic does not claim an API endpoint

#### Scenario: Malformed metadata does not control ownership
- **GIVEN** the repository lock is held but diagnostic metadata is absent, incomplete, or malformed
- **WHEN** another local orchestration-owning invocation attempts startup
- **THEN** the second invocation is rejected because the OS lock is held
- **AND** the diagnostic reports only fields/endpoints that can be read safely

#### Scenario: Stale metadata does not block startup
- **GIVEN** diagnostic metadata remains from a previous process
- **AND** no process holds the OS lock
- **WHEN** local orchestration starts
- **THEN** it acquires the lock and replaces stale diagnostic metadata
- **AND** previous PID or endpoint metadata does not affect workflow routing
- **AND** a pre-existing Unix socket path is still handled independently by the fail-closed path rule

#### Scenario: Dual endpoint metadata preserves old readers
- **GIVEN** UDS and TCP listeners completed startup
- **WHEN** owner metadata is written
- **THEN** `api_endpoints` contains Unix then TCP
- **AND** `api_url` contains the same actual TCP URL for old readers

#### Scenario: UDS-only metadata degrades safely for old readers
- **GIVEN** only UDS completed startup
- **WHEN** owner metadata is written
- **THEN** `api_endpoints` contains the absolute Unix path
- **AND** `api_url` is omitted
- **AND** an old reader safely reports no API URL

#### Scenario: New reader accepts legacy and mixed metadata
- **GIVEN** metadata contains only legacy `api_url` or contains both schemas
- **WHEN** a new reader parses it
- **THEN** it preserves valid endpoints in defined order
- **AND** it deduplicates a matching legacy TCP URL

#### Scenario: Partial listener startup publishes no endpoint
- **GIVEN** one requested listener binds and another fails
- **WHEN** startup rolls back
- **THEN** metadata claims no listener from the failed transaction
- **AND** created listener resources and owned socket are cleaned up
