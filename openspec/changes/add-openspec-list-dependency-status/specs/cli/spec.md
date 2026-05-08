## MODIFIED Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only active changes from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

For each active listed change that declares proposal dependencies, the human-readable change list MUST render a `Dependencies:` line. Each dependency entry MUST include the dependency id and a status label in the form `<dependency-id> [<status>]`.

The status label MUST be derived from workspace-local repository evidence as follows:

- `done` when the dependency target is archived under `openspec/changes/archive/`, including dated archive directory names whose date prefix maps to the dependency id
- `running` when the dependency target is listed in `.conflux-inflight`
- `pending` when the dependency target exists as an active change under `openspec/changes/` and is not classified as `running` or `done`
- `missing` when the dependency target is not found as active, in-flight, or archived

The list command MUST omit the `Dependencies:` line for active changes that declare no dependencies.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`.

The native `cflx openspec archive <change-id>` subcommand MUST archive successful changes into a date-prefixed destination under `openspec/changes/archive/` using the format `YYYY-MM-DD-<change-id>`.

#### Scenario: list shows pending active dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/feature-a/proposal.md` exists
- **AND** `feature-a` is not listed in `.conflux-inflight`
- **AND** no archive entry for `feature-a` exists
- **WHEN** the user runs `cflx openspec list`
- **THEN** the `feature-b` block includes `Dependencies: feature-a [pending]`

#### Scenario: list shows running in-flight dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `.conflux-inflight` contains `feature-a`
- **WHEN** the user runs `cflx openspec list`
- **THEN** the `feature-b` block includes `feature-a [running]` in its `Dependencies:` line

#### Scenario: list shows done archived dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `openspec/changes/archive/2026-05-08-feature-a/proposal.md` exists
- **WHEN** the user runs `cflx openspec list`
- **THEN** the `feature-b` block includes `feature-a [done]` in its `Dependencies:` line

#### Scenario: list shows missing dependency

- **GIVEN** active change `feature-b` declares dependency `feature-a`
- **AND** `feature-a` does not exist under active changes, `.conflux-inflight`, or archive entries
- **WHEN** the user runs `cflx openspec list`
- **THEN** the `feature-b` block includes `feature-a [missing]` in its `Dependencies:` line

#### Scenario: list omits dependency line for independent change

- **GIVEN** active change `feature-independent` declares no dependencies
- **WHEN** the user runs `cflx openspec list`
- **THEN** the `feature-independent` block does not include a `Dependencies:` line

#### Scenario: specs listing remains unchanged

- **GIVEN** canonical specs exist under `openspec/specs/`
- **WHEN** the user runs `cflx openspec list --specs`
- **THEN** the output lists specification names, paths, and requirement counts
- **AND** the specs output does not include dependency status lines
