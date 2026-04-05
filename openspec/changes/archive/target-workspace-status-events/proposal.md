---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/orchestration-events/spec.md
  - src/orchestration/state.rs
---

# WorkspaceStatus 更新イベントを change 単位で正しく適用する

**Change Type**: implementation

## Problem / Context

`ExecutionEvent::WorkspaceStatusUpdated` の reducer 反映 (`src/orchestration/state.rs` L962-988) は `current_change_id` に依存している。並列モードでは複数 change が同時進行しうるため、WorkspaceStatus 更新が誤った change に適用され、`applying/accepting/rejecting/archiving/resolving` が別行に表示される危険がある。

これは `WorkspaceStatus` と `ChangeRuntimeState` の二重管理を悪化させ、active stage の正典が崩れる。

## Proposed Solution

`WorkspaceStatusUpdated` イベントを workspace 名だけでなく `change_id` を伴う targeted event に変更し、Reducer は `current_change_id` ではなく対象 change を直接更新する。

併せて、`ApplyStarted` / `AcceptanceStarted` / `ArchiveStarted` / `ResolveStarted` などの専用イベントを active stage の正典とし、`WorkspaceStatusUpdated` は互換補助または削除候補として位置付ける。

## Acceptance Criteria

- 並列モードで複数 change 実行中でも WorkspaceStatus 更新が誤った change に適用されないこと
- Reducer の active stage が targeted event により change 単位で更新されること
- `current_change_id` はステータス同期の判定材料として不要になること
- 回帰テストで並列実行中の accepting/rejecting/resolving の表示先が安定すること

## Out of Scope

- `WorkspaceStatus` enum 自体の削除
- TUI/ダッシュボードの見た目変更
