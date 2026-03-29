## Requirements

### Requirement: change-selection-state

サーバーは各プロジェクトの各 change について `selected: bool` 状態をインメモリで管理する。

#### Scenario: new-change-defaults-to-selected

**Given**: プロジェクトが登録されている
**When**: 新しい change が検出される
**Then**: その change の `selected` は `true` に設定される

#### Scenario: server-restart-resets-selection

**Given**: サーバーが再起動される
**When**: change 一覧が初期化される
**Then**: 全 change の `selected` は `true` に設定される

### Requirement: toggle-change-selection-api

REST API で個別 change の選択状態をトグルできる。

#### Scenario: toggle-individual-change

**Given**: change `foo` が `selected: true` である
**When**: `POST /api/v1/projects/{id}/changes/foo/toggle` が呼ばれる
**Then**: change `foo` の `selected` は `false` になり、WebSocket で `change_update` が配信される

#### Scenario: toggle-all-changes

**Given**: プロジェクトに change `foo` (selected: true) と `bar` (selected: false) がある
**When**: `POST /api/v1/projects/{id}/changes/toggle-all` が呼ばれる
**Then**: 1つでも未選択があるため、全 change が `selected: true` になる

### Requirement: selected-field-in-websocket

WebSocket メッセージの `RemoteChange` に `selected` フィールドが含まれる。

#### Scenario: full-state-includes-selected

**Given**: WebSocket 接続が確立されている
**When**: `full_state` メッセージが送信される
**Then**: 各 `RemoteChange` に `selected: bool` フィールドが含まれる

### Requirement: dashboard-change-checkbox

ダッシュボードの change 行にチェックボックスが表示される。

#### Scenario: checkbox-toggles-selection

**Given**: ダッシュボードに change が表示されている
**When**: ユーザーがチェックボックスをクリックする
**Then**: toggle API が呼ばれ、チェックボックスの表示が更新される

### Requirement: global-orchestration-status

サーバーはグローバルなオーケストレーション状態 (Idle/Running/Stopped) を管理する。

#### Scenario: initial-status-is-idle

**Given**: サーバーが起動した
**When**: 初期状態を確認する
**Then**: `orchestration_status` は `idle` である

#### Scenario: status-changes-to-running-on-run

**Given**: `orchestration_status` が `idle` または `stopped` である
**When**: `POST /api/v1/control/run` が呼ばれる
**Then**: `orchestration_status` は `running` に変わる

#### Scenario: status-changes-to-stopped-on-stop

**Given**: `orchestration_status` が `running` である
**When**: `POST /api/v1/control/stop` が呼ばれる
**Then**: 全プロジェクトが graceful stop され、`orchestration_status` は `stopped` に変わる

### Requirement: global-run-uses-selected-changes

グローバル Run は各プロジェクトの `selected: true` な change のみを実行対象にする。

#### Scenario: run-spawns-only-selected-changes

**Given**: プロジェクト A に change `foo` (selected: true) と `bar` (selected: false) がある
**When**: `POST /api/v1/control/run` が呼ばれる
**Then**: プロジェクト A の runner は `cflx run --change foo` で起動される

#### Scenario: run-skips-project-with-no-selected-changes

**Given**: プロジェクト B の全 change が `selected: false` である
**When**: `POST /api/v1/control/run` が呼ばれる
**Then**: プロジェクト B の runner は起動されない

### Requirement: auto-enqueue-new-projects-during-run

Running 中に追加されたプロジェクトは自動的にオーケストレーションに参加する。

#### Scenario: new-project-auto-starts

**Given**: `orchestration_status` が `running` である
**When**: 新しいプロジェクトが追加される
**Then**: そのプロジェクトの selected change を対象に runner が自動的に spawn される

### Requirement: websocket-orchestration-status

WebSocket `full_state` メッセージに `orchestration_status` フィールドが含まれる。

#### Scenario: full-state-includes-orchestration-status

**Given**: WebSocket 接続が確立されている
**When**: `full_state` メッセージが送信される
**Then**: メッセージに `orchestration_status: "idle" | "running" | "stopped"` が含まれる

### Requirement: dashboard-global-run-stop

ダッシュボードの Header にグローバル Run/Stop ボタンが配置される。

#### Scenario: run-button-starts-orchestration

**Given**: `orchestration_status` が `idle` または `stopped` である
**When**: ユーザーが Run ボタンをクリックする
**Then**: `POST /api/v1/control/run` が呼ばれる

#### Scenario: stop-button-stops-orchestration

**Given**: `orchestration_status` が `running` である
**When**: ユーザーが Stop ボタンをクリックする
**Then**: `POST /api/v1/control/stop` が呼ばれる

### Requirement: per-project-control-run

プロジェクト単位の `POST /projects/{id}/control/run` エンドポイントを廃止する。グローバル Run に置き換えられた。

### Requirement: per-project-control-stop

プロジェクト単位の `POST /projects/{id}/control/stop` エンドポイントを廃止する。グローバル Stop に置き換えられた。

### Requirement: per-project-control-retry

プロジェクト単位の `POST /projects/{id}/control/retry` エンドポイントを廃止する。リトライはグローバル Run で代替される。

### Requirement: per-project-run-stop-buttons

`ProjectCard` の Run/Stop/Retry ボタンを削除する。グローバル Header のボタンに置き換えられた。


#


#


#
