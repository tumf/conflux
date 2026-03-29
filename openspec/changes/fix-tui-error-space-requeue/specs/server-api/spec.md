## MODIFIED Requirements

### Requirement: change-selection-state

サーバーは各プロジェクトの各 change について `selected: bool` 状態をインメモリで管理する。

Error 状態の change についても `selected` は同じフィールドで表現されなければならない（SHALL）。

change が Error になった時点では、その change の `selected` は `false` でなければならない（SHALL）。

ユーザーが Error change を再度 `selected: true` にした場合、その change は次回 Run の再実行対象として扱われなければならない（SHALL）。

#### Scenario: new-change-defaults-to-selected

**Given**: プロジェクトが登録されている
**When**: 新しい change が検出される
**Then**: その change の `selected` は `true` に設定される

#### Scenario: server-restart-resets-selection

**Given**: サーバーが再起動される
**When**: change 一覧が初期化される
**Then**: 全 change の `selected` は `true` に設定される

#### Scenario: error-change-defaults-to-unselected

**Given**: ある change が Error 状態に遷移した
**When**: サーバーがその change を状態スナップショットに含める
**Then**: その change の `selected` は `false` である

#### Scenario: remarking-error-change-makes-it-runnable-again

**Given**: Error 状態の change `foo` が `selected: false` である
**When**: ユーザーが toggle API もしくは同等の UI 操作で `selected: true` に戻す
**Then**: 次回の Run で change `foo` は再実行対象に含まれる

### Requirement: toggle-change-selection-api

REST API で個別 change の選択状態をトグルできる。

Error 状態の change に対しても同じ API を使わなければならない（SHALL）。

#### Scenario: toggle-individual-change

**Given**: change `foo` が `selected: true` である
**When**: `POST /api/v1/projects/{id}/changes/foo/toggle` が呼ばれる
**Then**: change `foo` の `selected` は `false` になり、WebSocket で `change_update` が配信される

#### Scenario: toggle-all-changes

**Given**: プロジェクトに change `foo` (selected: true) と `bar` (selected: false) がある
**When**: `POST /api/v1/projects/{id}/changes/toggle-all` が呼ばれる
**Then**: 1つでも未選択があるため、全 change が `selected: true` になる

#### Scenario: toggle-error-change-back-to-selected

**Given**: Error 状態の change `foo` が `selected: false` である
**When**: `POST /api/v1/projects/{id}/changes/foo/toggle` が呼ばれる
**Then**: change `foo` の `selected` は `true` になる
- **AND** 次回 Run では再実行対象に含まれる
