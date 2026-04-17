## ADDED Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

#### Scenario: List canonical specs through the native CLI

- **WHEN** the user runs `cflx openspec list --specs`
- **THEN** the CLI lists canonical specifications from `openspec/specs/`
- **AND** the command does not rely on `scripts/cflx.py`

#### Scenario: Show change deltas as machine-readable JSON

- **GIVEN** a change id exists under `openspec/changes/` or `openspec/changes/archive/`
- **WHEN** the user runs `cflx openspec show <change-id> --json --deltas-only`
- **THEN** the CLI emits machine-readable JSON for that change's delta-oriented view
- **AND** the output is sufficient for skill guidance that currently uses helper-script JSON output

#### Scenario: Strict validation with evidence mode is available natively

- **GIVEN** a change id exists under `openspec/changes/`
- **WHEN** the user runs `cflx openspec validate <change-id> --strict --evidence error`
- **THEN** the CLI applies the same strict proposal validation contract used by bundled Conflux skills
- **AND** the command exits non-zero when validation fails

#### Scenario: Archive executes without a Python helper

- **GIVEN** a change is ready to archive
- **WHEN** the user runs `cflx openspec archive <change-id> --yes`
- **THEN** the CLI archives the change and promotes spec deltas without invoking a bundled Python helper
