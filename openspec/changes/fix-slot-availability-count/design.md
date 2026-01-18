## Context
並列実行の空きスロット算出で worktree 数をそのまま使っており、クリーンアップ済みや停止状態の worktree が残っていると空きスロットが 0 と判定される可能性がある。実行中 change のみをアクティブとして数える必要がある。

## Goals / Non-Goals
- Goals: 実行中の change のみをアクティブとして空きスロットを計算する
- Goals: キュー追加が空きスロットに反映される
- Non-Goals: 並列実行全体のスケジューリングアルゴリズム変更

## Decisions
- Decision: `WorkspaceStatus` を参照してアクティブ判定を行う
- Decision: アクティブ判定は並列実行の空きスロット計算にのみ影響させる

## Risks / Trade-offs
- 状態遷移が不十分だと誤ってアクティブ判定されるため、status 更新箇所の整合性を確認する必要がある

## Migration Plan
- 既存の status を維持しつつ、アクティブ判定を追加する

## Open Questions
- なし
