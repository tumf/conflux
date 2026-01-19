# Change: 実行中でも無関係worktreeを削除可能にする

## Why
TUIのWorktreesビューでは、実行中のchangeが1件でもあると全worktreeの削除がブロックされます。Changes一覧に存在しない、またはNotQueuedのchangeに紐づくworktreeは実行中でも削除可能にして、不要なworktreeを整理できるようにします。

## What Changes
- Worktreesビューで削除対象worktreeのchange関連性を判定し、実行中changeと無関係なら削除を許可する
- 選択worktreeがQueued/Processing/Archiving/Resolving/Accepting/MergeWaitのchangeに紐づく場合は削除を拒否する
- changeに紐づいていないworktreeは実行中でも削除を許可する

## Impact
- Affected specs: cli
- Affected code: src/tui/state/mod.rs, src/tui/runner.rs, src/vcs/git/mod.rs
