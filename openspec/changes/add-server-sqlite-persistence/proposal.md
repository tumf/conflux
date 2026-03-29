# Change: サーバーモードに SQLite 永続化層を追加

**Change Type**: implementation

## Why

`cflx server` のランタイムデータ（apply/archive/acceptance/resolve 履歴、ログ、チェンジ選択状態）はすべてインメモリ `HashMap` で管理されており、プロセス再起動時に消失する。これにより：

- リトライ時のコンテキスト注入が失われ、エージェントが同じ失敗を繰り返す
- 過去のオーケストレーション実行の統計が取れない
- チェンジ選択・エラー状態が再起動でリセットされる
- ダッシュボードで履歴データを表示できない

## What Changes

- `src/server/db.rs` を新設し、SQLite (`rusqlite` bundled) による永続化層を追加
- `data_dir/cflx.db` にデータベースファイルを配置
- テーブル: `orchestration_runs`, `change_events`, `log_entries`, `change_states`
- `AppState` に `Arc<ServerDb>` を追加し、write-through キャッシュパターンで統合
- `ProjectRegistry` のチェンジ選択・エラー状態を `change_states` テーブルで永続化
- 新 API エンドポイント: `GET /api/v1/stats/overview`, `GET /api/v1/stats/projects/:id/history`, `GET /api/v1/logs`
- ログローテーション: 30日超の `log_entries` を定期削除
- Cargo.toml に `rusqlite` (bundled) を `web-monitoring` feature 依存で追加

## Impact

- Affected specs: server-persistence (新規)
- Affected code: `src/server/mod.rs`, `src/server/api.rs`, `src/server/registry.rs`, `Cargo.toml`
- **projects.json は維持**（レジストリ構造がシンプルなため SQLite 化のメリットが薄い）

## Acceptance Criteria

- サーバー再起動後に apply/archive/acceptance/resolve 履歴が保持される
- チェンジ選択・エラー状態が再起動後に復元される
- `GET /api/v1/stats/overview` が全プロジェクトの集計統計を返す
- `GET /api/v1/logs` が永続化ログを返す（limit, before パラメータ対応）
- 30日超のログエントリが自動削除される
- `cargo test` が通る

## Out of Scope

- `projects.json` の SQLite 移行
- run/tui モードへの SQLite 導入
- ダッシュボードフロントエンドの変更（別 proposal `add-dashboard-overview` で対応）
