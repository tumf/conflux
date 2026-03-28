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
