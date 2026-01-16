# documentation Specification

## Purpose

Define documentation requirements for the OpenSpec Orchestrator project.

## ADDED Requirements

### Requirement: README.md Content Accuracy

The README.md SHALL accurately document all current features, commands, and project structure.

#### Scenario: Default behavior documentation

- **WHEN** a user reads the README.md
- **THEN** they understand that running `cflx` without subcommands launches the TUI
- **AND** the TUI is described as the default interactive mode

#### Scenario: Init command documentation

- **WHEN** a user reads the README.md
- **THEN** they find documentation for the `init` subcommand
- **AND** all available templates (claude, opencode, codex) are described
- **AND** the `--force` and `--template` flags are documented

#### Scenario: Project structure accuracy

- **WHEN** a user reads the README.md
- **THEN** the project structure lists all current source files
- **AND** includes `hooks.rs`, `task_parser.rs`, and `templates.rs`

### Requirement: Japanese Localization

The project SHALL provide README.ja.md as a complete Japanese translation.

#### Scenario: README.ja.md availability

- **GIVEN** a Japanese-speaking user visits the repository
- **WHEN** they look for documentation
- **THEN** README.ja.md provides complete Japanese documentation
- **AND** the content matches README.md in structure and completeness

#### Scenario: Technical consistency

- **WHEN** README.ja.md is compared with README.md
- **THEN** code examples are identical
- **AND** command-line examples are identical
- **AND** only prose text is translated to Japanese
