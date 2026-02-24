## MODIFIED Requirements
### Requirement: README.md Content Accuracy

README.md SHALL 正確に現在の機能・コマンド・プロジェクト構成を説明し、Golden Path の最短導線を現行 CLI の動作に合わせて提示しなければならない（SHALL）。存在しないコマンドやフラグを記載してはならない（MUST NOT）。

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

#### Scenario: Golden Path quick start alignment

- **WHEN** a user follows the Quick Start section
- **THEN** the steps match the actual CLI entry points (`cflx`, `cflx init`, `cflx run`)
- **AND** no non-existent commands or flags are referenced

## ADDED Requirements
### Requirement: Usage Guide Accuracy and Golden Path

docs/guides/USAGE.md SHALL 現行 CLI のコマンドとフラグに一致した使用例のみを記載し、Golden Path の導線は README.md と矛盾してはならない（SHALL）。存在しないコマンドやフラグは記載してはならない（MUST NOT）。

#### Scenario: Usage examples match current CLI

- **WHEN** a user reads docs/guides/USAGE.md
- **THEN** all commands and flags appear in `cflx --help` output
- **AND** deprecated or removed commands are not documented

#### Scenario: Golden Path parity with README

- **WHEN** a user compares Quick Start flows
- **THEN** the Golden Path in docs/guides/USAGE.md matches README.md in sequence and intent
