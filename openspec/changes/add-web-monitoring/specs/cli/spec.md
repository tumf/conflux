## ADDED Requirements

### Requirement: Web Monitoring Flags

The CLI SHALL support flags to enable and configure web-based monitoring.

#### Scenario: Enable web monitoring
- **WHEN** user runs with `--web` flag
- **THEN** HTTP server starts for web monitoring
- **AND** server binds to default port 8080 on 127.0.0.1

#### Scenario: Configure web port
- **WHEN** user runs with `--web --web-port 3000`
- **THEN** HTTP server starts on port 3000 instead of default

#### Scenario: Configure bind address
- **WHEN** user runs with `--web --web-bind 0.0.0.0`
- **THEN** HTTP server accepts connections from any network interface
- **AND** warning is logged about exposing server to network

#### Scenario: Web flags without --web
- **WHEN** user runs with `--web-port 3000` but without `--web` flag
- **THEN** HTTP server does not start
- **AND** web-port flag is ignored

#### Scenario: Invalid port number
- **WHEN** user runs with `--web --web-port 99999`
- **THEN** error message is displayed about invalid port range
- **AND** orchestrator exits with non-zero status

#### Scenario: Web monitoring in TUI mode
- **WHEN** user runs TUI mode with `--web` flag
- **THEN** HTTP server starts in background
- **AND** TUI displays message indicating web server is running
- **AND** TUI shows web server URL (e.g., "Web monitoring: http://127.0.0.1:8080")

#### Scenario: Web monitoring in run mode
- **WHEN** user runs `openspec-orchestrator run --web`
- **THEN** HTTP server starts before orchestration begins
- **AND** server URL is logged to console
- **AND** orchestration proceeds normally
