## Implementation Tasks

- [x] 1. `src/tui/orchestrator.rs` 内の `shared_state.write().await.apply_execution_event` と `ws.apply_execution_event` の呼び出し箇所を全て列挙し、共通パターンを確認する（verification: `rg "apply_execution_event" src/tui/orchestrator.rs` で全行を把握）
- [x] 2. `src/tui/orchestrator.rs` に `dispatch_event` async ヘルパー関数を追加する。引数は `tx: &mpsc::Sender<OrchestratorEvent>`, `shared_state: &Arc<RwLock<OrchestratorState>>`, `web_state` (cfg-gated), `event: OrchestratorEvent`。内部で (a) `tx.send(event.clone()).await`, (b) `shared_state.write().await.apply_execution_event(&event)`, (c) cfg-gated で `ws.apply_execution_event(&event).await` を呼ぶ（verification: 関数が存在し、3つの送信先を呼び出している）
- [x] 3. `src/tui/orchestrator.rs` の `run_orchestrator` 関数内で、手動の3行パターン（tx.send + shared_state.write + ws.apply_execution_event）を全て `dispatch_event()` 呼び出しに置き換える。対象は L265, L274-282, L411-414, L467, L491, L512-515, L527-530, L544, L557, L573, L589, L606, L622, L636, L652, L666, L694, L711, L727, L744, L766, L788, L799, L815, L833, L872 付近（verification: `run_orchestrator` 内の該当箇所を `dispatch_event()` 経由に統一）
- [x] 4. `src/orchestrator.rs` 内の同パターン（L1202 付近）も同様にヘルパーに置き換える（verification: `is_new_change` 分岐の `ProcessingStarted` 更新を `update_shared_state()` 経由に置換）
- [x] 5. `cargo test` が全パスすることを確認する（verification: `cargo test` exit code 0）
- [x] 6. `cargo clippy -- -D warnings` がクリアであることを確認する（verification: warning 0）
