## ADDED Requirements
### Requirement: REST API - Worktree一覧取得
HTTPサーバーはworktree一覧を取得するREST APIを提供しなければならない（SHALL）。

#### Scenario: worktree一覧取得
- **WHEN** client sends `GET /api/worktrees`
- **THEN** server responds with HTTP 200 status
- **AND** response body contains an array of worktree objects
- **AND** each object includes `path`, `head`, `branch`, `is_detached`, `is_main`, `merge_conflict`, `has_commits_ahead`, `is_merging`

#### Scenario: main worktreeの識別
- **GIVEN** リポジトリにmain worktreeが存在する
- **WHEN** client sends `GET /api/worktrees`
- **THEN** 少なくとも1件は `is_main = true` を含む

### Requirement: REST API - Worktree操作
HTTPサーバーはWorktrees Viewと同等の操作API（作成/削除/マージ/コマンド実行/再取得）を提供しなければならない（SHALL）。

#### Scenario: worktreeの作成
- **GIVEN** `worktree_command` が設定済みでGitリポジトリである
- **WHEN** client sends `POST /api/worktrees/create`
- **THEN** server creates a new worktree with a unique branch name
- **AND** response status is HTTP 200
- **AND** response body includes created `path` and `branch`

#### Scenario: worktree作成の拒否
- **GIVEN** `worktree_command` が未設定、またはGitリポジトリではない
- **WHEN** client sends `POST /api/worktrees/create`
- **THEN** server responds with HTTP 409
- **AND** no worktree is created

#### Scenario: worktreeの削除
- **GIVEN** 指定したworktreeが削除可能である
- **WHEN** client sends `POST /api/worktrees/delete` with `path`
- **THEN** server removes the worktree and responds with HTTP 200

#### Scenario: worktree削除の禁止
- **GIVEN** 指定したworktreeがmainである、または処理中changeに紐づく
- **WHEN** client sends `POST /api/worktrees/delete` with `path`
- **THEN** server responds with HTTP 409
- **AND** worktree is not removed

#### Scenario: worktreeのマージ
- **GIVEN** 指定したworktreeが衝突なしでaheadである
- **WHEN** client sends `POST /api/worktrees/merge` with `branch`
- **THEN** server merges the branch and responds with HTTP 200

#### Scenario: worktreeのコマンド実行
- **GIVEN** `worktree_command` が設定済みである
- **WHEN** client sends `POST /api/worktrees/command` with `path`
- **THEN** server executes the worktree command and responds with HTTP 200

### Requirement: REST/WS - Worktree状態反映
HTTPサーバーはworktree再取得結果をREST APIとWebSocketの状態更新に反映しなければならない（SHALL）。

#### Scenario: /api/stateにworktreeが含まれる
- **WHEN** client sends `GET /api/state`
- **THEN** response body includes `worktrees`
- **AND** `worktrees` は最新のworktree再取得結果を反映する

#### Scenario: Worktrees再取得の通知
- **WHEN** worktree再取得または操作が完了する
- **THEN** server broadcasts a `state_update` with updated `worktrees`

### Requirement: Dashboard UI - Worktrees View
WebダッシュボードはWorktrees Viewを提供し、TUIと同等の情報と操作を表示しなければならない（SHALL）。

#### Scenario: worktree一覧の表示
- **WHEN** Web UIがWorktrees Viewを表示する
- **THEN** 各worktreeの `path`, `branch`, `head`, `is_main`, `is_detached`, `has_commits_ahead`, `merge_conflict` が表示される

#### Scenario: 操作ボタンのガード
- **GIVEN** 選択中worktreeがmain、または削除/マージ不可である
- **WHEN** Web UIがWorktrees Viewを表示する
- **THEN** 該当操作ボタンは無効化される

#### Scenario: 削除の確認
- **WHEN** Web UIで削除操作を実行する
- **THEN** 確認ダイアログが表示される
