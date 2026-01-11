## ADDED Requirements

### Requirement: Web Monitoring Configuration

The configuration file SHALL support web monitoring settings to control HTTP server behavior.

#### Scenario: Enable web monitoring via config
- **WHEN** config file contains `web.enabled = true`
- **THEN** HTTP server starts automatically without `--web` CLI flag
- **AND** server uses configured port and bind address

#### Scenario: Configure web port in config file
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "enabled": true,
      "port": 9000
    }
  }
  ```
- **THEN** HTTP server binds to port 9000

#### Scenario: Configure bind address in config file
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "enabled": true,
      "bind": "0.0.0.0"
    }
  }
  ```
- **THEN** HTTP server accepts connections from any network interface

#### Scenario: CLI flags override config file
- **WHEN** config file has `web.port = 8080`
- **AND** user runs with `--web-port 3000` CLI flag
- **THEN** HTTP server binds to port 3000 (CLI takes precedence)

#### Scenario: Web disabled in config
- **WHEN** config file contains `web.enabled = false` or omits web section
- **THEN** HTTP server does not start unless `--web` CLI flag is provided

#### Scenario: Partial web configuration
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "port": 9000
    }
  }
  ```
- **AND** `enabled` field is omitted
- **THEN** web monitoring is disabled by default
- **AND** port setting is used only if `--web` CLI flag is provided

#### Scenario: Invalid port in config file
- **WHEN** config file contains `web.port = 99999` (out of valid range)
- **THEN** error message is displayed on startup
- **AND** orchestrator exits with non-zero status

#### Scenario: Default values when web enabled without specific settings
- **WHEN** config file contains only `web.enabled = true`
- **THEN** HTTP server uses default port 8080
- **AND** HTTP server uses default bind address 127.0.0.1
