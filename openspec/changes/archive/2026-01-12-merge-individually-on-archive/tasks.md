# タスク: Archive 完了時に個別にマージする

## 実装タスク

- [x] `src/parallel/mod.rs` の `execute_apply_and_archive_parallel` メソッドに個別マージロジックを追加
  - archive 完了時の `final_revision` を取得
  - `merge_and_resolve(&[final_revision])` を呼び出し
  - マージ成功/失敗を適切に処理

- [x] `src/parallel/mod.rs` の `execute_group` メソッドからグループ単位のマージコードを削除
  - `let revisions: Vec<String> = successful.iter()...` の部分を削除
  - `self.merge_and_resolve(&revisions)` の呼び出しを削除

- [x] 個別マージのイベント追加（必要に応じて）
  - 既存の `MergeStarted` と `MergeCompleted` イベントを使用
  - 新しいイベント追加は不要（既存のイベント構造で対応可能）

- [x] エラーハンドリングの調整
  - マージ失敗時に適切なエラーメッセージを表示
  - マージ失敗時は即座にエラーを返すよう実装

## テストタスク

- [x] 既存の並列実行テストが通ることを確認
  - `cargo test parallel` - 全43テスト通過
  - E2Eテスト2件も通過
