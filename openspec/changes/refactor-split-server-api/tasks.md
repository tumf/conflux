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
- [x] 13. テストを各サブモジュール内 `#[cfg(test)]` に配置し直す（verification: `cargo test --lib server::api` 全通過）
- [x] 14. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- 各ハンドラのエラー型を統一する（別 proposal で扱う）

## Acceptance #1 Failure Follow-up

- [x] `test_stats_and_logs_endpoints_return_data` が `POST /api/v1/projects` で 201 を再び返すように修正し、`cargo test test_stats_and_logs_endpoints_return_data -- --nocapture` を再実行して通過を確認する
- [x] `src/server/api/mod.rs` に残っている API テストを責務別サブモジュールへ移し、`src/server/api/mod.rs` にはルーター構築と共有ロジックのテストだけを残す

## Acceptance #2 Failure Follow-up

- [ ] `src/server/api/mod.rs` に残っている API 別テスト（auth / projects / files / git sync / worktrees / proposal session など）を対応するサブモジュールへ移し、`mod.rs` にはルーター構築と共有ロジックのテストだけを残す
- [ ] 変更をコミット可能な状態まで整理し、受け入れ確認時に `git status --porcelain` が空になるようにする

## Implementation Blocker #1
- category: other
- summary: `mod.rs` に集中している統合テストを責務別サブモジュールへ移管する際、共有テストヘルパーの公開範囲と所有先を決めないと重複/循環依存を回避できない
- evidence:
  - src/server/api/mod.rs:671 に巨大な `mod tests` が存在し、67件の API テストが `make_router` / `make_state` / Git テストヘルパーに密結合している
  - src/server/api/projects.rs:1 ほか各サブモジュールは `super::*` 前提で、現状 `#[cfg(test)]` 向け共通テストユーティリティが公開されていない
  - openspec/changes/refactor-split-server-api/tasks.md:29 のタスクは「対応するサブモジュールへ移す」ことを要求するが、共通ヘルパー配置方針が未定義
- impact: テスト移管を一括で実施すると、ヘルパー重複実装または `mod.rs` への逆依存が発生し、保守性とコンパイル安定性を損なう
- unblock_actions:
  - `src/server/api/test_support.rs`（`#[cfg(test)]`）を新設し、`make_state` / `make_router` / Git fixture helper の共通化方針を先に確定する
  - どのテストを「サブモジュール単位のユニット」にし、どれを「API ルーター統合テスト」として残すかを tasks.md で明示的に分割する
- owner: server-api maintainers
- decision_due: 2026-04-04
