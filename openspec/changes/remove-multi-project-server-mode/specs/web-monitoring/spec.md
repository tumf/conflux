## MODIFIED Requirements

### Requirement: HTTP Server Lifecycle

When web monitoring is enabled for a local TUI or `cflx run` process, the HTTP server SHALL start as a process-scoped single-instance monitoring and remote-control endpoint. The system SHALL NOT provide a standalone multi-project server daemon.

#### Scenario: Server enabled via CLI flag
- **WHEN** ユーザーが`--web`を指定し、CLIおよび設定ファイルでポートが未指定
- **THEN** HTTPサーバーはOSが割り当てる未使用ポート（ポート0による自動割り当て）で起動する
- **AND** 実際のバインド先（アドレス/ポート）がログに表示される
- **AND** オーケストレーターは通常通り動作を継続する

#### Scenario: Server disabled by default
- **WHEN** ユーザーが`--web`を指定せずに実行する
- **THEN** HTTPサーバーは起動しない
- **AND** ネットワークポートはバインドされない

#### Scenario: Port already in use
- **WHEN** HTTPサーバーが明示指定されたポートにバインドしようとして、そのポートが使用中
- **THEN** オーケストレーターはポート番号を含む明確なエラーメッセージを出力する
- **AND** オーケストレーターは非ゼロのステータスで終了する

#### Scenario: Graceful shutdown
- **WHEN** オーケストレーターが終了シグナル（Ctrl+C）を受信する
- **THEN** HTTPサーバーはアクティブな接続を穏やかに閉じる
- **AND** オーケストレーターは進行中のリクエスト完了を待機する
- **AND** オーケストレーターは正常に終了する

#### Scenario: Run mode success shuts down web monitoring
- **GIVEN** ユーザーが `cflx run --web` を実行している
- **AND** オーケストレーションが成功裏に完了する
- **WHEN** run モードが成功終了へ遷移する
- **THEN** run モードが起動したHTTPサーバーと関連バックグラウンドタスクは停止する
- **AND** プロセスは追加の外部シグナルなしで正常終了する

#### Scenario: No standalone daemon lifecycle
- **GIVEN** the installed CLI
- **WHEN** a user requests CLI help
- **THEN** no standalone multi-project server command or service lifecycle is advertised

### Requirement: Configuration Options

The HTTP monitoring server SHALL remain configurable by its retained CLI options and `web` configuration. Removed multi-project `server.*` configuration SHALL NOT be required to start or use local web monitoring.

#### Scenario: Port configuration via CLI
- **WHEN** ユーザーが`--web --web-port 3000`で実行する
- **THEN** HTTPサーバーはデフォルトではなくポート3000にバインドする

#### Scenario: Auto port selection by default
- **WHEN** CLIと設定ファイルの両方でポートが未指定
- **THEN** HTTPサーバーはOSが割り当てる未使用ポートで起動する
- **AND** 実際のバインド先がログに表示される

#### Scenario: Configuration via config file
- **WHEN** 設定ファイルに`web.enabled = true`と`web.port = 9000`がある
- **THEN** CLIフラグがなくてもHTTPサーバーはポート9000で起動する
- **AND** CLIで指定した値は設定ファイルより優先される

#### Scenario: Retained web options configure listener
- **GIVEN** a local TUI or run invocation
- **WHEN** the user supplies retained web bind, port, token, token-env, or allowed-origin options
- **THEN** those values configure the process-scoped web listener under existing validation rules

#### Scenario: Server configuration is absent
- **GIVEN** a configuration without obsolete `server.*` fields
- **WHEN** local web monitoring starts
- **THEN** startup does not require multi-project server configuration
