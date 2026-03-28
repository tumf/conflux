## Context

Server Mode Dashboard にWorktree管理機能を追加するための、バックエンドAPI設計。Web Monitoring (`src/web/api.rs`) に既存のworktree API実装があり、そのロジックを共通化して再利用する。

## Goals / Non-Goals

- Goals: プロジェクトスコープのWorktree CRUD + マージAPIを提供、WebSocket経由でリアルタイム配信
- Non-Goals: 任意コマンド実行API、エディタ起動、フロントエンドUI

## Decisions

### 共通モジュール抽出

`src/worktree_ops.rs` を新設し、以下のロジックを `src/tui/worktrees.rs` から移動:
- `get_worktrees(repo_root) -> Vec<WorktreeInfo>`: worktree一覧取得 + 並列コンフリクト検出
- `check_merge_conflicts()`: ベースブランチとのマージコンフリクト検出
- `count_commits_ahead()`: 先行コミット数取得
- `can_delete_worktree()`, `can_merge_worktree()`: バリデーション（Web Monitoring側から移植）

### パス解決

Web Monitoring は `std::env::current_dir()` を使うが、Server Mode ではプロジェクトのworktreeパスを registry から取得:
```
registry.data_dir()/worktrees/{project_id}/{branch}
```
API ハンドラでは `ProjectRegistry` からプロジェクトの worktree パスを取得し、`worktree_ops` に渡す。

### 排他制御

既存の `registry.project_lock(&project_id)` を利用。worktree操作は同一プロジェクト内で直列化される。

### WebSocket 拡張

`FullState` 構造体に `worktrees: HashMap<String, Vec<RemoteWorktreeInfo>>` を追加。2秒ごとの定期配信で各プロジェクトのworktree状態を含める。パフォーマンスへの影響が大きい場合は、変更検出による差分配信に切り替え可能。

## Risks / Trade-offs

- 全プロジェクトのworktreeを2秒ごとにスキャンするのはI/O負荷になり得る → 初期は選択中プロジェクトのみスキャン、または差分検出で軽量化
- Web Monitoring APIとの重複 → 共通モジュール化で対処済み
