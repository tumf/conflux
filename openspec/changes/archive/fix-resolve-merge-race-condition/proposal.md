---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
---

# Fix: resolve_merge の is_resolving 即時設定による二重 resolve 防止

**Change Type**: implementation

## Problem/Context

TUI の Changes ビューで、ある change が resolving 中に別の MergeWait 状態の change に対して M キーを押すと、本来 ResolveWait（キュー待ち）に遷移すべきところ、即座に resolving が開始されてしまう。

### 根本原因

`resolve_merge()` メソッド（`src/tui/state.rs`）の即時開始パスで、`TuiCommand::ResolveMerge` を返す前に `self.is_resolving = true` を設定していない。`is_resolving` フラグは後続の `ResolveStarted` イベントハンドラ（`handle_resolve_started`）で初めて `true` になるが、このイベントは非同期で到着するため、イベント到着前に別の M キー操作が入ると `is_resolving == false` と判定され、2つ目の resolve も即時開始パスに入る。

## Proposed Solution

`resolve_merge()` の即時開始パス（`is_resolving == false` 分岐）で、`TuiCommand::ResolveMerge` を返す前に `self.is_resolving = true` を設定する。`handle_resolve_started` で再度 `true` に設定されるが冪等なので問題ない。`handle_resolve_failed` / `handle_resolve_completed` で `false` に戻されるためエラー時のクリーンアップも正常。

## Acceptance Criteria

1. resolving 中の change がある状態で、別の MergeWait change に M を押すと ResolveWait に遷移し、resolve キューに追加される
2. resolving 中でない状態で MergeWait change に M を押すと、従来通り即座に resolving が開始される
3. resolve 完了後、キューの次の change が自動開始される（既存動作の維持）
4. resolve 失敗時に is_resolving が正しく false に戻る（既存動作の維持）

## Out of Scope

- resolve キューの優先順位変更
- Web UI 側の同等修正（Web UI は別の状態管理を使用）
