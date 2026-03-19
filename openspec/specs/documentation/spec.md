# documentation Specification

## Purpose
Defines requirements for README files and project documentation accuracy.
## Requirements
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

#### Scenario: Parallel execution documentation parity

- **WHEN** README.ja.md documents parallel execution
- **THEN** it includes both jj workspaces and Git worktrees support
- **AND** VCS backend selection options (auto, jj, git) are documented
- **AND** CLI flags `--parallel`, `--max-concurrent`, `--vcs`, `--dry-run` are documented

#### Scenario: Hooks documentation parity

- **WHEN** README.ja.md documents hooks
- **THEN** it includes all current hook types (on_start, on_finish, on_error, on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end, on_queue_add, on_queue_remove, on_approve, on_unapprove)
- **AND** deprecated hooks are not documented
- **AND** placeholder variables match README.md

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

### Requirement: OpenAPI YAML generation
ドキュメントは Web 監視 API の OpenAPI 3.1 形式の YAML をコードから自動生成し、`docs/openapi.yaml` として提供しなければならない（SHALL）。OpenAPI YAML は手動編集してはならない（MUST NOT）。

#### Scenario: 生成コマンドで更新する
- **WHEN** 開発者が `make openapi` を実行する
- **THEN** `docs/openapi.yaml` が最新の仕様で生成される
- **AND** `GET /api/health`, `GET /api/state`, `GET /api/changes`, `GET /api/changes/{id}` の仕様が含まれる
- **AND** 変更の承認 API と WebSocket `/ws` が記載される

#### Scenario: 生成差分を検知する
- **WHEN** API 実装が変更され、生成結果がリポジトリと一致しない
- **THEN** `make check-openapi` は失敗する
- **AND** CI は差分を検知して失敗する

### Requirement: Git hooks ツールの案内
README.md、README.ja.md、DEVELOPMENT.md は Git hooks 管理に prek を使用することを明示し、インストール/フック導入/実行方法を記載しなければならない（SHALL）。
pre-commit のインストール手順は記載してはならない（MUST NOT）。
`.pre-commit-config.yaml` が prek の互換設定として利用されることを明示しなければならない（MUST）。

#### Scenario: prek 導入手順の提示
- **WHEN** 開発者が Git hooks のセットアップ手順を確認する
- **THEN** `prek install` が記載されている
- **AND** `pre-commit uninstall` を含む移行手順が記載されている

#### Scenario: README の Git hooks セクション整合
- **WHEN** README.md と README.ja.md を参照する
- **THEN** Git hooks セクションが両方に存在する
- **AND** コマンド例が一致している
- **AND** `.pre-commit-config.yaml` が互換設定として記載されている

#### Scenario: DEVELOPMENT.md のフック手順
- **WHEN** DEVELOPMENT.md を読む
- **THEN** prek のインストールと実行方法が記載されている
- **AND** `pre-commit install` の記述がない


#


#
