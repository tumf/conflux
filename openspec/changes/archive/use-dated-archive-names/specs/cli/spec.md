## MODIFIED Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only active changes from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`.

The native `cflx openspec archive <change-id>` subcommand MUST archive successful changes into a date-prefixed destination under `openspec/changes/archive/` using the format `YYYY-MM-DD-<change-id>`.

#### Scenario: archive subcommand stores change in dated archive directory

- **GIVEN** active change `add-env-openspec-cmd` exists under `openspec/changes/add-env-openspec-cmd`
- **WHEN** the user runs `cflx openspec archive add-env-openspec-cmd`
- **THEN** the active change directory is removed from `openspec/changes/add-env-openspec-cmd`
- **AND** the archived change exists at `openspec/changes/archive/2026-01-08-add-env-openspec-cmd`
- **AND** the success output reports `openspec/changes/archive/2026-01-08-add-env-openspec-cmd`

#### Scenario: archive subcommand fails when dated destination already exists

- **GIVEN** active change `add-env-openspec-cmd` exists under `openspec/changes/add-env-openspec-cmd`
- **AND** `openspec/changes/archive/2026-01-08-add-env-openspec-cmd` already exists
- **WHEN** the user runs `cflx openspec archive add-env-openspec-cmd`
- **THEN** the command fails with an archive destination already exists error
- **AND** the active change directory is not silently moved to another generated name

### Requirement: Reliable Archive Tracking

archive 検証は `openspec/changes/{change_id}` が存在する場合に未アーカイブとして扱わなければならない（SHALL）。

archive 検証と archived change 解決は、archive entry として direct match (`openspec/changes/archive/<change_id>`) と date-prefixed match (`openspec/changes/archive/<date>-<change_id>`) の両方を同一 change として扱わなければならない（MUST）。

#### Scenario: changes が残っている場合は未アーカイブ扱い
- **WHEN** archive コマンドが成功する
- **AND** `openspec/changes/{change_id}` が存在している
- **THEN** archive 検証は未アーカイブとして扱われる
- **AND** archive コマンドは再実行される

#### Scenario: dated archive entry is treated as archived completion
- **GIVEN** `openspec/changes/{change_id}` は存在しない
- **AND** `openspec/changes/archive/2026-01-08-{change_id}` が存在する
- **WHEN** archive completion verification or archived change resolution runs for `{change_id}`
- **THEN** the change is treated as archived
- **AND** the implementation does not require a direct-match archive directory to exist
