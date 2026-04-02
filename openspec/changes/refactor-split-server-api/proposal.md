---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/server-api/spec.md
  - openspec/specs/code-maintenance/spec.md
  - src/server/api.rs
---

# Change: server/api.rs を責務別サブモジュールに分割する

**Change Type**: implementation

## Problem / Context

`src/server/api.rs` は現在 8,400 行超の単一ファイルであり、REST ハンドラ、WebSocket 処理、Git sync ロジック、ファイルツリー操作、ダッシュボード静的アセット配信、ターミナルセッション管理、プロポーザルセッション管理、テストヘルパーなど複数の責務が同居している。

この規模は開発速度とレビュー効率を著しく低下させ、並列作業でのコンフリクトリスクも高い。

## Proposed Solution

`src/server/api.rs` を以下のような責務別サブモジュールに分割する。

- `api/mod.rs` — AppState, ルーター構築 (`build_router`), 認証ミドルウェア
- `api/projects.rs` — プロジェクト CRUD (`add_project`, `delete_project`, `list_projects` 等)
- `api/git_sync.rs` — Git pull/push/sync とリモート同期モニター
- `api/control.rs` — グローバル Run/Stop/Status, change selection toggle
- `api/worktrees.rs` — worktree CRUD (list, create, delete, merge, refresh)
- `api/files.rs` — ファイルツリーとファイルコンテンツ取得
- `api/terminals.rs` — ターミナルセッション管理
- `api/proposals.rs` — プロポーザルセッション管理
- `api/ws.rs` — WebSocket ハンドラ
- `api/dashboard.rs` — ダッシュボード静的アセット配信
- `api/helpers.rs` — 共通ヘルパー (`error_response`, `now_rfc3339`, etc.)
- テストはそれぞれのサブモジュール内 `#[cfg(test)]` へ移動

## Acceptance Criteria

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test` がすべて成功する
- CLI (`cflx run`) と Server モードの既存動作が変わらない
- 公開 API (ルーター構造、エンドポイントパス) に変更がない
- `src/server/api.rs` が削除され、`src/server/api/` ディレクトリに置き換わる

## Out of Scope

- エンドポイントの追加・削除・パス変更
- ハンドラ内部ロジックの変更（シグネチャ変更を含む）
- パフォーマンス改善
