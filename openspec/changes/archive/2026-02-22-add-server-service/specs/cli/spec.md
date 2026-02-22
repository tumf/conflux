## ADDED Requirements

### Requirement: service Subcommand Manages Background Server

The CLI SHALL provide a `service` subcommand group for managing `cflx server` as a background service.

Supported operations MUST include: `install`, `uninstall`, `status`, `start`, `stop`, `restart`.

#### Scenario: Service commands are discoverable
- **WHEN** a user runs `cflx service --help`
- **THEN** help text lists `install`, `uninstall`, `status`, `start`, `stop`, `restart`

#### Scenario: Service command rejects unknown operation
- **WHEN** a user runs `cflx service unknown`
- **THEN** the CLI reports an unknown subcommand error
