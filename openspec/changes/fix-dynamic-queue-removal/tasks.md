# 実装タスク

## 1. DynamicQueueの拡張
- [ ] 1.1 `src/tui/queue.rs` に `remove` メソッドを追加（指定されたIDをキューから削除）
- [ ] 1.2 `remove` メソッドのユニットテストを追加（正常削除、存在しないID、複数削除など）

## 2. TUIコマンド処理の修正
- [ ] 2.1 `src/tui/runner.rs` の `TuiCommand::UnapproveAndDequeue` で、dynamic_queue を参照できるように修正
- [ ] 2.2 unapprove処理時に `dynamic_queue.remove()` を呼び出す
- [ ] 2.3 削除ログメッセージを追加

## 3. Spaceキー処理の修正
- [ ] 3.1 `src/tui/state/mod.rs` の `toggle_selection` メソッドで、キューから削除する際に dynamic_queue も参照できるようにする
- [ ] 3.2 `QueueStatus::Queued` から `NotQueued` に変更する際、dynamic_queue からも削除
- [ ] 3.3 削除が成功したことをログに記録

## 4. テストと検証
- [ ] 4.1 DynamicQueue単体テストの実行確認（`cargo test queue`）
- [ ] 4.2 手動テスト: sequenceモード実行中に [x] を [@] に変更して、実行されないことを確認
- [ ] 4.3 手動テスト: Spaceキーでキューから削除して、実行されないことを確認
- [ ] 4.4 全テストの実行（`cargo test`）
