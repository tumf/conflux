# cli Specification Delta

## ADDED Requirements

### Requirement: init Subcommand

`init` subcommand SHALL generate a `.openspec-orchestrator.jsonc` configuration template file in the current directory.

#### Scenario: Generate default template (claude)

- **WHEN** user runs `openspec-orchestrator init`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: Generate opencode template

- **WHEN** user runs `openspec-orchestrator init --template opencode`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: Generate claude template explicitly

- **WHEN** user runs `openspec-orchestrator init --template claude`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: Generate codex template

- **WHEN** user runs `openspec-orchestrator init --template codex`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: Config file already exists without force flag

- **WHEN** user runs `openspec-orchestrator init`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: Overwrite existing config with force flag

- **WHEN** user runs `openspec-orchestrator init --force`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: Invalid template name

- **WHEN** user runs `openspec-orchestrator init --template invalid`
- **THEN** the command exits with an error
- **AND** an error message lists valid template options (opencode, claude, codex)
