# Tasks: Workspace レジューム機能

## 1. WorkspaceManager trait拡張

- [ ] 1.1. `find_existing_workspace(change_id: &str) -> Option<WorkspaceInfo>` メソッドを追加
- [ ] 1.2. `WorkspaceInfo` 構造体を定義（path, change_id, progress, last_modified）
- [ ] 1.3. `can_resume_workspace(workspace: &WorkspaceInfo) -> bool` メソッドを追加

## 2. jjバックエンド実装

- [ ] 2.1. `list_jj_workspaces()` を活用して既存workspaceを検索
- [ ] 2.2. workspace名からchange_idを抽出するロジックを実装
- [ ] 2.3. workspace内のtasks.md進捗を確認するロジックを実装
- [ ] 2.4. workspace再利用時の初期化処理を実装

## 3. Gitバックエンド実装

- [ ] 3.1. `git worktree list` で既存worktreeを検索
- [ ] 3.2. worktree名からchange_idを抽出するロジックを実装
- [ ] 3.3. worktree内のtasks.md進捗を確認するロジックを実装
- [ ] 3.4. worktree再利用時の初期化処理を実装

## 4. ParallelExecutor統合

- [ ] 4.1. workspace作成前に既存workspaceをチェックするロジックを追加
- [ ] 4.2. 再利用可能なworkspaceがある場合の処理フローを実装
- [ ] 4.3. 再利用/新規作成の選択ロジックを実装

## 5. CLI対応

- [ ] 5.1. `--resume` フラグを追加（デフォルト: 自動検出して確認）
- [ ] 5.2. `--no-resume` フラグを追加（常に新規作成）
- [ ] 5.3. 検出時の確認プロンプトを実装

## 6. TUI対応

- [ ] 6.1. 既存workspace検出時の通知イベントを追加
- [ ] 6.2. 再利用確認ダイアログを実装
- [ ] 6.3. 再利用中のステータス表示を実装

## 7. テスト

- [ ] 7.1. jj workspace検出のユニットテスト
- [ ] 7.2. Git worktree検出のユニットテスト
- [ ] 7.3. 進捗確認ロジックのユニットテスト
- [ ] 7.4. 再利用フローのE2Eテスト

## 8. ドキュメント

- [ ] 8.1. README更新（レジューム機能の説明）
- [ ] 8.2. CLIヘルプテキスト更新
