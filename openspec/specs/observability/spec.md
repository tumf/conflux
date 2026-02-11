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

hookコマンドは、実行前にコマンド文字列をTUI Logs Viewへ表示しなければならない（MUST）。
hookコマンドのstdout/stderrは取得可能な範囲でTUI Logs Viewへ表示しなければならない（MUST）。

**追加**: TUI Logs Viewに表示されるすべてのログエントリーは、`--logs`オプションが指定されている場合にデバッグログファイルにも出力されなければならない（MUST）。

#### Scenario: hook実行のコマンドと出力がLogs Viewに表示される
- **GIVEN** `hooks.pre_apply` が `echo 'hello'` に設定されている
- **WHEN** pre_apply hook が実行される
- **THEN** Logs View に `Running pre_apply hook: echo 'hello'` が表示される
- **AND** Logs View に `hello` が表示される

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
- 実行コマンド（program + args、利用可能な場合）
- stderr/stdout（取得できた場合）

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

#### Scenario: VCS command failure includes stderr and command

- **GIVEN** a VCS command fails with stderr output
- **WHEN** the orchestrator records the error
- **THEN** the error message includes the full command (program + args)
- **AND** the error message includes the working directory when available
- **AND** the error message includes the captured stderr (and stdout if available)

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
