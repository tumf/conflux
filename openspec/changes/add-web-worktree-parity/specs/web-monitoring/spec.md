## ADDED Requirements

### Requirement: REST API - Worktrees List
システムは `GET /api/worktrees` を提供し、各worktreeについて `name`, `branch`, `path`, `clean`, `has_conflicts`, `can_merge` を含む一覧を返却しなければならない（SHALL）。

#### Scenario: 一覧取得が成功する
- **WHEN** クライアントが `GET /api/worktrees` を呼び出す
- **THEN** サーバーは `200` を返し、各要素に `name`, `branch`, `path`, `clean`, `has_conflicts`, `can_merge` を含む配列を返す

### Requirement: REST API - Worktree Operations
システムはworktreeの `create`, `delete`, `merge` 操作APIを提供しなければならない（MUST）。`delete` は未マージ状態を `409` で拒否し、`merge` はコンフリクト状態を `409` で拒否しなければならない（MUST）。

#### Scenario: 未マージworktreeの削除拒否
- **WHEN** クライアントが未マージのworktreeに対して削除APIを呼び出す
- **THEN** サーバーは `409` を返し、削除を実行しない

#### Scenario: コンフリクトworktreeのマージ拒否
- **WHEN** クライアントが `has_conflicts=true` のworktreeに対してマージAPIを呼び出す
- **THEN** サーバーは `409` を返し、マージを実行しない

### Requirement: WebSocket - Worktree Parity Updates
システムはWebSocketの `state_update.worktrees` に `/api/state` と同等意味のworktreeスナップショットを含め、REST取得結果と状態語彙を一致させなければならない（SHALL）。

#### Scenario: 状態更新でworktreesが同期される
- **WHEN** worktree操作後に `state_update` イベントが配信される
- **THEN** イベントの `worktrees` は同時点の `/api/state` と整合するスナップショットである

### Requirement: Worktree Operations Logging and Failure Policy
システムはWorktree操作失敗を隠蔽してはならない（MUST NOT）。各操作で `request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を構造化ログとして記録し、VCS失敗時は `500` を返さなければならない（MUST）。

#### Scenario: VCS失敗時に500と構造化ログを返す
- **WHEN** create/delete/merge のいずれかでVCS処理が失敗する
- **THEN** サーバーは `500` を返し、`request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を含むエラーログを出力する
