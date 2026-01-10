# configuration Specification Delta

## ADDED Requirements

### Requirement: Approved File Format

The approval system SHALL use a file-based approval mechanism with MD5 checksums.

#### Scenario: Approved file structure

- **WHEN** a change is approved
- **THEN** an `approved` file is created at `openspec/changes/{change_id}/approved`
- **AND** the file contains one line per tracked file
- **AND** each line format is `{md5sum}  {relative_path}` (two spaces between)
- **AND** paths are relative to project root

#### Scenario: Files included in approval

- **WHEN** generating the approved file
- **THEN** all `.md` files in the change directory are included
- **AND** files in subdirectories (e.g., `specs/cli/spec.md`) are included
- **AND** `tasks.md` is included in the manifest but excluded from validation
- **AND** files are sorted alphabetically by path

#### Scenario: Approved file example

```
47dadc8fb73c2d2ec6b19c0de0d71094  openspec/changes/my-change/design.md
88585d9f377f89cededbb4eeabcf9cf2  openspec/changes/my-change/proposal.md
c1fce89931c1142dd06f67a03059619d  openspec/changes/my-change/specs/cli/spec.md
ba74d36d6cdc1effcae37cfed4f97e19  openspec/changes/my-change/tasks.md
```

### Requirement: Approval Validation Logic

The system SHALL validate approval by comparing current files against the approved manifest.

#### Scenario: Validation excludes tasks.md

- **WHEN** validating approval status
- **THEN** `tasks.md` hash changes do NOT affect approval status
- **AND** `tasks.md` missing from current directory does NOT affect approval status
- **AND** only non-tasks.md files are compared for validation

#### Scenario: File list mismatch invalidates approval

- **WHEN** validating approval status
- **AND** a new `.md` file (not `tasks.md`) is added to the change directory
- **THEN** the change is considered unapproved
- **AND** re-approval is required

#### Scenario: File content change invalidates approval

- **WHEN** validating approval status
- **AND** any `.md` file (except `tasks.md`) has different content than at approval time
- **THEN** the change is considered unapproved
- **AND** re-approval is required

#### Scenario: File removed invalidates approval

- **WHEN** validating approval status
- **AND** a `.md` file (not `tasks.md`) listed in the manifest no longer exists
- **THEN** the change is considered unapproved
- **AND** re-approval is required

#### Scenario: Missing approved file means unapproved

- **WHEN** checking approval status
- **AND** the `approved` file does not exist
- **THEN** the change is considered unapproved
- **AND** `is_approved` field is `false`
