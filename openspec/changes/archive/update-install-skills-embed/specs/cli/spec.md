## MODIFIED Requirements

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled Conflux agent skills into the standard `.agents/skills` locations without requiring a source argument.

Skills SHALL be embedded into the binary at compile time. The subcommand SHALL NOT depend on a `skills/` directory existing at runtime. When embedded skills are available, the subcommand SHALL use them as the installation source. When embedded skills are unavailable (development builds), the subcommand SHALL fall back to discovering skills from the repository's top-level `skills/` directory.

#### Scenario: Install bundled skills in project scope by default

- **WHEN** the user runs `cflx install-skills`
- **THEN** the CLI installs embedded bundled skills (cflx-proposal, cflx-workflow, cflx-run)
- **AND** installed skills are written under `./.agents/skills` with their full file trees (SKILL.md, scripts, references)
- **AND** the lock file is written to `./.agents/.skill-lock.json`
- **AND** lock entries have `source_type` of `"self"`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills --global`
- **THEN** the CLI installs embedded bundled skills (cflx-proposal, cflx-workflow, cflx-run)
- **AND** installed skills are written under `~/.agents/skills` with their full file trees
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Install succeeds without skills directory at runtime

- **WHEN** the user runs `cflx install-skills` in a directory that does not contain a `skills/` subdirectory
- **AND** the binary was built with embedded skills
- **THEN** the install succeeds and all bundled skills are installed

#### Scenario: Development fallback to local skills directory

- **WHEN** the binary has no embedded skills (e.g. library-only build or test harness)
- **AND** the user runs `cflx install-skills` in a directory containing a `skills/` subdirectory
- **THEN** the CLI discovers and installs skills from the `skills/` directory

#### Scenario: Reject legacy explicit self source syntax

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`

#### Scenario: Reject unsupported explicit local source syntax

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the command exits with an error
- **AND** the error message instructs the user to run `cflx install-skills` or `cflx install-skills --global`
