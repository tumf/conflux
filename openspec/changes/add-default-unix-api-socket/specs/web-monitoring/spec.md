## MODIFIED Requirements

### Requirement: HTTP Server Lifecycle

When `web-monitoring` is compiled, a local default TUI, `cflx tui`, or `cflx run` process SHALL start a process-scoped single-instance monitoring and remote-control endpoint on its selected Unix domain socket before effectful startup work. `--web` or retained `web.enabled = true` SHALL additionally start the TCP HTTP/Web UI listener against the same application state. The system SHALL NOT provide a standalone multi-project server daemon.

#### Scenario: Server enabled via CLI flag
- **WHEN** ユーザーが`--web`を指定し、CLIおよび設定ファイルでポートが未指定
- **THEN** TCP HTTPサーバーはOSが割り当てる未使用ポート（ポート0による自動割り当て）で起動する
- **AND** default or explicit UDS remains active unless explicitly opted out
- **AND** 実際のTCPバインド先（アドレス/ポート）がログに表示される
- **AND** オーケストレーターは通常通り動作を継続する

#### Scenario: Server disabled by default
- **WHEN** ユーザーが`--web`を指定せずに実行する
- **THEN** TCP HTTP/Web UI listener does not start
- **AND** no TCP network port is bound for web monitoring
- **AND** the default or explicit Unix API listener still starts unless opted out

#### Scenario: Port already in use
- **WHEN** TCP HTTPサーバーが明示指定されたポートにバインドしようとして、そのポートが使用中
- **THEN** オーケストレーターはポート番号を含む明確なエラーメッセージを出力する
- **AND** already-created listener resources are rolled back
- **AND** オーケストレーターは非ゼロのステータスで終了する

#### Scenario: Graceful shutdown
- **WHEN** オーケストレーターが終了シグナル（Ctrl+CまたはSIGTERM）を受信する
- **THEN** refresh, SSE, and WebSocket producers are cancelled
- **AND** ordinary HTTP requests receive a bounded grace period
- **AND** remaining listener tasks are stopped and awaited after the deadline
- **AND** the owned Unix socket is cleaned up
- **AND** オーケストレーターは正常に終了する

#### Scenario: Run mode success shuts down web monitoring
- **GIVEN** ユーザーが `cflx run` を実行し、Unix listenerと任意のTCP listenerが起動している
- **AND** オーケストレーションが成功裏に完了する
- **WHEN** run モードが成功終了へ遷移する
- **THEN** run モードが起動した全listener、stream producer、refresh taskは停止する
- **AND** owned Unix socket is cleaned up
- **AND** プロセスは追加の外部シグナルなしで正常終了する

#### Scenario: No standalone daemon lifecycle
- **GIVEN** the installed CLI
- **WHEN** a user requests CLI help
- **THEN** no standalone multi-project server command or service lifecycle is advertised

#### Scenario: Default Unix API starts before effectful startup
- **GIVEN** a Linux or macOS web-enabled build inside a Git repository
- **WHEN** a local orchestration-owning invocation starts without Unix override or opt-out
- **THEN** `/api/v2` binds `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** endpoint publication completes before effectful upstream preparation, lifecycle adapters, AI subprocesses, or orchestration

#### Scenario: Feature-disabled build remains API-free
- **GIVEN** Conflux is compiled without `web-monitoring`
- **WHEN** local TUI or run starts
- **THEN** it retains existing API-free behavior
- **AND** no Unix listener contract or Unix-only CLI flag applies

### Requirement: Configuration Options

The HTTP monitoring server SHALL remain configurable by its retained CLI options and `web` configuration. On Linux and macOS web-enabled builds, the local API SHALL additionally support the default repository-scoped Unix listener, an explicit Unix path override, and an explicit Unix opt-out. Removed multi-project `server.*` configuration SHALL NOT be required.

#### Scenario: Port configuration via CLI
- **WHEN** ユーザーが`--web --web-port 3000`で実行する
- **THEN** TCP HTTPサーバーはデフォルトではなくポート3000にバインドする
- **AND** the selected Unix listener remains active unless opted out

#### Scenario: Auto port selection by default
- **WHEN** TCP web monitoring is enabled and CLIと設定ファイルの両方でポートが未指定
- **THEN** TCP HTTPサーバーはOSが割り当てる未使用ポートで起動する
- **AND** 実際のバインド先がログとendpoint metadataに表示される

#### Scenario: Configuration via config file
- **WHEN** 設定ファイルに`web.enabled = true`と`web.port = 9000`がある
- **THEN** CLIフラグがなくてもTCP HTTPサーバーはポート9000で起動する
- **AND** the selected Unix listener also starts unless opted out
- **AND** CLIで指定した値は設定ファイルより優先される

#### Scenario: Retained web options configure listener
- **GIVEN** a local TUI or run invocation
- **WHEN** the user supplies retained web bind, port, token, token-env, or allowed-origin options
- **THEN** bind, port, and browser-origin values configure the retained TCP listener under existing validation rules
- **AND** resolved authentication applies to every active listener

#### Scenario: Server configuration is absent
- **GIVEN** a configuration without obsolete `server.*` fields
- **WHEN** local web monitoring starts
- **THEN** startup does not require multi-project server configuration

#### Scenario: Explicit Unix path overrides default
- **GIVEN** a Linux or macOS web-enabled build
- **WHEN** the user supplies `--web-unix-socket PATH`
- **THEN** UDS binds the validated absolute `PATH` instead of `${GIT_COMMON_DIR}/cflx-api.sock`

#### Scenario: Explicit Unix opt-out
- **GIVEN** a Linux or macOS web-enabled build
- **WHEN** the user supplies `--no-web-unix-socket`
- **THEN** no Unix listener starts
- **AND** retained TCP web monitoring may still start

#### Scenario: Non-Git default is rejected
- **GIVEN** a Linux or macOS web-enabled orchestration invocation outside Git
- **WHEN** neither explicit Unix path nor opt-out is supplied
- **THEN** startup fails before effectful work
- **AND** the error explains both supported choices
