## 1. Implementation
- [x] 1.1 `MergeWait`/`ResolveWait`のSpace操作で実行マークのみトグルできるようにガード/状態更新を調整する（`src/tui/state/guards.rs`）。
  - **Verify**: `src/tui/state/guards.rs`の`handle_toggle_running_mode`と`handle_toggle_stopped_mode`で`selected`のみ切り替え、`queue_status`とDynamicQueue操作が発生しないことを確認する。
- [x] 1.2 `ResolveWait`に対するSpace操作の完全ブロックを解除し、キュー状態の不変性を維持する。
  - **Verify**: `src/tui/state/guards.rs`の`validate_change_toggleable`で`ResolveWait`のSpaceを許可しつつ、`QueueStatus`が変化しないことを確認する。
- [x] 1.3 変更挙動に対応するユニットテストを追加/更新する。
  - **Verify**: `cargo test tui::state` または該当テストを実行し、`MergeWait`/`ResolveWait`のトグルが`selected`のみ変化することを確認する。

- [x] 1.4 `MergeWait`/`ResolveWait`の@操作で承認状態のみトグルできるようにする（キュー状態とDynamicQueueは不変）。
  - **Verify**: `src/tui/state/mod.rs`で`ResolveWait`をブロックしないこと、`src/tui/command_handlers.rs`でwait状態の承認/承認解除がキューへ副作用を持たないことを確認する。
- [x] 1.5 変更挙動に対応するユニットテストを追加/更新する（wait状態の@操作）。
  - **Verify**: `cargo test tui::state` で `ResolveWait`/`MergeWait`の@操作が `UnapproveOnly` を返すことを確認する。
