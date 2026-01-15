# Tasks

## 1. `QueueStatus::Merged` variant を追加

**ファイル**: `src/tui/types.rs`

`QueueStatus` enum に `Merged` variant を追加する。

**実装内容**:
- Line 52 付近: `Archived` の後に `Merged` を追加
- Line 70 付近: `display()` メソッドに `QueueStatus::Merged => "merged"` を追加
- Line 85 付近: `color()` メソッドに `QueueStatus::Merged => Color::LightBlue` を追加

**検証**:
- `cargo build` が成功すること
- `cargo clippy` で warning が出ないこと

## 2. `MergeCompleted` イベント処理を Merged 状態に変更

**ファイル**: `src/tui/state/events.rs`

`MergeCompleted` イベント処理で状態を `Archived` から `Merged` に変更する。

**実装内容**:
- Line 156 付近: `change.queue_status = QueueStatus::Archived;` を `change.queue_status = QueueStatus::Merged;` に変更

**検証**:
- `cargo build` が成功すること
- イベント処理が正しく動作すること

## 3. Progress 更新の保護ロジックに Merged を追加

**ファイル**: `src/tui/state/events.rs`

Progress 更新をスキップする条件に `Merged` を追加する。

**実装内容**:
- Line 50 付近: terminal state チェックに `| QueueStatus::Merged` を追加
- Line 310 付近: Refresh での Progress update 保護に `| QueueStatus::Merged` を追加
- Line 352 付近: Terminal state の判定に `| QueueStatus::Merged` を追加

**検証**:
- `cargo build` が成功すること
- Progress 更新が terminal state で停止すること

## 4. UI レンダリングに Merged 対応を追加

**ファイル**: `src/tui/render.rs`

UI 表示ロジックに `Merged` 状態の処理を追加する。

**実装内容**:
- Line 28 付近: `get_checkbox_display()` 関数で `Merged` を `Archived` と同様に扱う
- Line 220, 386 付近: `is_archived` チェックを `matches!(change.queue_status, QueueStatus::Archived | QueueStatus::Merged)` に拡張
- Line 440 付近: Terminal state でのステータス表示に `| QueueStatus::Merged` を追加
- Line 555 付近: "Done" 判定に `| QueueStatus::Merged` を追加

**検証**:
- `cargo build` が成功すること
- TUI でステータスが正しく表示されること

## 5. テストケースを更新

**ファイル**: `src/tui/types.rs`, `src/tui/state/events.rs`

新しい `Merged` 状態に対応するテストケースを追加・更新する。

**実装内容**:
- `src/tui/types.rs`: `test_queue_status_merged_display`, `test_queue_status_merged_color` テストを追加
- `src/tui/state/events.rs` Line 549: テスト名を `test_merge_completed_sets_merged_status` に変更
- `src/tui/state/events.rs` Line 560: アサーションを `QueueStatus::Merged` に変更
- Terminal state テスト（Line 1000-1026 付近）に `Merged` を追加

**検証**:
- `cargo test` がすべて成功すること
- 新しいテストが正しく動作すること

## 6. 全テスト実行と動作確認

すべてのテストを実行し、変更によって破壊された機能がないことを確認する。

**検証**:
- `cargo test` がすべて成功すること
- `cargo clippy -- -D warnings` が成功すること
- TUI モードで並列実行時に `Merged` 状態が正しく表示されること
- Serial モードで `Archived` が最終状態として機能すること
