## MODIFIED Requirements

### Requirement: HTTP Server Lifecycle

When the `web-monitoring` feature is compiled, every local default TUI, `cflx tui`, and `cflx run` orchestration-owning process SHALL start its process-scoped single-instance monitoring and remote-control endpoint on a Unix domain socket before orchestration-side effects. The default socket SHALL be `${GIT_COMMON_DIR}/cflx-api.sock`; an explicit socket path SHALL override it and an explicit opt-out SHALL disable it. `--web` SHALL additionally start the retained TCP HTTP/Web UI listener against the same application state. The system SHALL NOT provide a standalone multi-project server daemon. Listener startup MUST NOT eagerly generate or serialize optional OpenAPI documentation before the first TUI render; documentation resources SHALL be generated when requested without delaying listener availability.

#### Scenario: Default local API starts without web flag

- **GIVEN** a web-enabled build running inside a Git repository
- **WHEN** the user starts default TUI, `cflx tui`, or `cflx run` without `--web`
- **THEN** `/api/v2` is served on `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP port is bound for web monitoring
- **AND** orchestration begins only after the socket is usable

#### Scenario: OpenAPI generation is deferred from startup

- **GIVEN** a web-enabled build with the default Unix listener enabled
- **WHEN** the user starts the TUI and no client requests an OpenAPI or Swagger UI resource
- **THEN** router and listener startup do not generate or serialize the OpenAPI document
- **AND** the first TUI render is not delayed by optional documentation generation
- **AND** the Unix listener remains usable before orchestration begins

#### Scenario: Documentation remains available on demand

- **GIVEN** the local API listener has started without eager OpenAPI generation
- **WHEN** an authorized client requests `/api/v2/openapi.json`, `/api/v2/openapi.yaml`, or `/api/v2/docs`
- **THEN** the requested documentation resource is served successfully
- **AND** JSON and YAML represent the canonical generated `/api/v2` contract
- **AND** Swagger UI references the existing OpenAPI JSON path

#### Scenario: Web flag adds TCP without replacing UDS

- **GIVEN** the default Unix listener is enabled
- **WHEN** the user starts local orchestration with `--web`
- **THEN** UDS and TCP listeners both serve the same process-scoped app and state
- **AND** the embedded browser console remains available through TCP

#### Scenario: Required Unix listener fails before orchestration

- **WHEN** the selected Unix socket cannot be safely prepared, bound, or restricted to the required permissions
- **THEN** startup exits non-zero
- **AND** lifecycle adapters, AI subprocesses, and orchestration do not start
- **AND** any listener created during the failed startup transaction is stopped

#### Scenario: Finite run cleans up listeners

- **GIVEN** `cflx run` started the default Unix listener and optional TCP listener
- **WHEN** orchestration reaches terminal completion
- **THEN** all listener and refresh tasks stop without another external signal
- **AND** the Unix socket entry created by that process is removed

#### Scenario: Feature-disabled build remains API-free

- **GIVEN** Conflux is compiled without `web-monitoring`
- **WHEN** a local TUI or run invocation starts
- **THEN** it retains existing API-free behavior
- **AND** the default Unix socket contract is not applied

## ADDED Requirements

### Requirement: Local API startup regression budget

The repository SHALL maintain a real-process regression check for local API startup. The check MUST compare the default Unix-listener path with an API-free control path and MUST use a relative, variance-tolerant bound rather than one host's absolute elapsed time. The check MUST fail when optional API documentation initialization introduces a material fixed delay before the first TUI output.

#### Scenario: Default UDS startup stays near the control path

- **GIVEN** the same built `cflx` binary and repository fixture
- **WHEN** the regression check measures first terminal output for default TUI startup and for `--no-web-unix-socket`
- **THEN** the default UDS path remains within the repository-defined relative overhead bound
- **AND** both paths reach real TUI output rather than a mocked router function

#### Scenario: Timing variance does not create a brittle test

- **GIVEN** normal scheduler and filesystem variance on supported CI hosts
- **WHEN** individual startup samples fluctuate
- **THEN** the regression check uses repeated samples or an equivalent variance-tolerant comparison
- **AND** it does not require the investigation host's absolute startup time
