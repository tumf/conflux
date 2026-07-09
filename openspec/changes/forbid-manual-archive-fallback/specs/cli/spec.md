## MODIFIED Requirements

### Requirement: Reliable Archive Tracking

archive 検証は `openspec/changes/{change_id}` が存在する場合に未アーカイブとして扱わなければならない（SHALL）。

archive 検証と archived change 解決は、archive entry として direct match (`openspec/changes/archive/<change_id>`) と date-prefixed match (`openspec/changes/archive/<date>-<change_id>`) の両方を同一 change として扱わなければならない（MUST）。

archive 検証と archived change 解決は、nested date directory layout (`openspec/changes/archive/<date>/<change_id>`) を valid archive entry として扱ってはならない（MUST NOT）。

archive 検証は active change directory が存在しない場合でも、valid archive entry が存在しない、または matching invalid archive layout が存在するなら archive complete として扱ってはならない（MUST NOT）。

invalid archive layout の診断は、offending path と expected `openspec/changes/archive/YYYY-MM-DD-<change_id>` layout を含まなければならない（MUST）。

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

#### Scenario: nested archive layout is rejected
- **GIVEN** `openspec/changes/{change_id}` does not exist
- **AND** `openspec/changes/archive/2026-07-09/{change_id}/tasks.md` exists
- **WHEN** archive completion verification or archived change resolution runs for `{change_id}`
- **THEN** the change is not treated as archived
- **AND** the result reports invalid archive layout
- **AND** the diagnostic includes `openspec/changes/archive/2026-07-09/{change_id}`
- **AND** the diagnostic identifies `openspec/changes/archive/YYYY-MM-DD-{change_id}` as the expected layout

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only non-archived change entries from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

For each listed active change that declares proposal dependencies, the human-readable change list MUST render a `Dependencies:` line. Each dependency entry MUST include the dependency id and a status label in the form `<dependency-id> [<status>]`.

For active changes that declare proposal dependencies, `cflx openspec show <change-id>` MUST render dependency status details in human-readable output using the same `<dependency-id> [<status>]` format as list output.

For active changes that declare proposal dependencies, `cflx openspec show --json <change-id>` MUST expose dependency status details as structured JSON data containing each dependency id and status label.

The status label MUST be derived from workspace-local repository evidence as follows:

- `done` when the dependency target is archived under `openspec/changes/archive/`, including dated archive directory names whose date prefix maps to the dependency id
- `running` when the dependency target is listed in `.conflux-inflight`
- `rejected` when the dependency target has `openspec/changes/<id>/proposal.md` and `openspec/changes/<id>/REJECTED.md`
- `pending` when the dependency target exists as an active change under `openspec/changes/` and is not classified as `running`, `done`, or `rejected`
- `missing` when the dependency target is not found as active, in-flight, rejected, or archived

The list command and human-readable show command MUST omit the `Dependencies:` line for active changes that declare no dependencies.

The `cflx openspec show --deltas-only <change-id>` output MUST remain focused on spec deltas and MUST NOT add dependency status details.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`, but nested archive paths such as `openspec/changes/archive/YYYY-MM-DD/<change-id>` MUST NOT be resolved as archived changes.

The native `cflx openspec archive <change-id>` subcommand MUST archive successful changes into a date-prefixed destination under `openspec/changes/archive/` using the format `YYYY-MM-DD-<change-id>`.

#### Scenario: show displays pending active dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/feature-a/proposal.md` exists
- **AND** `feature-a` is not listed in `.conflux-inflight`
- **AND** no archive entry or `REJECTED.md` marker for `feature-a` exists
- **WHEN** the user runs `cflx openspec show feature-b`
- **THEN** the output includes `Dependencies: feature-a [pending]`

#### Scenario: show displays rejected dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/feature-a/proposal.md` exists
- **AND** `openspec/changes/feature-a/REJECTED.md` exists
- **WHEN** the user runs `cflx openspec show feature-b`
- **THEN** the output includes `feature-a [rejected]` in its `Dependencies:` line

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

#### Scenario: show rejects nested archive layout

- **GIVEN** `openspec/changes/archive/2026-07-09/feature-a/proposal.md` exists
- **AND** no valid archive entry exists at `openspec/changes/archive/feature-a` or `openspec/changes/archive/2026-07-09-feature-a`
- **WHEN** the user runs `cflx openspec show feature-a`
- **THEN** the command does not resolve `feature-a` as an archived change
- **AND** the command reports invalid archive layout instead of treating the nested directory as valid
