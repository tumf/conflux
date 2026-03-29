## Implementation Tasks

- [x] 1. `Cargo.toml` に `rusqlite = { version = "0.34", features = ["bundled"] }` を `web-monitoring` feature 依存で追加 (verification: `cargo check --features web-monitoring`)
- [x] 2. `src/server/db.rs` を作成: `ServerDb` 構造体、DB 初期化、WAL モード設定、PRAGMA user_version ベースのスキーママイグレーション (verification: `cargo test --lib server::db`)
- [x] 3. `orchestration_runs` テーブルの CRUD メソッド実装: `insert_run`, `update_run_status`, `get_recent_runs` (verification: unit test)
- [x] 4. `change_events` テーブルの CRUD メソッド実装: `insert_change_event`, `get_events_by_project_change`, `get_recent_events`, `get_stats_overview` (verification: unit test)
- [x] 5. `log_entries` テーブルの CRUD メソッド実装: `insert_log`, `query_logs` (limit/before/project_id フィルタ) (verification: unit test)
- [x] 6. `change_states` テーブルの CRUD メソッド実装: `upsert_change_state`, `load_change_states`, `delete_change_states_for_project` (verification: unit test)
- [x] 7. ログローテーション: `cleanup_old_logs(days: u32)` メソッドと、サーバー起動時 + 24h タイマーでの呼び出し (verification: unit test)
- [x] 8. `src/server/mod.rs`: サーバー起動時に `ServerDb::new(data_dir)` を初期化し `AppState` に `db: Arc<ServerDb>` を追加 (verification: `cargo build`)
- [x] 9. `src/server/registry.rs`: `change_selections` / `error_changes` の初期化時に `change_states` テーブルからロード、変更時に `upsert_change_state` で永続化 (verification: integration test - サーバー再起動後に状態復元)
- [x] 10. サーバーの各 runner/orchestration コード: apply/archive/acceptance/resolve 完了時に `insert_change_event` を呼び出す (verification: `cargo test`)
- [x] 11. サーバーのログ broadcast 時に `insert_log` を呼び出す (verification: `cargo test`)
- [x] 12. `src/server/api.rs`: `GET /api/v1/stats/overview` エンドポイント追加 — 全プロジェクトの成功/失敗数、平均処理時間 (verification: `cargo test` + curl)
- [x] 13. `src/server/api.rs`: `GET /api/v1/stats/projects/:id/history` エンドポイント追加 — プロジェクトの処理イベント履歴 (verification: `cargo test` + curl)
- [x] 14. `src/server/api.rs`: `GET /api/v1/logs` エンドポイント追加 — 永続化ログの検索 (limit, before, project_id パラメータ) (verification: `cargo test` + curl)
- [x] 15. `openapi.yaml` の更新: 新エンドポイントのスキーマ追加 (verification: `cargo run --bin openapi_gen` で差分確認)
- [x] 16. `cargo fmt && cargo clippy -- -D warnings && cargo test` の全パス確認

## Future Work

- サーバー起動時に SQLite から最新 N 件をインメモリ履歴にプリロード（初期は空起動で既存動作と同じ）
- `projects.json` の SQLite 移行検討
- ダッシュボードフロントエンドとの統合（`add-dashboard-overview` proposal）
