## 1. 実装
- [x] 1.1 worktree add 失敗分類に「ブランチ既存」を追加し、判定順序を調整する
  - 検証: `cargo test worktree_add_error_classification`
- [x] 1.2 ブランチ既存時に既存ブランチをアタッチするフォールバックを 1 回だけ実行する（他 worktree でチェックアウト済みなら実行しない）
  - 検証: `cargo test worktree_add_existing_branch_attach_success`
- [x] 1.3 フォールバック失敗時のエラーに元の失敗と再試行失敗を両方含める
  - 検証: `cargo test worktree_add_existing_branch_attach_failure`

## 2. テスト
- [x] 2.1 ブランチ既存かつ未チェックアウトで成功するケースのテストを追加する
  - 検証: `cargo test worktree_add_existing_branch_attach_success`
- [x] 2.2 ブランチ既存かつ他 worktree でチェックアウト済みの失敗テストを追加する
  - 検証: `cargo test worktree_add_existing_branch_attach_failure`
