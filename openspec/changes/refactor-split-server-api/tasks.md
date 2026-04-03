## Implementation Tasks

- [x] 1. 特性化テスト: 分割前に `cargo test --lib server::api` を実行し、全テストが通ることを記録する（verification: テスト結果をログとして保持）
- [x] 2. `src/server/api.rs` を `src/server/api/mod.rs` にリネームし、ビルドが通ることを確認する（verification: `cargo build` 成功）
- [x] 3. 共通ヘルパー (`error_response`, `now_rfc3339`, 型定義等) を `api/helpers.rs` に抽出する（verification: `cargo build` 成功、テスト全通過）
- [x] 4. プロジェクト CRUD ハンドラを `api/projects.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 5. Git sync 関連を `api/git_sync.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 6. グローバル制御 + change selection を `api/control.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 7. Worktree 操作を `api/worktrees.rs` を抽出する（verification: `cargo test` 全通過）
- [x] 8. ファイル操作を `api/files.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 9. ターミナルセッション管理を `api/terminals.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 10. プロポーザルセッション管理を `api/proposals.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 11. WebSocket ハンドラを `api/ws.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 12. ダッシュボード静的アセット配信を `api/dashboard.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 13. テストを各サブモジュール内 `#[cfg(test)]` に配置し直す（verification: `cargo test` 全通過）
- [x] 14. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- 各ハンドラのエラー型を統一する（別 proposal で扱う）
