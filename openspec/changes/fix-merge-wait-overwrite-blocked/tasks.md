## 1. Implementation

- [x] 1.1 `src/tui/state.rs` の `apply_merge_wait_status()` の除外条件に `QueueStatus::Blocked` を追加する (verification: `cargo build` が成功すること)
- [x] 1.2 回帰テスト `test_apply_merge_wait_status_does_not_demote_blocked` を追加する。既存テスト `test_apply_merge_wait_status_does_not_demote_merged` と同パターン。Blocked 状態の変更が `apply_merge_wait_status` で MergeWait に降格されないことを検証 (verification: `cargo test test_apply_merge_wait_status_does_not_demote_blocked` が PASS)
- [x] 1.3 既存テストが壊れていないことを確認する (verification: `cargo test` が全て PASS)
- [x] 1.4 `cargo clippy -- -D warnings` と `cargo fmt --check` が PASS することを確認する
