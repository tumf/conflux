## Implementation Tasks

- [ ] 1. Worktree操作ロジックを `src/worktree_ops.rs` に抽出する。`src/tui/worktrees.rs` の `get_worktrees()`, `check_merge_conflicts()`, `count_commits_ahead()` を共通モジュールに移動し、`src/web/api.rs` と `src/tui/worktrees.rs` の両方がこの共通モジュールを利用するようリファクタリングする (verification: `cargo test` が全て通り、TUI/Web Monitoringの既存動作に影響がない)
- [ ] 2. `RemoteWorktreeInfo` 型を定義する。`path`, `label`, `head`, `branch`, `is_detached`, `is_main`, `is_merging`, `has_commits_ahead`, `merge_conflict` フィールドを含み、Serialize/Deserialize を derive する (verification: `cargo build` が成功する)
- [ ] 3. `GET /api/v1/projects/{id}/worktrees` エンドポイントを追加し、プロジェクトのworktree一覧をコンフリクト情報付きで返す (verification: `cargo test` + 手動で `curl` でAPI応答を確認可能)
- [ ] 4. `POST /api/v1/projects/{id}/worktrees` エンドポイントを追加する。リクエストボディは `{ change_id, base_commit? }`、worktree作成 + セットアップスクリプト実行 (verification: `cargo test` + API経由でworktreeが作成される)
- [ ] 5. `DELETE /api/v1/projects/{id}/worktrees/{branch}` エンドポイントを追加する。`can_delete_worktree` バリデーション付き (verification: `cargo test` + API経由でworktreeが削除される)
- [ ] 6. `POST /api/v1/projects/{id}/worktrees/{branch}/merge` エンドポイントを追加する。`can_merge_worktree` バリデーション付き (verification: `cargo test` + API経由でマージが実行される)
- [ ] 7. `POST /api/v1/projects/{id}/worktrees/refresh` エンドポイントを追加し、コンフリクト検出を再実行する (verification: `cargo test`)
- [ ] 8. WebSocket `full_state` メッセージに `worktrees` フィールド（`HashMap<String, Vec<RemoteWorktreeInfo>>`）を追加し、2秒ごとの状態更新で各プロジェクトのworktree情報を含める (verification: WebSocket接続でworktree情報が配信されることを確認)
- [ ] 9. 新規APIエンドポイントのルーティングテストと `worktree_ops` モジュールのユニットテストを追加する (verification: `cargo test`)
- [ ] 10. `cargo fmt --check && cargo clippy -- -D warnings` を通す (verification: コマンドが成功する)

## Future Work

- Worktree内でのコマンド実行APIの追加（セキュリティ設計要）
