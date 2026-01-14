## 1. Worktree検出/表示
- [x] 1.1 Change一覧で`change_id`ごとのworktree存在フラグを取得できるAPIを追加する（`WorkspaceManager`経由で取得でき、unit/e2eで参照可能）
- [x] 1.2 Change一覧の行にworktreeインジケータ（例: `WT`）を表示する（worktreeあり時のみ表示、無し時は非表示）

## 2. 削除操作
- [x] 2.1 Selectモードで`D`キーを押すと削除確認ダイアログを表示する（キャンセル/確定で挙動が分岐する）
- [x] 2.2 worktree削除処理を実装する（存在時のみ削除し、存在しない場合は通知メッセージを表示する）
- [x] 2.3 Running/Processing中のchangeでは削除を拒否し、理由メッセージを表示する

## 3. テスト/検証
- [x] 3.1 TUI状態/表示に関するテストを追加する（worktree表示と削除ガードを検証する）
- [x] 3.2 `cargo test` を実行し、全テストが成功することを確認する
