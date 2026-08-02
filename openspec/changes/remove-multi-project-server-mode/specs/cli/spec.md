## REMOVED Requirements

### Requirement: server サブコマンド

The obsolete multi-project server daemon command is removed.

#### Scenario: Removed server command is rejected

**Given**: A user invokes `cflx server`
**When**: CLI parsing runs
**Then**: The invocation fails as an unknown command before side effects

### Requirement: リモートサーバ指定フラグ

Remote-server TUI mode is removed.

#### Scenario: Removed server option is rejected

**Given**: A user invokes the default or explicit TUI with `--server`
**When**: CLI parsing runs
**Then**: The invocation fails as an unknown option before side effects

### Requirement: リモートサーバ認証トークン

Remote-server authentication options are removed with remote TUI mode.

#### Scenario: Removed token options are rejected

**Given**: A user supplies `--server-token` or `--server-token-env`
**When**: CLI parsing runs
**Then**: The invocation fails as an unknown option before side effects

### Requirement: server データディレクトリの CLI 上書き

The server data-directory override is removed with the server command.

#### Scenario: Removed data directory surface is unavailable

**Given**: A user invokes `cflx server --data-dir PATH`
**When**: CLI parsing runs
**Then**: The server command is rejected

### Requirement: server の resolve_command フラグは受け付けない

The server-specific rejection contract is unnecessary because the server command no longer exists.

#### Scenario: Removed server command rejects all arguments

**Given**: A user invokes `cflx server --resolve-command true`
**When**: CLI parsing runs
**Then**: The server command is rejected as unknown

### Requirement: Project サブコマンドによるサーバプロジェクト管理

Server project management is removed.

#### Scenario: Removed project command is rejected

**Given**: A user invokes `cflx project`
**When**: CLI parsing runs
**Then**: The invocation fails as an unknown command before network access

### Requirement: Project サブコマンドの接続先解決と認証非対応

Project server endpoint resolution is removed with project management.

#### Scenario: Removed project endpoint option is unavailable

**Given**: A user invokes `cflx project --server URL status`
**When**: CLI parsing runs
**Then**: The project command is rejected before configuration or network access

### Requirement: service Subcommand Manages Background Server

Background service management for the obsolete daemon is removed.

#### Scenario: Removed service command is rejected

**Given**: A user invokes `cflx service`
**When**: CLI parsing runs
**Then**: The invocation fails as an unknown command before OS service-manager access

### Requirement: project sync --all による全件同期

Multi-project bulk synchronization is removed.

#### Scenario: Removed bulk sync is unavailable

**Given**: A user invokes `cflx project sync --all`
**When**: CLI parsing runs
**Then**: The project command is rejected before network or git access

### Requirement: project add のブランチ表記を URL から解決する

Server project URL branch parsing is removed with project management.

#### Scenario: Removed project add is unavailable

**Given**: A user invokes `cflx project add URL`
**When**: CLI parsing runs
**Then**: The project command is rejected before URL resolution

### Requirement: project add のデフォルトブランチ解決

Remote default-branch discovery for server project registration is removed.

#### Scenario: Removed default branch discovery is unavailable

**Given**: A user invokes `cflx project add URL` without a branch
**When**: CLI parsing runs
**Then**: The project command is rejected before remote discovery
