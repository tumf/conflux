# Change: worktree ブランチ既存時の安全な再利用

## Why
`git worktree add` が「a branch named ... already exists」で失敗した際に、現状はパス既存として誤分類され、復旧不能に見えるためです。正しい診断と安全なフォールバックで、既存ブランチの再利用を可能にします。

## What Changes
- worktree add 失敗の分類に「ブランチ既存」を追加する
- ブランチ既存かつ他 worktree で未チェックアウトのときに、既存ブランチをアタッチするフォールバックを 1 回だけ実行する
- フォールバック失敗時は元の失敗と再試行失敗を同時にログへ残す

## Impact
- Affected specs: vcs-worktree-operations
- Affected code: src/vcs/git/commands/worktree.rs
