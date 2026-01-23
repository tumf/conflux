# Observability Specification

## Purpose

This specification defines the logging and observability requirements for the Conflux orchestrator. It ensures that all command executions, TUI events, and system operations are properly logged for debugging and troubleshooting purposes.

The specification covers:
- Command execution logging (VCS, AI agents, hooks)
- TUI log synchronization to debug files
- Log level classification and formatting standards
## Requirements
### Requirement: REQ-OBS-001 Command Execution Logging

オーケストレーターは外部コマンドを実行する前にコマンド情報をログ出力しなければならない（MUST）。

ログには以下を含めなければならない（MUST）。
- 実行ファイル名
- 引数一覧
- 作業ディレクトリ（設定されている場合）

apply/archive/resolveのAIエージェントコマンドは、`{change_id}`、`{prompt}`、`{conflict_files}`などのプレースホルダーを展開した完全なコマンド文字列を、実行前にTUI Logs Viewへ表示しなければならない（MUST）。このログはユーザー向けの`info`相当ログとして扱う（SHALL）。

**追加**: TUI Logs Viewに表示されるすべてのログエントリーは、`--logs`オプションが指定されている場合にデバッグログファイルにも出力されなければならない（MUST）。

#### Scenario: VCSコマンドの実行ログ
- **GIVEN** git worktreeを作成する
- **WHEN** `git worktree add`コマンドを実行する
- **THEN** コマンド全体が`debug!`レベルでログに記録される
- **AND** 作業ディレクトリが含まれる

#### Scenario: AIエージェントコマンドの実行ログ
- **GIVEN** change `alpha` をapplyする
- **WHEN** OpenCodeのapplyコマンドを実行する
- **THEN** コマンドラインが`info!`レベルでログに記録される
- **AND** TUI Logs Viewにはプレースホルダー展開後のコマンドが表示される

#### Scenario: Resolveコマンドのプレースホルダー展開ログ
- **GIVEN** resolve_commandに`{conflict_files}`を含む
- **WHEN** resolveを開始する
- **THEN** TUI Logs Viewには`{conflict_files}`を実際のファイル一覧に展開したコマンドが表示される
- **AND** 表示は実行前に行われる

#### Scenario: フック実行ログ
- **GIVEN** on_apply_startフックが設定されている
- **WHEN** フックコマンドを実行する
- **THEN** コマンドラインが`info!`レベルでログに記録される
- **AND** ログには"Running on_apply_start hook"のコンテキストが含まれる

#### Scenario: TUIログのデバッグファイル同期
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** エージェント処理中にエラーが発生しTUI Logs Viewに表示される
- **THEN** 同じエラーメッセージがデバッグログファイルに`ERROR`レベルで記録される
- **AND** ログには`tui_log`ターゲットが含まれる

#### Scenario: Warningログの同期
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** マージが延期され警告がTUI Logs Viewに表示される
- **THEN** 同じ警告メッセージがデバッグログファイルに`WARN`レベルで記録される

#### Scenario: Infoログの同期
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** 処理開始時のinfoログがTUI Logs Viewに表示される
- **THEN** 同じメッセージがデバッグログファイルに`INFO`レベルで記録される

### Requirement: REQ-OBS-002 Appropriate Log Level Classification

The orchestrator MUST use appropriate log levels based on command importance.

Log level criteria:
- `info!`: Major user-facing operations (apply, archive, analyze, hook execution)
- `debug!`: Internal VCS commands, auxiliary command execution

#### Scenario: Output Control with Default Log Level

- **GIVEN** RUST_LOG environment variable is not set (default)
- **WHEN** running the orchestrator
- **THEN** `info!` level command logs are displayed
- **AND** `debug!` level VCS command logs are not displayed

#### Scenario: Detailed Log Output in Debug Mode

- **GIVEN** RUST_LOG=debug is set
- **WHEN** running the orchestrator
- **THEN** all VCS command logs are displayed
- **AND** internal auxiliary command logs are also displayed

### Requirement: REQ-OBS-003 Unified Log Format

The orchestrator MUST ensure error messages include actionable context such as operation type, change ID, and workspace or working directory when available.

#### Scenario: Error message includes execution context
- **GIVEN** an apply operation fails for change `alpha`
- **WHEN** the orchestrator records the error
- **THEN** the error message includes the operation type (`apply`) and change ID (`alpha`)
- **AND** the message includes the workspace or working directory when available

### Requirement: REQ-OBS-004 Error Messages with Context

The orchestrator MUST ensure error messages include actionable context information to aid troubleshooting and debugging.

Context information MUST include:
- Operation type (e.g., apply, archive, resolve, analyze)
- Change ID (when the error is related to a specific change)
- Workspace path or working directory (when available and relevant)
- Failure reason or error details (when available)

#### Scenario: Apply Operation Failure with Context

- **GIVEN** an apply operation fails for change `alpha`
- **WHEN** the orchestrator records the error
- **THEN** the error message includes the operation type (`apply`)
- **AND** the error message includes the change ID (`alpha`)
- **AND** the error message includes the workspace or working directory when available

#### Scenario: Cancelled Operation with Context

- **GIVEN** an archive operation is cancelled for change `beta`
- **WHEN** the cancellation is logged
- **THEN** the error message includes "Cancelled archive for 'beta'"
- **AND** the message includes the workspace path if applicable

#### Scenario: Internal Error with Command Context

- **GIVEN** stdout/stderr capture fails during command execution
- **WHEN** the internal error is recorded
- **THEN** the error message includes the command that was being executed
- **AND** the error message includes the working directory where the command was running

#### Scenario: TUI and Log Message Consistency

- **GIVEN** a parallel execution error is encountered
- **WHEN** the error is displayed in both TUI and log files
- **THEN** the TUI event message and the log message contain identical context information
- **AND** both include the operation type, change ID, and workspace path

### Requirement: REQ-OBS-005 TUI Input Rejection Logging

The orchestrator MUST log warning messages when user input is ignored in the TUI to help users understand why their actions had no effect.

#### Scenario: Enter Key Ignored in Worktrees View

- **GIVEN** the TUI is displaying the Worktrees view
- **WHEN** the Enter key is pressed but ignored due to missing conditions
- **THEN** a warning log is displayed with a message explaining the rejection reason
- **AND** the message enables the user to determine the required conditions
