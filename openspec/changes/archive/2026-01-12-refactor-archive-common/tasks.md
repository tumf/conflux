# Tasks: アーカイブロジックの共通化

## 1. 共通アーカイブモジュールの作成

- [x] 1.1 `src/execution/archive.rs` を作成
- [x] 1.2 `src/execution/mod.rs` に `pub mod archive;` を追加

## 2. パス検証ロジックの抽出

- [x] 2.1 `verify_archive_completion()` 関数を実装
  - change_path と archive_path を受け取る
  - change が archive/ に移動したかを検証
  - 日付プレフィックス付きのアーカイブパスもサポート
- [x] 2.2 TUI Serial 版のパス検証ロジックを新関数で置き換え
- [x] 2.3 Parallel 版のパス検証ロジックを新関数で置き換え

## 3. コマンド実行ロジックの抽出

Note: Command execution logic was analyzed but not fully extracted because:
- TUI Serial uses AgentRunner with streaming via tokio channels
- Parallel uses direct Command with different streaming patterns
- The core duplication was in verification logic, not execution
- Full extraction would require unifying streaming interfaces (future work)

## 4. タスク完了検証の共通化

- [x] 4.1 `verify_task_completion()` 関数を実装
  - tasks.md を解析して完了率をチェック
  - 既存の `task_parser` を活用
  - Added `get_task_progress()` for full progress info
- [x] 4.2 TUI Serial 版を新関数で置き換え (uses existing task checking via Change.is_complete())
- [x] 4.3 Parallel 版を新関数で置き換え (check_task_progress delegates to get_task_progress)

## 5. テストの作成

- [x] 5.1 `verify_archive_completion()` のユニットテスト
- [x] 5.2 `verify_task_completion()` のユニットテスト
- [x] 5.3 既存の E2E テストが引き続き動作することを確認

## 6. 検証

- [x] 6.1 `cargo build` が成功すること
- [x] 6.2 `cargo test` が成功すること
- [x] 6.3 `cargo clippy` が警告なしで通ること
