# Change: Web WorktreesのTUIパリティ統合

## Why
Web監視ではWorktreeの一覧確認と運用操作をTUIに依存しており、Web UI/APIだけで完結した保守運用ができません。重複した2つの提案を統合し、仕様と実装方針を単一の変更として明確化します。

## What Changes
- `GET /api/worktrees` を追加し、TUIと同等語彙の一覧スナップショットを返す
- 操作APIとして `refresh`, `create`, `delete`, `merge`, `command` エンドポイントを追加する
- fail-fast方針を適用し、`409`/`404`/`500` の拒否条件を明示する
- 構造化ログに `request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を必須化する
- `state_update.worktrees` と `/api/state` の整合を必須にし、REST/WSの状態差分をなくす
- WebダッシュボードにWorktrees Viewを追加し、操作ガードと削除確認を実装する

## Impact
- Affected specs: `web-monitoring`
- Affected code: `src/web/server.rs`, `src/web/state.rs`, `src/web/websocket.rs`, `src/tui/runner.rs`, `src/vcs/`, `web/`, `openapi`, `README`
