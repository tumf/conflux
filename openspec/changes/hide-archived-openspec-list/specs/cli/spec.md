## MODIFIED Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only active changes from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`.

#### Scenario: List canonical specs through the native CLI

- **WHEN** the user runs `cflx openspec list --specs`
- **THEN** the CLI lists canonical specifications from `openspec/specs/`
- **AND** the command does not rely on `scripts/cflx.py`

#### Scenario: List command hides archived changes

- **GIVEN** `openspec/changes/active-change/proposal.md` exists
- **AND** `openspec/changes/archive/2026-04-27-archived-change/proposal.md` exists
- **WHEN** the user runs `cflx openspec list`
- **THEN** the CLI lists `active-change`
- **AND** the CLI does not list `archived-change`
- **AND** the CLI does not render an archived status row for that archive entry

#### Scenario: Show change deltas as machine-readable JSON

- **GIVEN** a change id exists under `openspec/changes/` or `openspec/changes/archive/`
- **WHEN** the user runs `cflx openspec show <change-id> --json --deltas-only`
- **THEN** the CLI emits machine-readable JSON for that change's delta-oriented view
- **AND** the output is sufficient for skill guidance that currently uses helper-script JSON output

#### Scenario: Show command still resolves archived change

- **GIVEN** `openspec/changes/archive/2026-04-27-archived-change/proposal.md` exists
- **WHEN** the user runs `cflx openspec show archived-change`
- **THEN** the CLI resolves the archived change entry
- **AND** the command does not require the archived change to appear in `cflx openspec list`

#### Scenario: Strict validation with evidence mode is available natively

- **GIVEN** a change id exists under `openspec/changes/`
- **WHEN** the user runs `cflx openspec validate <change-id> --strict --evidence error`
- **THEN** the CLI applies the same strict proposal validation contract used by bundled Conflux skills
- **AND** the command exits non-zero when validation fails

#### Scenario: Archive executes without a Python helper

- **GIVEN** a change is ready to archive
- **WHEN** the user runs `cflx openspec archive <change-id> --yes`
- **THEN** the CLI archives the change and promotes spec deltas without invoking a bundled Python helper
