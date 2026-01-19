# Change: TUIアーカイブ途中の進捗ゼロ化を防止

## Why
TUIでアーカイブ途中（worktree上でファイル移動済み・コミット未完了）に自動更新が走ると、`parse_change_with_worktree_fallback` が 0/0 を返しても無条件に上書きされ、進捗が0/0にリセットされる。

## What Changes
- `src/tui/runner.rs` の自動更新処理で、進捗が 0/0 の場合はアーカイブ先を試し、それでも 0/0 なら既存の値を保持するように修正する。

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/runner.rs` (L356-358)
