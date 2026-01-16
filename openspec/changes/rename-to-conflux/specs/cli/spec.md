## MODIFIED Requirements

### Requirement: Default TUI Mode Launch

ユーザーが引数なしでコマンドを実行した場合、TUIモードを起動しなければならない (MUST)。

#### Scenario: 引数なしでTUIモード起動

- **WHEN** user runs `cflx` without arguments
- **THEN** the TUI dashboard is launched
- **AND** displays the list of changes with progress

### Requirement: Change Selection via CLI Argument

`run` サブコマンドは `--change` フラグで特定の変更のみを処理できなければならない (MUST)。

#### Scenario: 単一の変更を指定

- **WHEN** user runs `cflx run --change <id>`
- **AND** the change exists
- **THEN** only that change is processed

#### Scenario: カンマ区切りで複数の変更を指定

- **WHEN** user runs `cflx run --change a,b,c`
- **THEN** changes `a`, `b`, and `c` are processed in order

#### Scenario: 存在しない変更IDを指定

- **WHEN** user runs `cflx run --change nonexistent`
- **THEN** a warning is logged
- **AND** the orchestrator continues with no changes

#### Scenario: 存在する変更と存在しない変更の混在

- **WHEN** user runs `cflx run --change a,nonexistent,c`
- **THEN** changes `a` and `c` are processed
- **AND** a warning is logged for `nonexistent`

### Requirement: Default Run Mode When No Arguments

`run` サブコマンドを引数なしで実行した場合、すべての承認済み変更を処理しなければならない (MUST)。

#### Scenario: 引数なしでrunサブコマンド実行

- **WHEN** user runs `cflx run`
- **THEN** all approved changes are processed
- **AND** unapproved changes are skipped with a warning

### Requirement: init Subcommand

`init` サブコマンドは `.cflx.jsonc` 設定テンプレートファイルをカレントディレクトリに生成しなければならない (MUST)。

#### Scenario: デフォルトテンプレート (claude) の生成

- **WHEN** user runs `cflx init`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: opencode テンプレートの生成

- **WHEN** user runs `cflx init --template opencode`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: claude テンプレートを明示的に指定

- **WHEN** user runs `cflx init --template claude`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: codex テンプレートの生成

- **WHEN** user runs `cflx init --template codex`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: 設定ファイルが既に存在する場合（force フラグなし）

- **WHEN** user runs `cflx init`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: force フラグで既存設定を上書き

- **WHEN** user runs `cflx init --force`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: 無効なテンプレート名

- **WHEN** user runs `cflx init --template invalid`
- **THEN** the command exits with an error
- **AND** an error message lists valid template options (opencode, claude, codex)

### Requirement: Version Display

`--version` または `-V` フラグでバージョン情報を表示しなければならない (MUST)。

#### Scenario: --version フラグ

- **WHEN** user runs `cflx --version`
- **THEN** the version number is displayed
- **AND** the command exits with code 0

#### Scenario: -V フラグ

- **WHEN** user runs `cflx -V`
- **THEN** the version number is displayed
- **AND** the command exits with code 0

### Requirement: Approval Subcommand

`approve` サブコマンドで変更の承認状態を管理できなければならない (MUST)。

#### Scenario: 変更を承認

- **WHEN** user runs `cflx approve set {change_id}`
- **AND** the change exists
- **THEN** an `approved` file is created with checksums
- **AND** a success message is displayed
