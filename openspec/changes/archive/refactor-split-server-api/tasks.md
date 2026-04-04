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
- [x] 13. テストを各サブモジュール内 `#[cfg(test)]` に配置し直す（`mod.rs` には共有ロジックのテストのみを残し、`cargo test --lib server::api` 全通過を確認済み）
- [x] 14. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- 各ハンドラのエラー型を統一する（別 proposal で扱う）

## Acceptance Follow-up（Consolidated）

- [x] API 別テストを `src/server/api/mod.rs` から責務別サブモジュール（`projects.rs` / `git_sync.rs` / `worktrees.rs` / `files.rs` / `proposals.rs` / `control.rs` など）へ移管し、`mod.rs` には共有ロジックの `test_classify_sync_state_variants` のみを残す
- [x] `src/server/api/control.rs` の `clippy::items-after-test-module` を解消するため、`list_selected_change_ids_in_worktree` / `start_single_project_run` を test module より前へ再配置
- [x] `tests/no_backup_files_test.rs` の参照先を分割後構成へ追従させ、削除済み `src/server/api.rs` を参照しない状態へ修正
- [x] flaky 要因だった環境依存テストを安定化（`src/config/mod.rs` の env 変更系テスト直列化）
- [x] flaky 要因だった Git fixture 初期化の衝突リスクを低減（`src/server/api/test_support.rs` の一意名強化・初期化手順安定化）
- [x] archive 前品質ゲートを再実行し、`cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` / `prek run --all-files` の通過を確認
- [x] `tasks.md` の完了状態を実装実態に合わせて修正（Task 13、および関連 Acceptance 項目の整合を反映）
- [x] `REJECTED.md` 不在を確認し、rejection に進まず apply 継続で回復済みであることを確認

## Historical Blocker Note

category: other
summary: `mod.rs` 集中テストの移管にあたり、共通テストヘルパーの所有先未定義が重複/循環依存リスクだった
resolution:
  1. `src/server/api/test_support.rs` を共通基盤として導入
  2. サブモジュール単位テストと共有ロジックテストの責務を分離

## Rejecting Recovery Tasks

- [x] Investigate blocker in openspec/changes/refactor-split-server-api/REJECTED.md and implement a non-rejection recovery path before rerunning apply（`REJECTED.md` 不在を確認し、rejection に進まず apply 継続で回復）
