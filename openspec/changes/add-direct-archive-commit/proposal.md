---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/archive.rs
  - src/vcs/git/commands.rs
---

# Change: ensure_archive_commit で AI resolve の前に直接 git commit を試行する

**Change Type**: implementation

## Why

`ensure_archive_commit()` は archive 後の dirty working tree を検出すると、AI エージェント（resolve コマンド）に `git add -A && git commit` を委任する。しかし、この操作は純粋に機械的であり AI に委任する必要がない。AI エージェントがコミットに失敗すると `is_archive_commit_complete()` の verification が失敗してエラーになる。実際にこのエラーが production で発生している。

## What Changes

- `ensure_archive_commit()` で AI resolve コマンドを呼ぶ前に、直接 `git add -A && git commit -m "Archive: {change_id}"` を実行する
- 直接コミットが成功した場合は AI resolve を呼ばずに完了
- 直接コミットが失敗した場合（pre-commit hook 等）のみ、既存の AI resolve にフォールバック

## Impact

- Affected specs: parallel-execution
- Affected code: `src/execution/archive.rs`

## Acceptance Criteria

- dirty working tree の状態で `ensure_archive_commit` を呼ぶと、AI resolve なしで直接 git commit が行われる
- 直接 commit が成功した場合、`is_archive_commit_complete()` が `true` を返す
- 直接 commit が失敗した場合、既存の AI resolve コマンドにフォールバックする
- pre-commit hook がファイルを変更するケースでは AI resolve が対応する

## Out of Scope

- archive コマンド自体の変更
- serial モードの archive フロー変更（`ensure_archive_commit` は共通パスなので自動的に適用される）
