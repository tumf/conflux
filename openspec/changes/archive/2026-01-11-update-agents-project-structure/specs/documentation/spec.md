## ADDED Requirements

### Requirement: AGENTS.md Project Structure Accuracy

The AGENTS.md SHALL accurately document the current project structure and module organization.

#### Scenario: Module listing accuracy

- **WHEN** a developer reads the Project Structure section in AGENTS.md
- **THEN** all source files in src/ directory are listed
- **AND** each file has a brief description of its purpose
- **AND** no non-existent files are listed

#### Scenario: TUI subdirectory documentation

- **WHEN** AGENTS.md documents the project structure
- **THEN** the tui/ subdirectory and its contents are documented
- **AND** the relationship between tui module files is clear

#### Scenario: Dependencies table accuracy

- **WHEN** AGENTS.md lists key dependencies
- **THEN** all major crates from Cargo.toml are listed
- **AND** each dependency has its purpose described
