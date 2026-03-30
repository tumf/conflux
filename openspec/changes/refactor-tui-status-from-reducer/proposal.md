---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/types.rs
  - src/tui/type_impls.rs
  - src/tui/render.rs
  - src/orchestration/state.rs
---

# Change: TUI の QueueStatus enum を廃止し Reducer の display_status() から表示を導出する

**Change Type**: implementation

## Why
TUI の `QueueStatus` enum（`src/tui/types.rs` L55）が Reducer の `ChangeRuntimeState::display_status()` と同一の状態遷移を独自に保持している。`src/tui/state.rs` に307箇所、`render.rs` に59箇所の参照があり、イベントハンドラ内で毎回 `change.queue_status = QueueStatus::Applying` のようにローカルに書き換えている。

一方、Reducer 側は既に `display_status() -> &'static str` で全状態を文字列として返せる。TUI が独自 enum を持つ必要はなく、Reducer の出力をそのまま使えばよい。

## Proposed Solution
**段階的移行**（一度に置換しない）:

1. `ChangeState` に `display_status_cache: String` と `display_color_cache: Color` を追加
2. `render.rs` と `key_handlers.rs` を cache フィールド参照に切り替え
3. `state.rs` のイベントハンドラ内の `queue_status` 書き込みを cache 更新に置き換え
4. `state.rs` の読み取り参照を cache に切り替え
5. `ChangeState` から `queue_status` フィールドを削除
6. `QueueStatus` enum を削除

各ステップで `cargo test` が通る状態を維持する。

## Acceptance Criteria
- `QueueStatus` enum が `src/tui/types.rs` に存在しない
- `src/tui/type_impls.rs` の `impl QueueStatus` が削除されている
- TUI 表示が変更前と同一（同じ文字列・同じ色）
- `cargo test` 全パス
- `cargo clippy -- -D warnings` クリア

## Out of Scope
- Reducer のステートマシン変更
- Web 側の変更（別 proposal `refactor-web-status-derivation` で対応済み）
- イベントディスパッチの変更
