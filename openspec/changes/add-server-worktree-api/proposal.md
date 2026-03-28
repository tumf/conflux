# Change: Server Mode にプロジェクトスコープのWorktree管理APIを追加

**Change Type**: implementation

## Why

Server Mode Dashboard にはWorktree操作機能がなく、TUIやWeb Monitoringモードと機能格差がある。サーバーモードでもプロジェクトごとのWorktree一覧表示・作成・削除・ブランチマージを可能にし、ダッシュボードUIの基盤を整備する。

## What Changes

- `/api/v1/projects/{id}/worktrees` エンドポイント群を `src/server/api.rs` に追加
- WebSocket の `full_state` メッセージに `worktrees` フィールドを追加（プロジェクトごとのWorktree情報）
- `src/tui/worktrees.rs` の共通ロジック（一覧取得・コンフリクト検出）を `src/worktree_ops.rs` に抽出し、server/web/tui 共通で利用可能にする
- `RemoteWorktreeInfo` 型を `src/server/types.rs`（または `src/remote/types.rs`）に定義

## Impact

- Affected specs: `server-mode`
- Affected code: `src/server/api.rs`, `src/server/runner.rs`, `src/tui/worktrees.rs`, 新規 `src/worktree_ops.rs`

## Out of Scope

- ダッシュボードフロントエンドUI（別提案 `add-dashboard-worktree-ui` で対応）
- Worktree内での任意コマンド実行API（セキュリティ考慮が必要で、v1では除外）
- エディタ起動（サーバーサイド操作のため対象外）
