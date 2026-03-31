## Implementation Tasks

- [ ] 1. `handle_workspace_completion` から merge/resolve 処理を分離し、成功時の merge+cleanup をバックグラウンドタスクとして spawn する (`src/parallel/queue_state.rs`; verification: `handle_workspace_completion` が merge/resolve の完了を待たずに即座に return する)
- [ ] 2. バックグラウンド merge タスクの結果を受け取る仕組みを追加する（`join_set` への追加または専用チャネル）(`src/parallel/orchestration.rs`; verification: merge 結果がスケジューラループに正しく届く)
- [ ] 3. merge 完了時の `retry_deferred_merges` 呼び出しをスケジューラ側の merge 結果受信時に実行する (`src/parallel/merge.rs`, `src/parallel/queue_state.rs`; verification: deferred merge が merge 成功後にリトライされる)
- [ ] 4. `AutoResolveGuard`（RAII）がバックグラウンドタスク内で正しく drop されることを確認・必要に応じて修正する (`src/parallel/conflict.rs`; verification: resolve 完了後に `auto_resolve_count` がデクリメントされる)
- [ ] 5. スケジューラループの `select!` arm 内で merge 結果の受信を追加する (`src/parallel/orchestration.rs`; verification: merge 中もスケジューラループが回り続け queued change が dispatch される)
- [ ] 6. 既存テストの更新・追加: resolve 中に queued change が dispatch されるシナリオのテスト (`src/parallel/tests/`; verification: `cargo test` で全テスト通過)

## Future Work

- TUI の `is_resolving` フラグによる F5 ブロックの見直し（別 proposal）
