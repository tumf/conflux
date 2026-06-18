## ADDED Requirements

### Requirement: ChangeDequeued イベントで選択状態を解除する

TUI は `OrchestratorEvent::ChangeDequeued` を受信したとき、対象 change の `selected` フラグを `false` に設定し、`display_status_cache` を `"not queued"` に更新しなければならない（MUST）。

#### Scenario: force-kill 後に選択マークが消える

**Given**: Running モードで change `alpha` が `"applying"` 状態で `selected=true` である
**When**: `OrchestratorEvent::ChangeDequeued { change_id: "alpha" }` が TUI に届く
**Then**: `alpha` の `selected` が `false` になる
**And**: `alpha` の `display_status_cache` が `"not queued"` になる
**And**: `alpha` のチェックボックスが `[ ]`（未選択）で表示される
