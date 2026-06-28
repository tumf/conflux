# Observability Specification

## Purpose

This specification defines the logging and observability requirements for the Conflux orchestrator. It ensures that all command executions, TUI events, and system operations are properly logged for debugging and troubleshooting purposes.

The specification covers:
- Command execution logging (VCS, AI agents, hooks)
- TUI log synchronization to debug files
- Log level classification and formatting standards
## Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

Conflux observability MUST distinguish recoverable degraded execution paths from terminal workflow failures. The bundled log mining helper MUST remain observability-only and MUST NOT influence scheduler decisions, resume routing, acceptance, archive, merge, or next-action behavior.

#### Scenario: recoverable analysis fallback is not mined as terminal error

- **GIVEN** dependency analysis rejects an LLM-produced graph
- **AND** Conflux successfully falls back to metadata-dependency-only analysis
- **WHEN** runtime logs are emitted and later mined by `scripts/cflx-log-mine.py`
- **THEN** the fallback remains visible as degraded analysis evidence
- **AND** the recoverable fallback is not emitted as an ERROR-level terminal workflow failure
- **AND** missing or rejected dependency blockers remain visible as actionable diagnostics
- **AND** mined log output does not affect workflow-control decisions

### Requirement: REQ-OBS-002 Appropriate Log Level Classification

The orchestrator MUST use appropriate log levels based on command importance.

Log level criteria:
- `info!`: Major user-facing operations (apply, archive, analyze, hook execution)
- `debug!`: Internal VCS commands, auxiliary command execution
- Agent subprocess stderr: `info!` (agent CLIs such as opencode use stderr for normal operation output)
- Internal orchestrator warnings: `warn!`

The `OutputHandler` trait MUST distinguish between agent subprocess stderr (normal operation output) and internal orchestrator warnings. Agent subprocess stderr MUST be logged at `info` level via a dedicated `on_agent_stderr` method. The existing `on_stderr` method MUST remain at `warn` level for internal warnings.

Default terminal-facing `cflx run` logging MUST suppress `debug!` and `trace!` records while preserving `info!`, `warn!`, and `error!` records. Persistent file logging MAY retain more detailed diagnostic records and MUST NOT be reduced by the default stdout filter.

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

#### Scenario: Agent subprocess stderr is logged at info level

- **GIVEN** an AI agent command (e.g., opencode) writes progress output to stderr
- **WHEN** the orchestrator captures the stderr output
- **THEN** the output is logged at `info` level via `on_agent_stderr`
- **AND** the output is NOT logged at `warn` level

#### Scenario: Internal orchestrator warnings remain at warn level

- **GIVEN** the orchestrator generates an internal warning (e.g., hook failure, cancellation)
- **WHEN** the warning is recorded
- **THEN** the warning is logged at `warn` level via `on_warn` or `on_stderr`
- **AND** the warning is NOT logged at `info` level

#### Scenario: Run stdout suppresses internal diagnostics by default

- **GIVEN** `cflx run` initializes stdout logging for a normal non-interactive run
- **AND** internal code emits `debug!` VCS command logs or dependency `trace!` poller logs
- **WHEN** the run writes terminal-facing stdout logs
- **THEN** stdout does not include `DEBUG Executing git command`
- **AND** stdout does not include `TRACE registering event source with poller`
- **AND** stdout does not include `TRACE deregistering event source from poller`
- **AND** user-facing `INFO`, `WARN`, and `ERROR` logs remain eligible for stdout display

#### Scenario: Persistent logs retain diagnostic detail

- **GIVEN** `cflx run` initializes both stdout and persistent file logging
- **AND** internal code emits `debug!` diagnostic records
- **WHEN** the logging system records events
- **THEN** the stdout filter does not reduce the persistent file logging layer's configured diagnostic level
- **AND** operators can still inspect detailed diagnostics through persistent log files and `cflx logs`

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

### Requirement: 無出力タイムアウトの警告ログ

オーケストレーターは無出力タイムアウトを検知した場合、警告ログを出力しなければならない (MUST)。

警告ログには以下を含めなければならない (MUST)：
- どの操作で発生したか（apply/archive/resolve/analyze/acceptance）
- 対象の change_id（該当する場合）
- 無出力継続時間と設定タイムアウト値

#### Scenario: 無出力タイムアウトの警告ログ
- **GIVEN** apply 実行中に無出力タイムアウトが発生する
- **WHEN** タイムアウト検知が行われる
- **THEN** warning ログが出力される
- **AND** ログに操作種別と change_id が含まれる

