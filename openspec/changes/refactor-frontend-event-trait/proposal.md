---
change_type: implementation
priority: medium
dependencies:
  - refactor-event-dispatch-helper
  - refactor-tui-status-from-reducer
  - refactor-web-status-derivation
references:
  - src/tui/orchestrator.rs
  - src/orchestrator.rs
  - src/web/state.rs
  - src/tui/state.rs
  - src/orchestration/state.rs
---

# Change: フロントエンド EventSink トレイトを導入し Core とフロントエンドの責務を明確に分離する

**Change Type**: implementation

## Why
Proposal 1（dispatch helper）と Proposal 2（TUI status）/既存 Proposal（Web status）完了後も、Core（Reducer + オーケストレーションループ）とフロントエンド（TUI/Web）の境界は暗黙的なままである。

現状：
- `tui/orchestrator.rs`（1392行）と `orchestrator.rs`（1718行）が別々のオーケストレーションループを持ち、イベント送信・ステータス更新のコードが重複
- 新しいフロントエンド追加時（例: headless CI モード）に既存コードをフォークする必要がある
- テスト時にフロントエンドをモックできない

## Proposed Solution
`EventSink` トレイトを定義し、TUI/Web/テスト用のフロントエンドが統一インターフェースで Reducer からの通知を受け取る構造にする。

```rust
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn on_event(&self, event: &ExecutionEvent);
    async fn on_state_changed(&self, state: &OrchestratorState);
}
```

`dispatch_event()` ヘルパーは `Vec<Arc<dyn EventSink>>` に送信するだけになる。TUI channel 送信と Web state 更新はそれぞれ `EventSink` 実装として提供する。

## Acceptance Criteria
- `EventSink` トレイトが `src/events.rs` または `src/orchestration/` に定義されている
- TUI 用 `TuiEventSink` と Web 用 `WebEventSink` が実装されている
- `dispatch_event()` が `EventSink` 経由で送信している
- テスト用の `MockEventSink` が存在し、少なくとも1つのテストで使用されている
- `cargo test` 全パス
- `cargo clippy -- -D warnings` クリア

## Out of Scope
- TUI/CLI のオーケストレーションループ統合（将来的な目標だが本 proposal ではトレイト導入のみ）
- Reducer のステートマシン変更
