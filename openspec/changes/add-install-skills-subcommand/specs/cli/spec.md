## ADDED Requirements

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled or local agent skills into the standard `.agents/skills` locations.

#### Scenario: Install bundled skills in project scope

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install local skills in project scope

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the CLI starts an install flow using the local source path `../my-skills`
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills self --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.agents/skills`
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Reject unsupported source schemes

- **WHEN** the user runs `cflx install-skills git:https://example.com/repo`
- **THEN** the command exits with an error
- **AND** the error message states that only `self` and `local:<path>` are supported
