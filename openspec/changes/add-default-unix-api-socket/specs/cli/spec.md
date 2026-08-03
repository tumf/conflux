## MODIFIED Requirements

### Requirement: Web Monitoring Flags

The CLI SHALL expose the browser-facing `--web` TCP listener and the default local Unix API listener as distinct controls. In web-enabled builds, default TUI, `tui`, and `run` SHALL use `${GIT_COMMON_DIR}/cflx-api.sock` unless `--web-unix-socket PATH` overrides it or `--no-web-unix-socket` disables it. The override and opt-out SHALL be mutually exclusive. `--web` SHALL add the retained TCP/Web UI listener without disabling UDS.

#### Scenario: Default UDS starts without web flag

- **GIVEN** a web-enabled build inside a Git repository
- **WHEN** the user starts default TUI, `cflx tui`, or `cflx run` without Unix socket flags
- **THEN** the API binds `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP Web UI listener starts unless `--web` is supplied

#### Scenario: Override default Unix path

- **WHEN** the user supplies `--web-unix-socket /run/user/1000/custom.sock`
- **THEN** the API binds that path instead of the Git common-directory default

#### Scenario: Disable default Unix listener

- **WHEN** the user supplies `--no-web-unix-socket`
- **THEN** no UDS listener starts
- **AND** local orchestration may continue

#### Scenario: Unix options are mutually exclusive

- **WHEN** the user supplies both `--web-unix-socket PATH` and `--no-web-unix-socket`
- **THEN** CLI parsing fails with an actionable conflict error

#### Scenario: Enable web monitoring alongside UDS

- **WHEN** the user runs with `--web`
- **THEN** the retained TCP server starts on the configured bind and actual port
- **AND** the default or explicit UDS remains active
- **AND** the TUI displays and encodes only the TCP Web UI URL as QR

#### Scenario: Configure TCP listener

- **WHEN** the user runs with `--web --web-bind 0.0.0.0 --web-port 3000` and valid required authentication
- **THEN** the TCP server accepts connections on port 3000 from the configured interface
- **AND** the UDS path remains controlled only by its default, override, or opt-out

#### Scenario: Non-Git invocation requires a decision

- **GIVEN** a web-enabled local orchestration invocation outside Git
- **WHEN** neither `--web-unix-socket PATH` nor `--no-web-unix-socket` is supplied
- **THEN** startup exits non-zero before orchestration
- **AND** the error explains both explicit choices

### Requirement: Repository Lock Conflict Diagnostics

A lock owner MUST publish best-effort diagnostic metadata containing its PID, start time, canonical workspace, invocation mode, and every API endpoint whose listener completed startup. Endpoint metadata MUST distinguish Unix and TCP addresses. Readers MUST accept legacy metadata containing only `api_url`. A conflicting invocation MUST display all valid available owner metadata, omit unavailable endpoints, and remain safe when metadata is missing or malformed. Endpoint metadata MUST NOT control lock ownership or workflow routing.

#### Scenario: Conflict reports default Unix endpoint

- **GIVEN** a process owns the repository lock
- **AND** its default Unix listener successfully bound
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic includes `unix://${GIT_COMMON_DIR}/cflx-api.sock`

#### Scenario: Conflict reports dual endpoints

- **GIVEN** a process successfully bound its UDS and `--web` TCP listeners
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic includes both actual endpoints
- **AND** an OS-assigned TCP port is reported when the owner requested port `0`

#### Scenario: Legacy single URL metadata remains readable

- **GIVEN** owner metadata was written by a prior version with only `api_url`
- **WHEN** a conflict diagnostic reads it
- **THEN** the legacy URL is reported as an available endpoint
- **AND** it does not affect OS lock ownership

#### Scenario: Partial startup publishes no endpoints

- **GIVEN** one requested listener binds but another requested listener fails during startup
- **WHEN** the process exits the failed startup transaction
- **THEN** owner metadata does not claim either endpoint as active
- **AND** any created listener and owned socket are cleaned up

#### Scenario: Malformed endpoint metadata does not control ownership

- **GIVEN** the repository lock is held but endpoint metadata is absent, incomplete, or malformed
- **WHEN** another invocation attempts startup
- **THEN** the second invocation is rejected because the OS lock is held
- **AND** the diagnostic reports only fields and endpoints that can be read safely
