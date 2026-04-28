## Implementation Tasks

- [ ] 1. selection / queue / retry / resume / log / worktree の現在挙動を固定する characterization test を先に追加または更新する (verification: unit - add or update Rust tests proving current TUI semantics for selection, queue operations, error retry, resume flow, log scrolling, and worktree cursor behavior before refactor)
- [ ] 2. `src/tui/state.rs` に残る選択・キュー・resume / retry 系の AppState 実装を責務別モジュールへ移し、shared reducer 同期と TuiCommand 生成を維持する (verification: unit - run selection/queue/retry/resume tests and confirm reducer-facing behavior is unchanged)
- [ ] 3. ログ管理と worktree 操作に関する AppState 実装を既存サブモジュールまたは新規責務モジュールへ整理し、`state.rs` を入口中心にする (verification: unit - run log/worktree tests and inspect module boundaries to confirm behavior stays the same)
- [ ] 4. display status / queue intent / reducer sync の回帰がないことを TUI 関連テストで確認する (verification: unit/integration - run relevant TUI state tests covering queued/not queued/error/retry and resolve-related state preservation)
- [ ] 5. proposal delta と関連コード変更を strict validation と Rust 検証で確認する (verification: integration - run `cflx openspec validate refactor-split-tui-state-appstate --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- `src/tui/render.rs` や `src/tui/orchestrator.rs` も同じ責務単位で段階的に整理する
- TUI 状態変更のテストサポート DSL を追加して、今後のリファクタリングをさらに安全にする