#

#

### Requirement: Stream-JSON Textify Emits Tool Event Summaries

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

このとき、ツール関連の非テキストイベントについては、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。
代わりに、`tool_use` / `tool_result` については 1行の要約を表示しなければならない (MUST)。

要約は「できるだけ情報を出す」方針とし、イベントに含まれる `name` や `input` / `result` から主要フィールドを抽出して含めなければならない (MUST)。
ただし、ログの肥大化を避けるため、長文の値や巨大な結果は省略(truncate)されなければならない (MUST)。

#### Scenario: tool_use が 1行サマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` と `input` を含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の生 JSON 行は表示されない
- **AND** `[tool_use:<name>]` で始まる 1行サマリが表示される
- **AND** サマリには `input` から抽出された主要フィールドが含まれる

#### Scenario: assistant message 内の tool_use ブロックもサマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `assistant` イベントを出力し、`message.content[]` に `tool_use` ブロックを含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** tool_use ブロックは生 JSON として表示されない
- **AND** tool_use の 1行サマリが表示される

#### Scenario: tool_result は巨大な内容を抑制したサマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_result` イベントを出力し、結果本文が非常に長い
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 生 JSON 行は表示されない
- **AND** `[tool_result:<name>]` で始まる 1行サマリが表示される
- **AND** 結果本文は必要に応じて省略(truncate)される

#### Scenario: textify 無効時は JSON 行が素通しされる

- **GIVEN** `stream_json_textify=false` である
- **AND** 子プロセスの stdout が stream-json の JSON 行を出力する
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の JSON 行は変換されず、そのまま表示される

### Requirement: CLI Hook Logs Include Captured Streams

The orchestrator SHALL expose captured hook output to both user-visible CLI(run) logs and persistent observability sinks when that output is available.

#### Scenario: CLI hook stdout is observable without debug logging

- **GIVEN** `RUST_LOG` is unset and a configured hook writes to stdout during `cflx run`
- **WHEN** the hook output is captured
- **THEN** the output is visible in the normal CLI run log stream
- **AND** the same hook execution remains available to the configured persistent log sink

#### Scenario: CLI hook stderr includes stream context

- **GIVEN** a configured hook writes to stderr during `cflx run`
- **WHEN** the output is logged
- **THEN** the log message identifies that the content came from captured hook output
- **AND** the message preserves enough context to distinguish stderr-derived diagnostics from the hook command log itself

#### Scenario: Truncated hook output is not silent

- **GIVEN** captured hook output exceeds the configured display threshold
- **WHEN** the orchestrator emits the CLI-visible hook log
- **THEN** the emitted message states that truncation occurred
- **AND** the persistent log representation follows the same truncation signaling rule when truncation is applied there

### Requirement: REQ-OBS-002 Appropriate Log Level Classification

The orchestrator MUST use appropriate log levels based on command importance.

Log level criteria:
- `info!`: Major user-facing operations (apply, archive, analyze, hook execution)
- `debug!`: Internal VCS commands, auxiliary command execution
- Agent subprocess stderr: `info!` (agent CLIs such as opencode use stderr for normal operation output)
- Internal orchestrator warnings: `warn!`

The `OutputHandler` trait MUST distinguish between agent subprocess stderr (normal operation output) and internal orchestrator warnings. Agent subprocess stderr MUST be logged at `info` level via a dedicated `on_agent_stderr` method. The existing `on_stderr` method MUST remain at `warn` level for internal warnings.

Default terminal-facing `cflx run` logging MUST suppress `debug!` and `trace!` records while preserving `info!`, `warn!`, and `error!` records. Persistent file logging MAY retain more detailed diagnostic records and MUST NOT be reduced by the default stdout filter.

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

#### Scenario: Agent subprocess stderr is logged at info level

- **GIVEN** an AI agent command (e.g., opencode) writes progress output to stderr
- **WHEN** the orchestrator captures the stderr output
- **THEN** the output is logged at `info` level via `on_agent_stderr`
- **AND** the output is NOT logged at `warn` level

#### Scenario: Internal orchestrator warnings remain at warn level

- **GIVEN** the orchestrator generates an internal warning (e.g., hook failure, cancellation)
- **WHEN** the warning is recorded
- **THEN** the warning is logged at `warn` level via `on_warn` or `on_stderr`
- **AND** the warning is NOT logged at `info` level

