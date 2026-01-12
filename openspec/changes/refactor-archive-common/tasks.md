# Tasks: アーカイブロジックの共通化

## 1. 共通アーカイブモジュールの作成

- [ ] 1.1 `src/execution/archive.rs` を作成
- [ ] 1.2 `src/execution/mod.rs` に `pub mod archive;` を追加

## 2. パス検証ロジックの抽出

- [ ] 2.1 `verify_archive_completion()` 関数を実装
  - change_path と archive_path を受け取る
  - change が archive/ に移動したかを検証
  - 日付プレフィックス付きのアーカイブパスもサポート
- [ ] 2.2 TUI Serial 版のパス検証ロジックを新関数で置き換え
- [ ] 2.3 Parallel 版のパス検証ロジックを新関数で置き換え

## 3. コマンド実行ロジックの抽出

- [ ] 3.1 `execute_archive_command()` 関数を実装
  - workspace_path（Option）を受け取り、None ならカレントディレクトリで実行
  - ストリーミング出力をチャネル経由で返す
- [ ] 3.2 TUI Serial 版を新関数で置き換え
- [ ] 3.3 Parallel 版を新関数で置き換え

## 4. タスク完了検証の共通化

- [ ] 4.1 `verify_task_completion()` 関数を実装
  - tasks.md を解析して完了率をチェック
  - 既存の `task_parser` を活用
- [ ] 4.2 TUI Serial 版を新関数で置き換え
- [ ] 4.3 Parallel 版を新関数で置き換え

## 5. テストの作成

- [ ] 5.1 `verify_archive_completion()` のユニットテスト
- [ ] 5.2 `verify_task_completion()` のユニットテスト
- [ ] 5.3 既存の E2E テストが引き続き動作することを確認

## 6. 検証

- [ ] 6.1 `cargo build` が成功すること
- [ ] 6.2 `cargo test` が成功すること
- [ ] 6.3 `cargo clippy` が警告なしで通ること
- [ ] 6.4 TUI モードで archive が正しく動作すること（手動テスト）
- [ ] 6.5 Parallel モードで archive が正しく動作すること（手動テスト）
