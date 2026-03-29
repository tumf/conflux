## Implementation Tasks

- [ ] `src/tui/state.rs`: `resolve_merge()` の `!self.is_resolving` 分岐で、`AppMode::Select | AppMode::Stopped` の場合に `self.mode = AppMode::Running` を追加する (verification: `cargo test resolve_merge`)
- [ ] `src/tui/state.rs`: テスト追加 — Select モードで MergeWait 変更に resolve_merge() を呼ぶと AppMode::Running に遷移すること (verification: `cargo test test_resolve_merge_select_transitions_to_running`)
- [ ] `src/tui/state.rs`: テスト追加 — Stopped モードで MergeWait 変更に resolve_merge() を呼ぶと AppMode::Running に遷移すること (verification: `cargo test test_resolve_merge_stopped_transitions_to_running`)
- [ ] `src/tui/state.rs`: テスト追加 — Running モードで resolve_merge() を呼んでも AppMode::Running のまま変わらないこと (verification: `cargo test test_resolve_merge_running_stays_running`)
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通ること (verification: コマンド実行)
