## 1. 仕様更新
- [ ] 1.1 `tui-propose-input` の `+` 起動条件を Worktrees ビュー基準に修正し、ブランチ名 `ws-session-<rand>` を明記する（検証: `openspec/changes/update-tui-specs-for-worktrees-plus-and-d/specs/tui-propose-input/spec.md` に MODIFIED 要件とシナリオがある）
- [ ] 1.2 `Runningモードでは提案作成不可` を `Changesビューでは提案作成不可` にリネームし、シナリオを更新する（検証: `openspec/changes/update-tui-specs-for-worktrees-plus-and-d/specs/tui-propose-input/spec.md` に RENAMED と MODIFIED がある）
- [ ] 1.3 `cli` の worktree 削除操作（`D`）を Worktrees ビュー前提に更新する（検証: `openspec/changes/update-tui-specs-for-worktrees-plus-and-d/specs/cli/spec.md` に MODIFIED 要件とシナリオがある）

## 2. 検証
- [ ] 2.1 `npx @fission-ai/openspec@latest validate update-tui-specs-for-worktrees-plus-and-d --strict` が成功する
