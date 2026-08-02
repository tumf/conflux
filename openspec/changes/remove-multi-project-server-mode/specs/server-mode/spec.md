## REMOVED Requirements

### Requirement: サーバ起動はグローバル設定のみを使用する

Removed with the standalone server product.

#### Scenario: Server command is absent

**Given**: The retained CLI
**When**: commands are enumerated
**Then**: No standalone server command exists

### Requirement: 非ループバック bind は bearer token 必須

Removed with standalone server binding.

#### Scenario: No standalone server binding

**Given**: The retained CLI
**When**: listeners are enumerated
**Then**: No standalone server listener exists

### Requirement: プロジェクト識別子と永続化

Removed with the server project registry.

#### Scenario: No server project registry

**Given**: Production modules
**When**: persistence owners are inspected
**Then**: No server project registry exists

### Requirement: リポジトリ操作の排他

Removed with server-managed repository operations.

#### Scenario: No server repository operation

**Given**: The retained API
**When**: operations are enumerated
**Then**: No server project operation exists

### Requirement: API v1 を提供する

The obsolete multi-project API v1 is removed; retained `/api/v2` remains governed separately.

#### Scenario: No multi-project API v1

**Given**: The retained router
**When**: routes are enumerated
**Then**: No multi-project API v1 route exists

### Requirement: Git 同期の非 fast-forward を明示エラーにする

Removed with the server Git-sync API.

#### Scenario: No server Git-sync error contract

**Given**: The retained API
**When**: routes are enumerated
**Then**: No server Git-sync route exists

### Requirement: グローバル同時実行上限

Removed with global server orchestration.

#### Scenario: No server-global concurrency

**Given**: A local run
**When**: concurrency is configured
**Then**: No server-global project limit applies

### Requirement: プロジェクト追加時の自動クローン

Removed with server project registration.

#### Scenario: No server auto-clone

**Given**: The retained CLI
**When**: projects are managed
**Then**: No server registration auto-clones repositories

### Requirement: Git 同期の auto_resolve オプション

Removed with server Git synchronization.

#### Scenario: No server auto-resolve option

**Given**: Current configuration
**When**: supported fields are inspected
**Then**: No server Git auto-resolve option exists

### Requirement: サーバの auto_resolve は共通 resolve_command を使用する

Removed with server auto-resolve.

#### Scenario: No server resolve invocation

**Given**: A local run
**When**: resolve behavior executes
**Then**: No server auto-resolve path is invoked

### Requirement: Git 同期の統合 API

Removed with the multi-project server API.

#### Scenario: No integrated server Git-sync API

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server Git-sync API exists

### Requirement: Git 同期の resolve 必須化

Removed with the multi-project Git-sync API.

#### Scenario: No server Git-sync resolve gate

**Given**: The retained API
**When**: operations are enumerated
**Then**: No server Git-sync resolve gate exists

### Requirement: リモートTUI向けのログ配信

Removed with remote TUI mode.

#### Scenario: No remote TUI log stream

**Given**: Local TUI
**When**: log sources are enumerated
**Then**: No server log stream is consumed

### Requirement: Service Start Enforces Server Security Validation

Removed with OS service management.

#### Scenario: No service start command

**Given**: The retained CLI
**When**: commands are enumerated
**Then**: No server service-start command exists

### Requirement: Version エンドポイントを認証なしで提供する

Removed with API v1; retained API v2 contracts remain unchanged.

#### Scenario: No server API v1 version endpoint

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server API v1 version endpoint exists

### Requirement: server-log-path

Removed with the standalone server daemon.

#### Scenario: No server daemon log path

**Given**: Current configuration
**When**: log destinations are inspected
**Then**: No standalone server log path is configured

### Requirement: Periodic remote sync-state monitoring

Removed with remote project monitoring.

#### Scenario: No periodic remote sync monitor

**Given**: A local process
**When**: background tasks are enumerated
**Then**: No server project sync monitor exists

### Requirement: Monitoring must be non-invasive

Removed with remote server monitoring.

#### Scenario: No remote monitoring process

**Given**: A local process
**When**: background tasks are enumerated
**Then**: No server project monitor runs
