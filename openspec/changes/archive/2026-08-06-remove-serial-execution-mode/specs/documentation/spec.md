## MODIFIED Requirements

### Requirement: Japanese Localization

The project SHALL provide README.ja.md as a complete Japanese translation whose command examples and supported execution controls match README.md.

#### Scenario: Execution documentation parity

- **WHEN** README.ja.md documents worktree execution
- **THEN** it documents the supported Git worktree backend and current VCS choices
- **AND** CLI options `--max-concurrent`, `--vcs`, and `--dry-run` are documented where applicable
- **AND** the retired `--parallel` flag is not documented

#### Scenario: Technical consistency

- **WHEN** README.ja.md is compared with README.md
- **THEN** code and command-line examples are identical
- **AND** only prose text is translated to Japanese
