## Context
TUIのWorktreesビューでは削除操作が実行中changeの存在で一律ブロックされるため、未関連のworktree整理ができない。

## Goals / Non-Goals
- Goals:
  - 実行中でも削除対象worktreeが未関連またはNotQueuedであれば削除可能にする
  - 関連changeが実行中/queuedの場合は削除を拒否する
- Non-Goals:
  - 実行中changeに紐づくworktreeの削除を許可する
  - Worktreesビュー以外の削除フローの刷新

## Decisions
- Decision: worktreeブランチ名からchange_idを抽出し、Changes一覧のqueue_statusと照合して削除可否を判断する
- Alternatives considered:
  - 実行中の有無だけで全削除を許可する: 実行中worktreeの誤削除リスクが高い

## Risks / Trade-offs
- ブランチ名からchange_idを抽出できないworktreeは未関連として扱うため、削除可否が広く許可される可能性がある
- 既存のwarning文言変更によりログ確認の見え方が変わる

## Migration Plan
- なし（TUIの削除判定ロジック変更のみ）

## Open Questions
- なし
