## 1. 実装
- [x] 1.1 TUI の `+` 提案作成フローの worktree 作成を「ブランチ付き」に変更する（detached を廃止）
- [x] 1.2 `+` フロー用のブランチ命名 `oso-session-<rand>` と衝突回避（再生成）を実装する
- [x] 1.3 parallel 実行の worktree ブランチ名を `{change_id}` として作成・再利用する
- [x] 1.4 resume は安全に一致判定できる場合のみ再利用し、不整合があれば削除して作り直す
- [x] 1.5 不整合時の削除対象（worktree/ブランチ）の決定とログを整備する

## 2. テスト
- [x] 2.1 `+` フローで作成される worktree が detached ではないことと `oso-session-<rand>` 形式であることをテスト
- [x] 2.2 parallel 実行の worktree ブランチ名が `{change_id}` であることをテスト
- [x] 2.3 resume 判定が安全一致のみを許可し、不整合時は削除・再作成されることをテスト

## 3. 検証
- [x] 3.1 `npx @fission-ai/openspec@latest validate add-worktree-branch-creation --strict`

## Future work
- 3.2 TUI の `+` を押して worktree が作られ、ブランチが確認できることを確認 - Manual testing required
- 3.3 `--parallel` 実行で worktree が作られ、ブランチが確認できることを確認 - Manual testing required
