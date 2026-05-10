# Observability Specification

## Purpose

This specification defines the logging and observability requirements for the Conflux orchestrator. It ensures that all command executions, TUI events, and system operations are properly logged for debugging and troubleshooting purposes.

The specification covers:
- Command execution logging (VCS, AI agents, hooks)
- TUI log synchronization to debug files
- Log level classification and formatting standards
## Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

The bundled Conflux log mining helper MUST be able to scan marker-selected runtime logs incrementally for actionable errors, manual operation markers, and resolve/merge timeline markers without requiring whole-file buffering. The helper output MUST remain observability-only and MUST NOT be used as a workflow-control input.

#### Scenario: large marker-selected log is mined without whole-file buffering

- **GIVEN** a log root contains `.last-checked` and a large `.log` file whose mtime is newer than the marker
- **WHEN** an operator runs `python3 scripts/cflx-log-mine.py --log-root <log-root> --top 30`
- **THEN** the helper emits the standard report sections for top error/warning groups, manual operation markers, action timeline markers, and recommended follow-up queries
- **AND** the scanner does not need to read the entire log file into memory before processing hits
- **AND** no mined log output affects scheduling, resume routing, acceptance, archive, merge, or next-action behavior

#### Scenario: change-id filtering remains compatible under streaming scan

- **GIVEN** a marker-selected log contains events for change `alpha` and unrelated events for change `beta`
- **WHEN** an operator runs `python3 scripts/cflx-log-mine.py --log-root <log-root> --change-id alpha --format json`
- **THEN** returned grouped examples, manual events, and action events are limited to hits whose text or captured context includes `alpha`
- **AND** the JSON report keeps the existing top-level keys used by consumers

#### Scenario: grouped diagnostics continue to normalize volatile local details

- **GIVEN** a marker-selected log line contains volatile values such as local absolute paths, process ids, project ids, branch names, or change ids
- **WHEN** the helper groups the diagnostic
- **THEN** the group key normalizes those volatile values consistently with existing behavior
- **AND** the helper does not write confidential mined log content into repository-tracked proposal or test artifacts

TUI Logs View は、依存未解決のまま進展しない queued change に対する repeated `AnalysisStarted` / `DependencyBlocked` event を受け取っても、同一状態の `Re-analyzing queued changes for dispatch` および `Change '<id>' blocked by dependencies` entries を無制限に追加してはならない（MUST NOT）。

#### Scenario: repetitive scheduler diagnostics are bounded in TUI logs and debug files

- **GIVEN** a scheduler diagnostic has the same change id, reason, and message across repeated loop iterations
- **WHEN** the diagnostic is emitted repeatedly without any relevant state change
- **THEN** the TUI Logs View does not show an unbounded sequence of identical entries
- **AND** the debug log file does not show an unbounded sequence of identical WARN-level entries for the same scheduler diagnostic
- **AND** the diagnostic remains available at least once or through a summary/rate-limited entry
- **AND** suppression state is not used to decide scheduling, resume routing, acceptance, archive, or next-action behavior

#### Scenario: dependency-blocked TUI logs are bounded while blocked state is unchanged

- **GIVEN** queued change `alpha` has already been displayed as `blocked`
- **AND** the TUI has already appended `Change 'alpha' blocked by dependencies`
- **WHEN** repeated `DependencyBlocked` events for `alpha` arrive without an intervening dependency resolution or display-state change
- **THEN** the TUI keeps `alpha` displayed as `blocked`
- **AND** the TUI does not append additional identical blocked log entries for `alpha`
- **AND** the suppression state is not used to decide scheduler dispatch, resume routing, acceptance, archive, or next-action behavior

#### Scenario: analysis-started TUI logs are bounded while remaining work is unchanged

- **GIVEN** the TUI has already appended `Re-analyzing queued changes for dispatch (remaining: 1)`
- **WHEN** repeated `AnalysisStarted { remaining_changes: 1 }` events arrive without relevant progress or state reset
- **THEN** the TUI does not append additional identical re-analysis log entries
- **AND** a changed remaining count or meaningful progress/reset event can make a later analysis-started log visible again

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
