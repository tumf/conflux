# Change: Git コマンドモジュールの分割

## Why
src/vcs/git/commands.rs が肥大化しており、変更時にどの領域へ影響があるか追跡しづらい。責務ごとのモジュール分割で保守性を上げる。

## What Changes
- Git コマンド群を basic / commit / worktree / merge などの責務別モジュールに分割する。
- 既存の公開 API と挙動は維持し、呼び出し側への影響を最小限にする。
- 既存挙動は変更せず、既存テストと追加テストで同一性を確認する。

## Impact
- Affected specs: code-maintenance
- Affected code: src/vcs/git/commands.rs, src/vcs/git/commands/*, src/vcs/git/mod.rs
