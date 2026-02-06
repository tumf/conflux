# Change: Web Worktree機能のTUIパリティ追加

## Why
Web監視機能にはTUIのWorktrees Viewと同等の取得・操作機能がなく、ブラウザ経由の運用でワークツリーの状態確認と保守作業が完結しません。

## What Changes
- `GET /api/worktrees` を追加し、TUIと同じ粒度のワークツリー一覧情報を返す
- Worktree操作エンドポイント（create/delete/merge）を追加し、TUIと同等の制約で実行する
- WebSocket `state_update` に `worktrees` の同期スナップショットを含め、状態差分をなくす
- WebダッシュボードにWorktreesパネルを追加し、一覧表示と操作を提供する
- fail-fast + structured logging方針を導入し、失敗を隠蔽せず診断可能なログを必須化する

## Impact
- Affected specs: `web-monitoring`, `tui-worktree-view`, `vcs-worktree-operations`
- Affected code: `src/web/server.rs`, `src/web/websocket.rs`, `src/web/state.rs`, `src/vcs/git/`, `web/`
