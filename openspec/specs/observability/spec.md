# Observability Specification

## Purpose

This specification defines the logging and observability requirements for the Conflux orchestrator. It ensures that all command executions, TUI events, and system operations are properly logged for debugging and troubleshooting purposes.

The specification covers:
- Command execution logging (VCS, AI agents, hooks)
- TUI log synchronization to debug files
- Log level classification and formatting standards
## Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

Conflux observability MUST distinguish recoverable degraded execution paths from terminal workflow failures across tracing records and runtime events. Equivalent recoverable fallback diagnostics MUST be deduplicated consistently across both tracing records and runtime events during the existing deduplication lifetime. The bundled log mining helper MUST remain observability-only and MUST NOT influence scheduler decisions, resume routing, acceptance, archive, merge, or next-action behavior.

VCS simulation diagnostics whose child output size is not intrinsically bounded MUST record structured summaries instead of complete raw stdout/stderr. A summary SHALL retain command outcome, output byte counts, worktree or branch identity when available, conflict count, at most 20 deterministic conflict paths, and at most 4096 bytes of each stdout/stderr prefix. Known merge conflicts SHALL remain ordinary conflict observations and SHALL NOT emit unbounded fallback output on each refresh.

<!-- Expected canonical result after archive: observability retains actionable merge-simulation evidence without allowing repeated child output to grow persistent logs without bound. -->

#### Scenario: recoverable analysis fallback is not presented as terminal failure

- **GIVEN** dependency analysis rejects an LLM-produced graph
- **AND** Conflux successfully constructs metadata-dependency-only fallback analysis
- **WHEN** runtime diagnostics and events are emitted
- **THEN** the fallback remains visible as a warning-level degraded analysis diagnostic
- **AND** the diagnostic identifies metadata dependency fallback and preserves the original analysis failure reason
- **AND** the successful fallback emits no error-level tracing record or terminal error event
- **AND** repeated equivalent tracing records and runtime events are each deduplicated by the same diagnostic signature
- **AND** missing or rejected dependency blockers remain visible as actionable diagnostics
- **AND** observability output does not affect workflow-control decisions

#### Scenario: changed fallback signature remains visible

- **GIVEN** a recoverable fallback diagnostic has already been emitted for one queued set, in-flight set, and normalized rejection reason
- **WHEN** a later fallback has a different rejection reason or queued/in-flight context
- **THEN** a new warning tracing record is emitted
- **AND** a new warning runtime event is emitted
- **AND** the changed context is preserved for operator diagnosis

#### Scenario: fallback preserves safe dependency execution

- **GIVEN** an LLM analysis response is invalid or omits queued change IDs
- **AND** queued changes declare proposal metadata dependencies
- **WHEN** Conflux rejects the LLM response and uses fallback analysis
- **THEN** every queued change remains represented exactly once in fallback order
- **AND** declared metadata dependencies remain present
- **AND** dispatch continues to fail closed for missing or rejected dependency targets

#### Scenario: Large merge conflict output is bounded

- **GIVEN** `git merge-tree` returns conflict output larger than the diagnostic sample limit
- **WHEN** Conflux records the conflict observation
- **THEN** the diagnostic contains the exit status, total stdout/stderr byte counts, conflict count, worktree identity, and deterministic bounded sample
- **AND** the diagnostic does not contain the complete raw stdout or stderr

#### Scenario: Repeated unchanged conflict does not flood logs

- **GIVEN** an eligible worktree conflict has already been observed for one unchanged revision tuple
- **WHEN** periodic refresh repeats without branch identity, base HEAD, worktree HEAD, or merge-base changes
- **THEN** no duplicate merge simulation output is logged
- **AND** the retained observation remains available to the Worktrees view

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

ツール関連の非テキストイベントについて、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。代わりに、`tool_use` / `tool_result` ごとに1件の意味的な要約を表示しなければならない (MUST)。`tool_result` prefix は利用可能な場合に `[tool_result:<tool_use_id>]`、IDがない場合に `[tool_result]` としなければならない (MUST)。

要約はイベントの `name`、許可された `input` scalar、`result` から主要フィールドを抽出しなければならない (MUST)。表示可能な `tool_use` scalar と `tool_result` content は、TUI/CLI表示より前に60、80、100、200文字等の固定表示長で省略してはならない (MUST NOT)。write/edit系bodyは本文を含めず安全なmetadataへ置換しなければならず (MUST)、raw JSON suppression と既存のprivacy redactionを維持しなければならない (MUST)。

prefixを含む完成後のsummary全体は、CLI/TUIへ分岐する前に共有operator-facing sanitizationと8,192-byte safety boundを論理的に一度だけ適用されなければならない (MUST)。後続の `LogEntry` constructionは既にsanitized/boundedなsummaryに対してidempotentでなければならず (MUST)、二重truncateによってmarkerを置換してはならない (MUST NOT)。上限超過時、最終messageはUTF-8境界を壊さず、完成後summary全体から実際に省略されたbyte数を示すmarkerを含まなければならない (MUST)。同じ最終summary representationを非TUI CLI出力とTUI `LogEntry`に渡さなければならない (MUST)。