#### Scenario: Run stdout suppresses internal diagnostics by default

- **GIVEN** `cflx run` initializes stdout logging for a normal non-interactive run
- **AND** internal code emits `debug!` VCS command logs or dependency `trace!` poller logs
- **WHEN** the run writes terminal-facing stdout logs
- **THEN** stdout does not include `DEBUG Executing git command`
- **AND** stdout does not include `TRACE registering event source with poller`
- **AND** stdout does not include `TRACE deregistering event source from poller`
- **AND** user-facing `INFO`, `WARN`, and `ERROR` logs remain eligible for stdout display

#### Scenario: Persistent logs retain diagnostic detail

- **GIVEN** `cflx run` initializes both stdout and persistent file logging
- **AND** internal code emits `debug!` diagnostic records
- **WHEN** the logging system records events
- **THEN** the stdout filter does not reduce the persistent file logging layer's configured diagnostic level
- **AND** operators can still inspect detailed diagnostics through persistent log files and `cflx logs`

### Requirement: Startup logs include cflx version identity

When Conflux starts a user-facing runtime mode, the startup log MUST include enough version identity to determine which cflx binary produced the log.

The startup log MUST include at least the product name, `CARGO_PKG_VERSION`, and `BUILD_NUMBER`.

#### Scenario: Headless run startup log includes version identity
- **GIVEN** a user starts `cflx run`
- **WHEN** the process emits its startup `info!` log before orchestration begins
- **THEN** at least one startup log entry includes the cflx version and build number
- **AND** the log entry is persisted to the configured log file

#### Scenario: Server startup log includes version identity
- **GIVEN** a user starts `cflx server`
- **WHEN** the server daemon emits its startup `info!` log
- **THEN** the log entry includes the cflx version and build number
- **AND** the entry can be used later to identify which daemon build produced the log

### Requirement: Startup logs identify the runtime mode

Startup logs for user-facing runtime modes MUST identify whether the process started in TUI, run, or server mode.

#### Scenario: Startup log distinguishes mode
- **GIVEN** a user starts either TUI, run, or server mode
- **WHEN** the initial startup log is emitted
- **THEN** the log includes the runtime mode in human-readable form
- **AND** the mode information is visible without requiring correlation with separate events

### Requirement: CLI Log Viewer

Conflux SHALL provide a read-only CLI log viewer that helps users locate, print, and follow existing persistent Conflux log files without knowing the internal state-directory layout.

The CLI log viewer SHALL preserve the existing persistent log file layout and SHALL NOT use log contents or log file presence as authoritative workflow-control input for scheduler, resume, acceptance, archive, merge, or next-action decisions.

#### Scenario: Print selected log path without creating logs

- **GIVEN** a user is in a Conflux workspace
- **WHEN** the user runs `cflx logs --path`
- **THEN** Conflux prints the selected log file path for the current project or selected project slug
- **AND** the command does not create a new log file
- **AND** the command does not append to an existing log file
- **AND** the command does not trigger log cleanup as a side effect of viewing

#### Scenario: Print bounded recent log lines

- **GIVEN** a selected Conflux log file exists with more than `N` lines
- **WHEN** the user runs `cflx logs --last N`
- **THEN** Conflux prints at most the last `N` lines from that file
- **AND** Conflux exits successfully without modifying the file

#### Scenario: Default logs command prints recent bounded tail

- **GIVEN** a selected Conflux log file exists
- **WHEN** the user runs `cflx logs` without a viewing mode
- **THEN** Conflux prints a documented bounded number of recent log lines
- **AND** Conflux does not require the user to know the log directory layout

#### Scenario: Follow appended log lines

- **GIVEN** a selected Conflux log file exists
- **WHEN** the user runs `cflx logs --follow`
- **THEN** Conflux prints recent selected log content
- **AND** Conflux streams lines appended after the command starts until the user interrupts it
- **AND** Conflux does not change workflow state while following logs

#### Scenario: Explicit project slug selection

- **GIVEN** multiple project log directories exist under the Conflux log root
- **WHEN** the user runs `cflx logs --project <slug> --path`
- **THEN** Conflux selects the log directory matching `<slug>`
- **AND** Conflux prints the selected log path for that project slug

#### Scenario: Missing selection lists available projects

- **GIVEN** the current project has no matching log file or the requested project slug does not exist
- **WHEN** the user runs `cflx logs`
- **THEN** Conflux returns an actionable error
- **AND** the output lists available project slugs from the log root when any exist
