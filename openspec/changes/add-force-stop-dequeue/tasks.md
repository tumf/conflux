## Implementation Tasks

- [x] 1. reducer に実行中 change を terminal 化せず `not queued` へ戻す command/event を追加する（verification: `src/orchestration/state.rs` の reducer tests で `display_status()` が `not queued` になり、terminal が `None` のままであることを確認）
- [x] 2. serial / parallel / TUI orchestrator の停止完了経路を新しい dequeue event に揃える（verification: `src/tui/orchestrator.rs`, `src/parallel/dispatch.rs`, `src/serial_run_service.rs` に対応イベント送信があり、既存 stop フローの unit/integration test が更新される）
- [x] 3. TUI の active change 操作を「強制停止してキュー解除」の意味論に更新する（verification: `src/tui/state.rs` と `src/tui/command_handlers.rs` のテストで active change の停止後表示が `not queued` になる）
- [x] 4. server API に change 単位の stop-and-dequeue endpoint を追加し、WebSocket/REST 状態更新を連動させる（verification: `src/server/api.rs` の API テストで endpoint 呼び出し後の change status が `not queued` / unselected-or-equivalent queue-off semantics と整合することを確認）
- [x] 5. dashboard UI に実行中 change の stop-and-dequeue 操作を追加し、停止完了後の表示を `not queued` として反映する（verification: frontend tests または dashboard state tests で操作後表示が更新される）
- [x] 6. stale event / refresh / resume に対する非回帰テストを追加する（verification: reducer / TUI / server tests で stop-and-dequeue 後に stale apply or refresh で active/stopped へ戻らないことを確認）
- [x] 7. proposal 実装時に `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` を実行して受け入れ条件を検証する

## Future Work

- 必要であれば worktree クリーンアップや WIP rollback を別 proposal で扱う
