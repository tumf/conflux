## MODIFIED Requirements

### Requirement: toggle-change-selection-api

REST API で個別 change の選択状態をトグルできる。

個別 change toggle の成功時、サーバーは dashboard が次回 `full_state` 更新待ちにならずに selection 変化を反映できる即時更新経路を提供しなければならない（SHALL）。

#### Scenario: toggle-individual-change

**Given**: change `foo` が `selected: true` である
**When**: `POST /api/v1/projects/{id}/changes/foo/toggle` が呼ばれる
**Then**: change `foo` の `selected` は `false` になる
**And**: dashboard が次回の定期 refresh や遅延した `full_state` を待たずに反映できる selection update が配信される

#### Scenario: toggle-error-change-back-to-selected-immediately

**Given**: change `foo` の status が `error` であり、`selected: false` である
**When**: `POST /api/v1/projects/{id}/changes/foo/toggle` が呼ばれる
**Then**: change `foo` の `selected` は `true` になる
**And**: change `foo` の status は `error` のまま維持される
**And**: dashboard は次回 `full_state` を待たずに checked state を反映できる
**And**: 次回 Run では change `foo` が再実行対象に含まれる

#### Scenario: toggle-individual-change-failure-does-not-commit-wrong-state

**Given**: dashboard 上で change `foo` に対する selection toggle が開始されている
**And**: サーバー側でその toggle request が失敗する
**When**: クライアントが失敗応答を受け取る
**Then**: server の persisted `selected` state は失敗前の値のままである
**And**: クライアントは最終確定 state を誤って committed success として扱わない

#### Scenario: toggle-all-changes-immediate-selection-update

**Given**: プロジェクトに selected / unselected な change が混在している
**When**: `POST /api/v1/projects/{id}/changes/toggle-all` が呼ばれる
**Then**: サーバーは全対象 change の最終 `selected` 値を確定する
**And**: dashboard が次回 `full_state` を待たずに一覧表示を更新できる selection update を提供する
