## ADDED Requirements
### Requirement: Parallel Mode Default at Startup
TUI は起動時に parallel モードの既定値を決定しなければならない（SHALL）。`parallel_mode` が設定されていればその値を優先し、未設定の場合は Git 検知に基づいて既定値を決定する。

#### Scenario: 設定未指定かつGit検知でparallelが既定ON
- **GIVEN** user starts the TUI
- **AND** config file does not contain `"parallel_mode"` key
- **AND** a `.git` directory exists in the current working directory
- **THEN** parallel mode is enabled by default

#### Scenario: 設定で無効化されている場合はparallelが既定OFF
- **GIVEN** user starts the TUI
- **AND** config file contains `"parallel_mode": false`
- **THEN** parallel mode is disabled by default

#### Scenario: 設定で有効化されている場合はparallelが既定ON
- **GIVEN** user starts the TUI
- **AND** config file contains `"parallel_mode": true`
- **AND** a `.git` directory exists in the current working directory
- **THEN** parallel mode is enabled by default

#### Scenario: Git未検知でparallelが既定OFF
- **GIVEN** user starts the TUI
- **AND** config file does not contain `"parallel_mode"` key
- **AND** no `.git` directory exists in the current working directory
- **THEN** parallel mode is disabled by default
