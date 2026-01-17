## 1. 仕様更新
- [x] 1.1 vcs-worktree-operations に worktree作成時のセットアップ実行要件を追加する

## 2. 実装
- [x] 2.1 worktree作成時に `.wt/setup` を検出して実行する処理を追加する
- [x] 2.2 実行失敗時のログとエラー処理を整備する

## 3. 検証
- [x] 3.1 npx @fission-ai/openspec@latest validate add-worktree-setup-script --strict
