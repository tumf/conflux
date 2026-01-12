# Tasks: Apply ロジックの共通化と VCS 操作の統一

## 1. 共通 Apply モジュールの作成

- [ ] 1.1 `src/execution/apply.rs` を作成
- [ ] 1.2 `src/execution/mod.rs` に `pub mod apply;` を追加

## 2. 進捗チェックの共通化

- [ ] 2.1 `check_task_progress()` を `execution/apply.rs` に移動または再エクスポート
- [ ] 2.2 `ProgressInfo` 構造体との統合
- [ ] 2.3 serial/parallel 両モードからの参照を更新

## 3. Apply 反復ロジックの抽出

- [ ] 3.1 `ApplyConfig` 構造体を定義
  - max_iterations, progress_commit_enabled, streaming_enabled
- [ ] 3.2 `execute_apply_iteration()` 関数を実装
  - 1回の apply 実行と進捗チェック
- [ ] 3.3 Parallel 版の反復ループを新関数で置き換え

## 4. VCS 操作の統一

- [ ] 4.1 `parallel/executor.rs` の git commit 操作を `WorkspaceManager::set_commit_message()` で置き換え
- [ ] 4.2 `parallel/executor.rs` の jj describe 操作を `WorkspaceManager::set_commit_message()` で置き換え
- [ ] 4.3 `parallel/executor.rs` の revision 取得を `WorkspaceManager::get_revision_in_workspace()` で置き換え

## 5. プログレスコミットの共通化

- [ ] 5.1 `create_progress_commit()` を `execution/apply.rs` に移動
- [ ] 5.2 `WorkspaceManager` を使用するよう変更
- [ ] 5.3 参照を更新

## 6. テストの作成

- [ ] 6.1 `ApplyConfig` のユニットテスト
- [ ] 6.2 進捗チェックのユニットテスト
- [ ] 6.3 既存の E2E テストが引き続き動作することを確認

## 7. 検証

- [ ] 7.1 `cargo build` が成功すること
- [ ] 7.2 `cargo test` が成功すること
- [ ] 7.3 `cargo clippy` が警告なしで通ること
- [ ] 7.4 TUI serial モードで apply が正しく動作すること（手動テスト）
- [ ] 7.5 Parallel モードで apply が正しく動作すること（手動テスト）
