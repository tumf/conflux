## 1. 実装

- [x] 1.1 `src/tui/state/event_handlers/mod.rs` の `handle_orchestrator_event` match 文に `OrchestratorEvent::ChangeDequeued { change_id } => self.handle_change_stopped(change_id),` を追加する (verification: unit - `src/tui/state/event_handlers/completion.rs` の `handle_change_stopped` で `selected = false` が設定されることを既存テスト `tui::state::event_handlers::completion::tests` で確認; `cargo test -p cflx`)

## Final Validation

```bash
cflx openspec validate fix-force-kill-selected-state --strict
cargo test -p cflx
```
