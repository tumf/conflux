---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - src/orchestration/state.rs
  - src/server/api/ws.rs
---

# WebSocket API 状態表示を Reducer 正典に統一する

**Change Type**: implementation

## Problem / Context

`orchestration-state` spec は "consumers SHALL NOT own an independent lifecycle copy" と定義しているが、WebSocket API (`src/server/api/ws.rs` L327-341) は `detect_workspace_state()` による git ファイル検出結果から独自に表示ステータスを導出している。

具体的な不整合:
1. `WorkspaceState::Created` → `"queued"` と表示されるが、Reducer 側では `not queued` の可能性がある
2. `WorkspaceState::Applied` → `"archiving"` と表示されるが、実際にはまだ archiving は開始されていない（accept/apply/archive のいずれかに進む中間状態）
3. base branch 上の `REJECTED.md` 存在で `"rejected"` を返すが、Reducer の `TerminalState::Rejected` とは二重経路
4. Reducer が `accepting` や `resolving` を表示するケースが ws.rs のマッピングに存在しない

## Proposed Solution

WebSocket の per-change ステータス導出を `ChangeRuntimeState.display_status()` ベースに切り替え、`detect_workspace_state()` ベースのマッピングを削除する。

Server モードで `OrchestratorState` へアクセスできない場合のフォールバックとして worktree 検出を残すが、Reducer が利用可能な場合は常に Reducer 正典を優先する。

## Acceptance Criteria

- WebSocket API が返す change ステータスが `ChangeRuntimeState.display_status()` と同一であること
- `accepting`, `rejecting`, `resolving`, `merge wait`, `resolve pending`, `blocked` ステータスが正しく表示されること
- TUI とダッシュボードの表示が一致すること
- 既存の `detect_workspace_state` テストは影響を受けないこと

## Out of Scope

- `detect_workspace_state` 自体の enum 拡張 (Accepting 追加等) — dispatch/resume 用途としては現行で正しい
- `WorkspaceStatus` enum の廃止
