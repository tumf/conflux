## ADDED Requirements

### Requirement: REST API - Worktrees List
システムは `GET /api/worktrees` を提供し、TUI Worktrees Viewと同等語彙のworktree一覧スナップショットを返却しなければならない（SHALL）。

#### Scenario: 一覧取得が成功する
- **WHEN** クライアントが `GET /api/worktrees` を呼び出す
- **THEN** サーバーは `200` を返し、worktree配列を返す
- **AND** 各要素はTUIと同等の判定に必要な識別子・状態フィールドを含む

### Requirement: REST API - Worktree Operations
システムはworktreeの操作APIとして `POST /api/worktrees/refresh`, `POST /api/worktrees/create`, `POST /api/worktrees/delete`, `POST /api/worktrees/merge`, `POST /api/worktrees/command` を提供しなければならない（MUST）。

#### Scenario: refreshが成功する
- **WHEN** クライアントが `POST /api/worktrees/refresh` を呼び出す
- **THEN** サーバーは `200` を返し、最新のworktree状態を反映する

#### Scenario: createが成功する
- **GIVEN** 作成前提条件（Git環境・設定）が満たされている
- **WHEN** クライアントが `POST /api/worktrees/create` を呼び出す
- **THEN** サーバーは `200` を返し、新規worktreeを作成する

#### Scenario: 未マージworktreeの削除拒否
- **WHEN** クライアントが未マージのworktreeに対して削除APIを呼び出す
- **THEN** サーバーは `409` を返し、削除を実行しない

#### Scenario: コンフリクトworktreeのマージ拒否
- **WHEN** クライアントが `has_conflicts=true` のworktreeに対してマージAPIを呼び出す
- **THEN** サーバーは `409` を返し、マージを実行しない

#### Scenario: commandが成功する
- **GIVEN** `worktree_command` が設定済みである
- **WHEN** クライアントが `POST /api/worktrees/command` を呼び出す
- **THEN** サーバーは `200` を返し、対象worktreeでコマンドを実行する

#### Scenario: 対象worktreeが存在しない
- **WHEN** クライアントが存在しないworktreeを指定して操作APIを呼び出す
- **THEN** サーバーは `404` を返し、操作を実行しない

### Requirement: WebSocket - Worktree Parity Updates
システムはWebSocketの `state_update.worktrees` に `/api/state` と同等意味のworktreeスナップショットを含め、RESTとWebSocketの状態語彙を一致させなければならない（SHALL）。

#### Scenario: 状態更新でworktreesが同期される
- **WHEN** worktree操作後に `state_update` イベントが配信される
- **THEN** イベントの `worktrees` は同時点の `/api/state` と整合するスナップショットである

#### Scenario: /api/stateにworktreesが反映される
- **WHEN** クライアントが `GET /api/state` を呼び出す
- **THEN** レスポンスの `worktrees` は最新の再取得結果を含む

### Requirement: Dashboard UI - Worktrees View
WebダッシュボードはWorktrees Viewを提供し、一覧表示・操作ガード・削除確認を備えなければならない（SHALL）。

#### Scenario: Worktrees Viewで一覧を表示する
- **WHEN** ユーザーがWebダッシュボードでWorktrees Viewを開く
- **THEN** 各worktreeの主要状態を一覧表示する

#### Scenario: 操作ガードが適用される
- **GIVEN** 選択中worktreeが削除不可またはマージ不可である
- **WHEN** ユーザーがWorktrees Viewを表示する
- **THEN** 対応する操作ボタンは無効化される

#### Scenario: 削除時に確認を要求する
- **WHEN** ユーザーが削除操作を実行する
- **THEN** UIは確認ダイアログを表示し、確認前に削除リクエストを送信しない

### Requirement: Worktree Operations Logging and Failure Policy
システムはWorktree操作失敗を隠蔽してはならない（MUST NOT）。各操作で `request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を構造化ログとして記録し、VCS失敗時は `500` を返さなければならない（MUST）。

#### Scenario: VCS失敗時に500と構造化ログを返す
- **WHEN** create/delete/merge のいずれかでVCS処理が失敗する
- **THEN** サーバーは `500` を返し、`request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を含むエラーログを出力する

#### Scenario: 想定外内部失敗時に500を返す
- **WHEN** refresh/create/delete/merge/command のいずれかで内部例外が発生する
- **THEN** サーバーは `500` を返し、同じ構造化ログ項目を記録する
