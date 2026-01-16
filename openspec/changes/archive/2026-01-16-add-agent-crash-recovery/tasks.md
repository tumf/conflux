# Tasks: AI エージェントクラッシュリカバリー

## 実装タスク

- [x] 1. Apply コマンドのクラッシュリカバリーを実装
  - ファイル: `src/parallel/executor.rs`
  - 関数: `execute_apply_in_workspace()`
  - `!status.success()` 時に即座にエラー返却せず、リトライを継続
  - 2 秒の待機時間を追加
  - 警告ログを出力
  - TUI への通知（ApplyOutput イベント）

- [x] 2. Archive コマンドのクラッシュリカバリーを実装
  - ファイル: `src/parallel/executor.rs`
  - 関数: `execute_archive_in_workspace()`
  - `!status.success()` 時に即座にエラー返却せず、リトライを継続
  - 2 秒の待機時間を追加
  - 警告ログを出力
  - TUI への通知（ArchiveOutput イベント）

- [x] 3. Resolve コマンドのクラッシュリカバリーを確認
  - ファイル: `src/parallel/conflict.rs`
  - **既に実装済み**: `resolve_conflicts_with_retry()` と `resolve_merges_with_retry()` は
    コマンド失敗時もリトライを継続する設計になっている

- [x] 4. リンターとフォーマッターを実行
  - `cargo fmt` - 成功
  - `cargo clippy -- -D warnings` - 成功

- [x] 5. 全テストを実行して回帰がないことを確認
  - `cargo test` - 全 136 テスト成功
