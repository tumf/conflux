## 1. Implementation

- [ ] 1.1 `start_processing()` の selected 変更フィルタをホワイトリスト方式に変更: `NotQueued` の変更のみ `Queued` に遷移させる。`Blocked`, `Merged`, `Error`, `Applying`, `Accepting`, `Archiving` 等のアクティブ/終端状態は除外する (`src/tui/state.rs` の行 983-988 と 1022-1031) (verification: `cargo build` が成功すること)
- [ ] 1.2 `handle_stopped()` の reset 対象に `QueueStatus::Blocked` を追加する (`src/tui/state.rs` の行 1540-1556) (verification: `cargo build` が成功すること)
- [ ] 1.3 `handle_all_completed()` の reset 対象に `QueueStatus::Blocked` を追加する (`src/tui/state.rs` の行 1515-1518) (verification: `cargo build` が成功すること)
- [ ] 1.4 回帰テスト: `test_start_processing_does_not_queue_blocked_changes` を追加。Blocked 状態 + selected=true の変更が `start_processing()` で Queued に遷移しないことを検証 (verification: `cargo test test_start_processing_does_not_queue_blocked_changes` が PASS)
- [ ] 1.5 回帰テスト: `test_handle_stopped_resets_blocked_to_not_queued` を追加。停止時に Blocked が NotQueued にリセットされることを検証 (verification: `cargo test test_handle_stopped_resets_blocked_to_not_queued` が PASS)
- [ ] 1.6 回帰テスト: `test_handle_all_completed_resets_blocked_to_not_queued` を追加。完了時に Blocked が NotQueued にリセットされることを検証 (verification: `cargo test test_handle_all_completed_resets_blocked_to_not_queued` が PASS)
- [ ] 1.7 既存テストが壊れていないことを確認する (verification: `cargo test` が全て PASS)
- [ ] 1.8 `cargo clippy -- -D warnings` と `cargo fmt --check` が PASS することを確認する
