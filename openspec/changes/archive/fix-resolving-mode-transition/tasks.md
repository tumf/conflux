## Implementation Tasks

- [x] 1. `handle_all_completed()` に Resolving ガードを追加（`src/tui/state.rs:1578`）: Resolving 中の change が 1 つ以上ある場合 `AppMode::Running` を維持、Queued/Blocked リセットは従来通り、ログを「Waiting for resolve to complete」に変更（verification: `cargo test test_all_completed` で Resolving 中は Select に遷移しないことを確認）

- [x] 2. 完了チェックヘルパー `try_transition_to_select()` を抽出（`src/tui/state.rs`）: 全 change に active なものがなく mode が Running の場合に `AppMode::Select` へ遷移、elapsed 記録・ログ出力・mode 遷移を共通化（verification: `cargo test try_transition` でヘルパーの単体テスト）

- [x] 3. `handle_resolve_completed()` と `handle_resolve_failed()` から `try_transition_to_select()` を呼び出す（`src/tui/state.rs`）: Resolve 完了/失敗後に他の active change が残っていなければ Select に遷移（verification: `cargo test test_resolve_completed_transitions`）

- [x] 4. `handle_stopped()` のリセット対象に `QueueStatus::Resolving` を追加（`src/tui/state.rs:1607`）（verification: `cargo test test_stopped_resets_resolving`）

- [x] 5. 回帰テスト追加: Resolving中にAllCompleted→Running維持、Resolve完了後に他activeなし→Select遷移、Resolve完了後に他activeあり→Running維持、Stopped時にResolving changeがリセットされる（verification: `cargo test` 全テスト通過）

- [x] 6. `cargo fmt --check && cargo clippy -- -D warnings` で lint 通過確認
