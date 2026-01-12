# タスク: 並列実行時の経過時間表示修正

## Phase 1: イベントハンドラの実装

### Task 1.1: `ApplyStarted` イベントハンドラを追加
- [x] `src/tui/state/events.rs` の `handle_orchestrator_event` に `ApplyStarted` ケースを追加
- [x] `started_at` が `None` の場合のみ `Some(Instant::now())` を設定
- [x] `queue_status` を `QueueStatus::Processing` に更新
- [x] `LogEntry::info` でログを記録
- **検証:** コンパイルが通ること
- **想定時間:** 10分

### Task 1.2: `ArchiveStarted` ハンドラに補完ロジックを追加
- [x] `src/tui/state/events.rs` の `ArchiveStarted` ケースに `is_none()` チェックを追加
- [x] `started_at` が未設定の場合のみ現在時刻を設定
- [x] 既存の `queue_status` 更新とログ記録は維持
- **検証:** コンパイルが通ること
- **想定時間:** 5分

## Phase 2: テストの実装

### Task 2.1: `ApplyStarted` の基本テストを追加
- [x] `test_apply_started_sets_started_at` を実装
  - `ApplyStarted` イベントを送信
  - `started_at` が `Some` になることを確認
  - `queue_status` が `Processing` になることを確認
  - ログエントリが追加されることを確認
- **検証:** `cargo test test_apply_started_sets_started_at` が成功
- **想定時間:** 15分

### Task 2.2: `ApplyStarted` の冪等性テストを追加
- [x] `test_apply_started_preserves_existing_started_at` を実装
  - 事前に `started_at` を設定
  - `ApplyStarted` イベントを送信
  - `started_at` が変更されないことを確認
- **検証:** `cargo test test_apply_started_preserves_existing_started_at` が成功
- **想定時間:** 10分

### Task 2.3: `ArchiveStarted` の補完テストを追加
- [x] `test_archive_started_sets_started_at_when_none` を実装
  - `started_at` が未設定の状態で `ArchiveStarted` を送信
  - `started_at` が設定されることを確認
- **検証:** `cargo test test_archive_started_sets_started_at_when_none` が成功
- **想定時間:** 10分

### Task 2.4: `ArchiveStarted` の保持テストを追加
- [x] `test_archive_started_preserves_started_at` を実装
  - 事前に `started_at` を設定
  - `ArchiveStarted` イベントを送信
  - `started_at` が変更されないことを確認
- **検証:** `cargo test test_archive_started_preserves_started_at` が成功
- **想定時間:** 10分

### Task 2.5: 並列実行フローの統合テストを追加
- [x] `test_parallel_execution_elapsed_time_flow` を実装
  - `ApplyStarted` → `ArchiveStarted` → `ChangeArchived` のイベント順序を再現
  - 各段階で `started_at` が正しく保持されることを確認
  - `ChangeArchived` で `elapsed_time` が記録されることを確認
- **検証:** `cargo test test_parallel_execution_elapsed_time_flow` が成功
- **想定時間:** 15分

### Task 2.6: 既存のテストが全てパスすることを確認
- [x] `cargo test` を実行
- [x] 全てのテストが成功することを確認
- [x] 失敗したテストがあれば修正
- **検証:** `cargo test` が 0 failures で完了
- **想定時間:** 5分

## Phase 3: 統合検証とドキュメント

### Task 3.4: コード品質チェック
- [x] `cargo fmt` でコードフォーマット
- [x] `cargo clippy -- -D warnings` でリントチェック
- [x] 警告やエラーがないことを確認
- **検証:** Clippy が警告なしで完了
- **想定時間:** 5分

### Task 3.5: 変更内容のレビュー
- [x] 変更されたファイルのレビュー
- [x] コードコメントの追加/更新
- [x] 不要なデバッグコードの削除
- **検証:** コードレビュー完了
- **想定時間:** 10分

## 完了条件

### 必須条件
- [x] 全てのタスクが完了している
- [x] `cargo test` が全て成功している
- [x] `cargo clippy` が警告なしで完了している
- [x] 並列実行で経過時間が正しく表示される
- [x] シリアル実行の動作が変更されていない

### 検証項目
- [x] `ApplyStarted` イベントで `started_at` が設定される
- [x] `ArchiveStarted` イベントで `started_at` が保持される
- [x] 並列実行で apply 開始から archive 完了までの経過時間が表示される
- [x] 既存のテストが全てパスする
- [x] 新しいテストが追加されている

## 見積もり

- **Phase 1:** 15分
- **Phase 2:** 65分
- **Phase 3:** 55分
- **合計:** 約 2時間15分

## 依存関係

- なし（単独で実装可能）

## 並列化可能なタスク

以下のタスクは並列実行可能：
- Task 2.1, 2.2, 2.3, 2.4, 2.5（テストは独立）
