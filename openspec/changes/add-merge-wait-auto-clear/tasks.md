## 1. 仕様と状態遷移の更新
- [x] 1.1 `QueueStatus::MergeWait` の自動解除条件と遷移先を spec に追記する（解除後は Queued と明記する）
  - 検証: `openspec/changes/add-merge-wait-auto-clear/specs/tui-architecture/spec.md` に MODIFIED 要件と Scenario が追加されている

## 2. TUI 自動更新の判定実装
- [x] 2.1 5秒ポーリング時に worktree の有無と ahead 状態を評価し、MergeWait を Queued に戻す処理を追加する
  - 検証: `src/tui/state/events.rs` の自動更新処理に MergeWait 解除ロジックが追加されている

## 3. 表示と操作ヒントの整合
- [x] 3.1 MergeWait が解除された場合に `M` ヒントが出ないことを保証する
  - 検証: `src/tui/render.rs` で MergeWait のみが `M` を表示していることを確認できる

## 4. テストと検証
- [x] 4.1 MergeWait の自動解除を検証するテストを追加する
  - 検証: 3つのテスト関数を追加済み (test_merge_wait_auto_cleared_when_worktree_missing, test_merge_wait_auto_cleared_when_not_ahead, test_merge_wait_preserved_when_ahead)
- [x] 4.2 既存の MergeWait 状態維持テストを更新する
  - 検証: `cargo test` が失敗しない
  - 完了: テストヘルパー関数 (create_test_change, create_approved_change) を追加し、TuiCommand インポートを修正。src/web/state.rs のテストケースに worktree_not_ahead_ids フィールドを追加。全テストが通ることを確認済み (871 tests passed)。
