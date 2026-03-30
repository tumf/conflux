## Implementation Tasks

- [ ] 1. `src/events.rs` に `EventSink` トレイトを定義する。`async fn on_event(&self, event: &ExecutionEvent)` と `async fn on_state_changed(&self, state: &OrchestratorState)` の2メソッドを持つ（verification: `rg "trait EventSink" src/events.rs` で定義が見つかる）
- [ ] 2. `src/tui/` 配下に `TuiEventSink` struct を実装する。`on_event` 内で `mpsc::Sender<OrchestratorEvent>` にイベントを送信する（verification: `cargo build` 成功）
- [ ] 3. `src/web/` 配下に `WebEventSink` struct を実装する。`on_event` 内で `WebState::apply_execution_event()` を呼ぶ（cfg-gated）（verification: `cargo build` 成功）
- [ ] 4. `dispatch_event()` ヘルパーを `EventSink` ベースに書き換える。`Vec<Arc<dyn EventSink>>` を受け取り、Reducer 更新後に各 sink に通知する（verification: `rg "dyn EventSink" src/tui/orchestrator.rs` で使用箇所が見つかる）
- [ ] 5. `src/orchestrator.rs` の dispatch パターンも同様に `EventSink` 経由に書き換える（verification: `rg "dyn EventSink" src/orchestrator.rs` で使用箇所が見つかる）
- [ ] 6. テスト用の `MockEventSink`（`Vec<ExecutionEvent>` を収集するだけ）を追加し、少なくとも1つの統合テストで使用する（verification: `rg "MockEventSink" src/` で使用箇所が見つかる、`cargo test` 通過）
- [ ] 7. `cargo test` 全パスと `cargo clippy -- -D warnings` クリアを確認する（verification: exit code 0, warning 0）
