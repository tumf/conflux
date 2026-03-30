---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/orchestrator.rs
  - src/orchestrator.rs
---

# Change: イベント送信の3重パターンをヘルパー関数に集約する

**Change Type**: implementation

## Why
`src/tui/orchestrator.rs` では、ほぼ全てのイベント送信で以下の3行パターンが繰り返されている（57箇所）:

```rust
let _ = tx.send(event.clone()).await;                      // TUI channel
shared_state.write().await.apply_execution_event(&event);  // Reducer
ws.apply_execution_event(&event).await;                    // Web (cfg-gated)
```

各送信先が別の引数セット（`tx`, `shared_state`, `web_state`）を必要とし、`#[cfg(feature = "web-monitoring")]` ガードも毎回必要。これにより：
- 新イベント追加時に3行をコピペし忘れるリスク
- `tui/orchestrator.rs` が1392行に肥大化する一因
- Web送信の cfg ガードが散在して読みにくい

## Proposed Solution
`dispatch_event()` ヘルパー関数を導入し、3箇所への送信を1行呼び出しに集約する。ステートマシンのロジックは一切変更しない。

```rust
async fn dispatch_event(
    tx: &mpsc::Sender<OrchestratorEvent>,
    shared_state: &Arc<RwLock<OrchestratorState>>,
    #[cfg(feature = "web-monitoring")] web_state: &Option<Arc<WebState>>,
    event: OrchestratorEvent,
) { ... }
```

同様に `src/orchestrator.rs` の4箇所も同パターンがあれば統一する。

## Acceptance Criteria
- `tui/orchestrator.rs` 内の `shared_state.write().await.apply_execution_event` 呼び出しが `dispatch_event()` 経由に統一されている
- `ws.apply_execution_event` の直接呼び出しが（テスト除き）ゼロ
- `cargo test` 全パス
- `cargo clippy -- -D warnings` クリア
- 機能的な動作変更なし

## Out of Scope
- Reducer のステートマシン変更
- Web/TUI のステータス管理方式変更
- フロントエンド trait の導入
