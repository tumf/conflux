# CLI Specification Delta

## MODIFIED Requirements

### Requirement: Native OpenSpec utility subcommands

The CLI SHALL provide a native `cflx openspec` command namespace for repository-scoped OpenSpec utility operations that were previously exposed through skill-local helper scripts. These subcommands MUST support `list`, `show`, `validate`, and `archive` without requiring a bundled Python helper.

When the user runs `cflx openspec list` without `--specs`, the command MUST enumerate only active changes from `openspec/changes/` and MUST NOT include archived change entries from `openspec/changes/archive/` in the human-readable change list.

Archived changes MAY still be resolved by detail-oriented subcommands such as `cflx openspec show <change-id>`.

#### Scenario: List canonical specs through the native CLI

- **WHEN** the user runs `cflx openspec list --specs`
- **THEN** the CLI lists canonical specifications from `openspec/specs/`
- **AND** each listed spec includes its canonical requirement count derived from `### Requirement:` headings in `spec.md`
- **AND** the command does not rely on `scripts/cflx.py`

#### Scenario: Canonical spec with no requirements shows zero count

- **GIVEN** a canonical spec exists under `openspec/specs/empty-spec/spec.md`
- **AND** that file contains no `### Requirement:` headings
- **WHEN** the user runs `cflx openspec list --specs`
- **THEN** the CLI still lists `empty-spec`
- **AND** it renders `Requirements: 0`
