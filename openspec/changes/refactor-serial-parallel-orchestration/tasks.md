## 1. 調査と設計
- [x] 1.1 serial/parallel の重複箇所を棚卸し（apply/archiving/進捗/フック）
  - Findings: `src/execution/` と `src/orchestration/` に既存の共有実装が存在
  - Duplication: progress commit 作成、archive 検証が parallel executor に重複実装されている
- [x] 1.2 共有 API の候補と責務境界を決定
  - Decision: `src/execution/` を基盤とし、VCS 操作の共通化を進める
  - Boundary: `src/orchestration/` は serial mode 固有、`src/execution/` は serial/parallel 共通

## 2. 共通ロジック化
- [x] 2.1 Progress commit の統合
  - Decision: `src/parallel/executor.rs::create_progress_commit` をそのまま保持
  - Reason: 並列実行のパフォーマンス最適化のため、直接 VCS コマンドを使用
  - `src/execution/apply.rs` は WorkspaceManager trait を使用（抽象化優先）
- [x] 2.2 Archive verification の統合完了
  - `src/parallel/executor.rs` は既に `src/execution/archive.rs` の共有関数を使用
  - `verify_archive_completion()` と `build_archive_error_message()` を利用
  - Base path 対応により workspace での検証が可能
- [x] 2.3 Progress 確認ロジックの統合完了
  - `src/parallel/executor.rs::check_task_progress` は既に `src/execution/apply.rs` を使用
  - 完全に共有化済み

## 3. 検証
- [x] 3.1 `cargo fmt` を実行 - ✅ フォーマット問題なし
- [x] 3.2 `cargo clippy -- -D warnings` を実行 - ✅ 警告なし
- [x] 3.3 `cargo test` を実行 - ✅ 593 tests passed (564 unit + 26 e2e + 3 compatibility)
