## ADDED Requirements

### Requirement: User-level TUI config

Conflux SHALL support a TUI-only JSONC user preference file named `tui.jsonc` under the global `cflx` config directory.

The TUI config SHALL be independent from orchestration configuration and SHALL NOT require orchestration command fields such as `apply_command`, `archive_command`, `analyze_command`, `acceptance_command`, or `resolve_command`.

TUI config loading SHALL consider global candidates in low-to-high priority order:

1. `dirs::config_dir()/cflx/tui.jsonc`
2. `~/.config/cflx/tui.jsonc`
3. `$XDG_CONFIG_HOME/cflx/tui.jsonc`

Project `.cflx.jsonc` files SHALL NOT override TUI config values.

#### Scenario: Missing TUI config uses defaults

- **GIVEN** no `tui.jsonc` exists in any global candidate path
- **WHEN** `cflx tui` starts
- **THEN** TUI config loading succeeds
- **AND** the resolved start keybinding is `F5`

#### Scenario: TUI config does not require orchestration commands

- **GIVEN** `~/.config/cflx/tui.jsonc` contains only:
  ```jsonc
  {
    "keybindings": {
      "start": ["F5", "r"]
    }
  }
  ```
- **WHEN** TUI config is loaded
- **THEN** loading succeeds without requiring `apply_command`, `archive_command`, `analyze_command`, `acceptance_command`, or `resolve_command`
- **AND** the resolved start keybindings are `F5` and `r`

#### Scenario: XDG environment path overrides default XDG path

- **GIVEN** `~/.config/cflx/tui.jsonc` sets `keybindings.start` to `["F5", "r"]`
- **AND** `$XDG_CONFIG_HOME/cflx/tui.jsonc` sets `keybindings.start` to `["F6"]`
- **WHEN** TUI config is loaded with `XDG_CONFIG_HOME` set
- **THEN** the resolved start keybinding is `F6`

#### Scenario: Project config does not override TUI keybindings

- **GIVEN** `~/.config/cflx/tui.jsonc` sets `keybindings.start` to `["F5", "r"]`
- **AND** `.cflx.jsonc` contains a TUI-like keybinding field
- **WHEN** `cflx tui` starts in that project
- **THEN** the resolved start keybindings remain `F5` and `r`
- **AND** `.cflx.jsonc` does not affect TUI keybindings

### Requirement: TUI start keybinding validation

The TUI config SHALL validate `keybindings.start` before entering the interactive TUI.

When omitted, `keybindings.start` SHALL default to `["F5"]`.

When present, `keybindings.start` SHALL contain at least one key and SHALL reject unknown or duplicate key entries with actionable errors.

The MVP supported key names SHALL include:

- function keys `F1` through `F12`
- named keys `Esc`, `Enter`, `Space`, `Tab`, `PageUp`, `PageDown`, `Home`, `End`, `Up`, `Down`, `Left`, and `Right`
- single-character keys such as `r`, `R`, `x`, and `m`

Unsupported modifier syntax such as `Ctrl+R` SHALL be rejected unless modifier support is implemented with validation and tests.

#### Scenario: Empty start binding is rejected

- **GIVEN** `tui.jsonc` contains `"keybindings": { "start": [] }`
- **WHEN** TUI config is loaded
- **THEN** loading fails
- **AND** the error identifies `keybindings.start` as requiring at least one key

#### Scenario: Unknown key name is rejected

- **GIVEN** `tui.jsonc` contains `"keybindings": { "start": ["HyperRun"] }`
- **WHEN** TUI config is loaded
- **THEN** loading fails
- **AND** the error identifies `HyperRun` as an unsupported key name

#### Scenario: Duplicate start key is rejected

- **GIVEN** `tui.jsonc` contains `"keybindings": { "start": ["F5", "F5"] }`
- **WHEN** TUI config is loaded
- **THEN** loading fails
- **AND** the error identifies the duplicate start keybinding
