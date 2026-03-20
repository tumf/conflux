## MODIFIED Requirements

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into the standard `.agents/skills` locations without requiring a source argument.

#### Scenario: Install bundled skills in project scope by default

- **WHEN** the user runs `cflx install-skills`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.agents/skills`
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Reject legacy explicit self source syntax

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

#### Scenario: Reject unsupported explicit local source syntax

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`