#### Scenario: tool_use が1件の幅非依存サマリとして保持される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout がstream-jsonの `tool_use` eventを出力し、表示可能なscalar fieldが従来の固定長上限を超える
- **WHEN** オーケストレーターがstdoutをtextifyする
- **THEN** stdoutの生JSON行は表示されない
- **AND** `[tool_use:<name>]` で始まる1件のsummaryが生成される
- **AND** redaction policyで許可されたscalarは60、80、100文字等の固定位置で `...` に置換されない
- **AND** write/edit body contentはsummaryに含まれない

#### Scenario: assistant message 内の tool_use ブロックも同じポリシーを使う

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスのstdoutがstream-jsonの `assistant` eventを出力し、`message.content[]` に `tool_use` blockを含む
- **WHEN** オーケストレーターがstdoutをtextifyする
- **THEN** tool_use blockは生JSONとして表示されない
- **AND** top-level tool_useと同じretention、redaction、sanitization、bound policyの1件のsummaryが表示される

#### Scenario: 200文字を超えるtool_resultは表示前に失われない

- **GIVEN** `stream_json_textify=true` である
- **AND** `tool_result` contentが200文字を超え、完成後summary全体が共有safety bound未満である
- **WHEN** オーケストレーターがeventをtextifyしてoperator-facing outputへ渡す
- **THEN** `[tool_result:<tool_use_id>]` で始まるsummaryが生成される
- **AND** contentは200文字地点で `...` に置き換えられない
- **AND** CLI outputとTUI `LogEntry.message`は同じ保持済みcontentを含む

#### Scenario: 巨大な完成後summaryは一度だけ正確に抑制される

- **GIVEN** `stream_json_textify=true` である
- **AND** prefixを含む完成後tool-event summaryが共有operator-facing safety boundを超える
- **WHEN** summaryがCLI/TUI consumerへ渡され、TUIでは `LogEntry` が構築される
- **THEN** 最終messageは8,192 bytes以内に収まる
- **AND** UTF-8境界は壊れない
- **AND** markerは完成後summary全体から実際に省略されたbyte数を示す
- **AND** `LogEntry` constructionはmarkerを別の二次truncate markerへ置換しない

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

When Conflux starts a retained user-facing runtime mode, the startup log MUST include enough version identity to determine which cflx binary produced the log.

The startup log MUST include at least the product name, `CARGO_PKG_VERSION`, and `BUILD_NUMBER`.

#### Scenario: Headless run startup log includes version identity
- **GIVEN** a user starts `cflx run`
- **WHEN** the process emits its startup `info!` log before orchestration begins
- **THEN** at least one startup log entry includes the cflx version and build number
- **AND** the log entry is persisted to the configured log file

### Requirement: Startup logs identify the runtime mode

Startup logs for retained user-facing runtime modes MUST identify whether the process started in TUI or run mode.

#### Scenario: Startup log distinguishes retained mode
- **GIVEN** a user starts either TUI or run mode
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

### Requirement: Final commit hook output MUST stream with structured context

Conflux MUST stream final Apply commit stdout and stderr to user-visible TUI output and persistent logs while retaining the complete raw command result required for repository rejection and index-lock classification. Each emitted line MUST identify the change, `commit` operation, source stream, and finalization attempt. Presentation MAY strip ANSI control sequences, but classification buffers MUST preserve raw output. Separate retry attempts MUST remain distinguishable and MUST NOT be silently deduplicated.

#### Scenario: Successful pre-commit progress is visible

**Given**: a hook-enabled final Apply commit writes progress to stdout or stderr and later succeeds
**When**: Conflux executes the commit
**Then**: each progress line is visible in normal TUI output and persistent logs before or at process completion
**And**: each line identifies the change, commit operation, stream, and attempt

#### Scenario: Hook rejection retains full and bounded evidence

**Given**: a final commit hook emits diagnostics and rejects the commit
**When**: Conflux records the failure
**Then**: persistent logs retain the complete streamed output
**And**: the next Apply prompt receives only bounded diagnostic tails under the existing prompt budget
**And**: typed rejection uses the preserved exit status and raw streams

#### Scenario: Index-lock retry output remains attributable

**Given**: final commit encounters eligible managed-worktree index-lock contention
**When**: Conflux retries finalization
**Then**: output from each attempt is labeled with its attempt number
**And**: repeated lines from separate attempts are not removed as duplicates
**And**: the complete raw stderr remains available to the existing lock classifier

#### Scenario: Long-running silent hook remains observable

**Given**: final commit hooks are still running but have emitted no recent output
**When**: the TUI renders commit progress
**Then**: the operator can see that pre-commit remains active under the commit phase
**And**: the presentation does not fabricate hook success or alter workflow-control state
