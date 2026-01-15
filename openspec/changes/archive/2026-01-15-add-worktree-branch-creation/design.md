## Context
本プロジェクトは parallel 実行の隔離ワークスペースとして Git worktree を利用している。
また、TUI の `+` キーによる提案作成フローは、一時ディレクトリに Git worktree を作成し、その worktree を `cwd` にして `worktree_command` を起動する。

しかし detached worktree は「現在どのブランチで作業しているか」が明確でなく、提案作成・コミット・共有の導線が悪い。

## Goals
- Git worktree を作成するフロー（parallel / `+`）では、常にブランチを作成して worktree をブランチに紐づける。
- 作成直後から通常の Git 操作（commit / push / PR）が自然に行える。

## Non-Goals
- ブランチ命名規則の config 化
- 定常的なworktreeクリーンアップ戦略の全面変更
- jj backends の変更

## Decisions
### Decision 1: detached worktree を作らない
TUI `+` フローを含め、worktree は `git worktree add -b <branch> <path> <base_rev>` で作成する。

### Decision 2: `+` フローは `oso-session-<rand>` をブランチ名に使う
提案作成フローでは change_id が未確定なため、`oso-session-<rand>` を採用する。
- `<rand>` は短いランダム値で衝突回避する

### Decision 3: resume は安全な一致判定ができる場合のみ行う
- worktree/ブランチの整合が取れない場合は自動削除し、新規作成でやり直す
- 破壊的な修復（参照の付け替え）は行わない

## Risks / Trade-offs
- `+` を何度も実行するとローカルブランチが増える。
  - ただし worktree 自体も残す仕様であり、ブランチ増加はその自然な帰結である。

## Open Questions
- `+` フローのブランチを、ユーザーが後から change_id ブランチへ rename する運用を推奨するか（今回は spec には踏み込まない）。
