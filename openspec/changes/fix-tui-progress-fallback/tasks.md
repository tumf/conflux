## 1. TUI worktree fallback配線
- [ ] 1.1 AppStateにworktreeパスのスナップショットを保持する（TUI refreshで更新されることを確認）
- [ ] 1.2 ChangesRefreshedイベントにworktreeパス情報を追加し、runnerで収集して送信する
- [ ] 1.3 event処理（ArchiveStarted/ChangeArchived/ResolveCompleted/MergeCompleted）でworktree fallbackを使って進捗を再取得する

## 2. 進捗保持ロジックの統一
- [ ] 2.1 update_changesでarchiving/resolving/archived/merged時の0/0を保持し、fallback取得を試みる
- [ ] 2.2 ProgressUpdatedイベントで0/0を受け取った場合に進捗を保持する

## 3. テスト更新
- [ ] 3.1 worktreeにtasks.mdがある場合のArchiveStarted進捗更新テストを追加する
- [ ] 3.2 worktreeにarchived tasks.mdがある場合のMergeCompleted進捗更新テストを追加する
- [ ] 3.3 fallback失敗時に進捗が保持されることを確認する

## 4. 検証
- [ ] 4.1 `cargo test tui::state` を実行し、TUI進捗関連テストが成功することを確認する
