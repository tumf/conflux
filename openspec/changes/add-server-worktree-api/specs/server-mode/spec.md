## ADDED Requirements

### Requirement: プロジェクトスコープのWorktree管理API

サーバはプロジェクトごとのWorktree一覧取得・作成・削除・ブランチマージのAPIを提供しなければならない（MUST）。

#### Scenario: Worktree一覧を取得する
- **WHEN** クライアントが `GET /api/v1/projects/{id}/worktrees` を呼び出す
- **THEN** サーバは該当プロジェクトのWorktree一覧を返す
- **AND** 各Worktreeにはブランチ名、コンフリクト情報、先行コミット有無が含まれる

#### Scenario: Worktreeを作成する
- **WHEN** クライアントが `POST /api/v1/projects/{id}/worktrees` に `change_id` を送信する
- **THEN** サーバは新しいWorktreeを作成する
- **AND** セットアップスクリプトが存在する場合は実行する
- **AND** 作成されたWorktree情報を返す

#### Scenario: Worktreeを削除する
- **GIVEN** 対象Worktreeがメインでなく、detachedでなく、active changeでない
- **WHEN** クライアントが `DELETE /api/v1/projects/{id}/worktrees/{branch}` を呼び出す
- **THEN** サーバはWorktreeと関連ブランチを削除する

#### Scenario: メインWorktreeの削除は拒否される
- **GIVEN** 対象Worktreeがメインである
- **WHEN** クライアントが `DELETE /api/v1/projects/{id}/worktrees/{branch}` を呼び出す
- **THEN** サーバはエラーを返す

#### Scenario: Worktreeブランチをマージする
- **GIVEN** 対象Worktreeにコンフリクトがなく、先行コミットがある
- **WHEN** クライアントが `POST /api/v1/projects/{id}/worktrees/{branch}/merge` を呼び出す
- **THEN** サーバはブランチをベースブランチにマージする

#### Scenario: コンフリクトがあるWorktreeのマージは拒否される
- **GIVEN** 対象Worktreeにマージコンフリクトがある
- **WHEN** クライアントが `POST /api/v1/projects/{id}/worktrees/{branch}/merge` を呼び出す
- **THEN** サーバはエラーを返す

### Requirement: WebSocketでWorktree状態を配信する

サーバはWebSocketの状態更新に、プロジェクトごとのWorktree情報を含めて配信しなければならない（MUST）。

#### Scenario: Worktree情報がWebSocketで配信される
- **GIVEN** クライアントがWebSocket `/api/v1/ws` に接続している
- **WHEN** サーバが定期的な状態更新を送信する
- **THEN** `full_state` メッセージにプロジェクトごとのWorktree一覧が含まれる
- **AND** 各Worktreeにはブランチ名、コンフリクト情報、マージ状態が含まれる
