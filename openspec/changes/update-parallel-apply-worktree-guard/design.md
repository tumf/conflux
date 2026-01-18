## Context
parallel 実行時に apply コマンドが base リポジトリで走ると作業ツリーが汚れるため、worktree 実行を保証するガードが必要。

## Goals / Non-Goals
- Goals: apply 実行前に worktree 実行であることを検証し、違反時は即時停止する
- Non-Goals: serial 実行の挙動変更、worktree 作成方式の変更

## Decisions
- Decision: apply 実行直前に workspace_path が git worktree と一致するか検証し、失敗時はエラーにする
- Alternatives considered: 事後検証で base を汚したあとに警告のみ（再現性が低く危険なため却下）

## Risks / Trade-offs
- guard 追加により誤検知が起きると parallel 実行が停止するため、worktree 判定は git worktree list 出力に基づき厳密に行う

## Migration Plan
- 機能追加のみ。既存の利用方法は変更不要。

## Open Questions
- なし
