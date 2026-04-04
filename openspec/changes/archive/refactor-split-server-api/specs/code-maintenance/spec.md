## ADDED Requirements

### Requirement: Server API モジュールの責務分割

サーバー API ハンドラは `src/server/api/` ディレクトリ配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。

ルーター構築 (`build_router`) と `AppState` は `mod.rs` に残し、各ハンドラはドメイン別サブモジュールからインポートしなければならない (MUST)。

#### Scenario: モジュール構成

- **WHEN** 開発者がサーバー API を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `api/mod.rs` — AppState, ルーター構築, 認証ミドルウェア
  - `api/projects.rs` — プロジェクト CRUD
  - `api/git_sync.rs` — Git pull/push/sync
  - `api/control.rs` — グローバル制御, change selection
  - `api/worktrees.rs` — Worktree 操作
  - `api/files.rs` — ファイルツリー/コンテンツ取得
  - `api/terminals.rs` — ターミナルセッション
  - `api/proposals.rs` — プロポーザルセッション
  - `api/ws.rs` — WebSocket ハンドラ
  - `api/dashboard.rs` — 静的アセット配信

#### Scenario: 各サブモジュールが独立してコンパイルできる

- **WHEN** 単一のサブモジュールのみを変更する
- **THEN** 他のサブモジュールへの影響は最小限であり、`cargo build` が成功する
