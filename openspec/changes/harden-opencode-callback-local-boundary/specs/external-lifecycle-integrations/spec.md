## ADDED Requirements

### Requirement: Reference OpenCode callback enforces a local operating-system trust boundary

The reference OpenCode auto-resume integration MUST accept only literal IPv4 or IPv6 loopback HTTP destinations and MUST reject hostnames, including `localhost`, before opening a connection. Callback deduplication state MUST be private to the invoking operating-system user. A configured state path MUST be a real directory rather than a symlink, MUST be owned by the current user where the platform exposes ownership, and MUST have owner-only permissions. The plugin MUST register a sink only from a supported versioned enqueue envelope with a successful admission outcome and non-empty string binding identifiers.

#### Scenario: Hostname spelling cannot assert loopback

- **GIVEN** the OpenCode server is configured as `http://localhost:<port>` or another hostname
- **WHEN** the callback or plugin validates the destination
- **THEN** it rejects before opening a connection
- **AND** literal `127.0.0.1` and `[::1]` remain valid

#### Scenario: Pre-created state cannot control delivery

- **GIVEN** the configured or default callback state path is a symlink, a non-directory, is owned by another user where ownership is available, or permits group or world access
- **WHEN** the callback attempts to claim an execution event
- **THEN** it rejects before reading or creating `.inflight` or `.done` records
- **AND** it does not send an OpenCode request

#### Scenario: Private state supports normal deduplication

- **GIVEN** the callback state directory is a real owner-owned directory with mode `0700`
- **WHEN** a valid completion event is delivered
- **THEN** claim and successful-delivery records operate with the existing retry and deduplication semantics

#### Scenario: Incompatible enqueue envelope fails closed

- **GIVEN** an enqueue tool result has an unsupported schema version, a non-admission outcome, or a missing, empty, or non-string `change_id`, `execution_id`, or `instance_id`
- **WHEN** the OpenCode plugin extracts the execution binding
- **THEN** it returns no binding
- **AND** it does not register a completion sink
