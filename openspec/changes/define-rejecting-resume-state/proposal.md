---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - openspec/changes/archive/split-rejecting-from-acceptance/specs/orchestration-state/spec.md
  - src/orchestration/state.rs
---

# Rejecting → resume_apply の Reducer 遷移を定義する

**Change Type**: implementation

## Problem / Context

`split-rejecting-from-acceptance` で `ActivityState::Rejecting` が追加されたが、rejection review が "RESUME" を返した場合の `Rejecting → Idle → Applying` 遷移パスが Reducer に定義されていない。

現状の問題:
1. `AcceptanceCompleted` は `Accepting → Idle` に遷移するが、**`Rejecting → Idle` に対応するイベントが存在しない**
2. rejection review が resume_apply を返した場合、activity が `Rejecting` のまま残る可能性がある
3. spec (`split-rejecting-from-acceptance`) は "the active execution stage becomes `Applying`" と定義しているが、Reducer コードに対応する遷移がない

## Proposed Solution

1. `RejectionReviewCompleted` イベント (confirm/resume バリアント付き) を追加し、Reducer が `Rejecting → Idle` (confirm → `Rejected` terminal) または `Rejecting → Applying` (resume) に遷移させる
2. `AcceptanceFailed` と同様に `RejectionReviewFailed` を追加して `Rejecting → Error` terminal パスをカバーする

## Acceptance Criteria

- Rejection review の confirm 結果で `Rejecting → Idle, terminal: Rejected` に遷移すること
- Rejection review の resume 結果で `Rejecting → Applying` に遷移すること
- Rejecting 中にエラーが発生した場合 `Error` terminal に遷移すること
- Rejecting がアイドルに戻らないまま残るケースが存在しないこと (invariants_hold テスト)

## Out of Scope

- Serial モードでの Rejecting サポート
- TUI 表示の変更
