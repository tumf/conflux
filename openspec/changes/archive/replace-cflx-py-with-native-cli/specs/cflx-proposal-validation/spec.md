## ADDED Requirements

### Requirement: Bundled skills use native OpenSpec CLI commands

Conflux bundled skill sources and skill-facing documentation MUST instruct agents and users to call native `cflx openspec` subcommands for list/show/validate/archive operations. Distributed bundled skills MUST NOT require a bundled `scripts/cflx.py` auxiliary file solely to perform those operations.

#### Scenario: Proposal skill references native validation command

- **GIVEN** the repository contains the bundled `cflx-proposal` skill source
- **WHEN** the skill documents how to validate a proposal strictly
- **THEN** it references `cflx openspec validate <id> --strict`
- **AND** it does not require `python3 "<SKILL_ROOT>/scripts/cflx.py"`

#### Scenario: Workflow skill references native list/show/archive commands

- **GIVEN** the repository contains the bundled `cflx-workflow` skill source or references
- **WHEN** the skill documents change lookup or archive actions
- **THEN** it references `cflx openspec list`, `cflx openspec show`, and `cflx openspec archive`
- **AND** it does not instruct the agent to call a bundled helper script for those operations

#### Scenario: Installed bundled skills no longer ship cflx.py helpers

- **WHEN** the user runs `cflx install-skills`
- **THEN** the installed `cflx-proposal` and `cflx-workflow` skill directories do not contain `scripts/cflx.py`
- **AND** the installed instructions still provide native command guidance for the equivalent operations
