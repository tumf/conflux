# Tasks: Workspace レジューム機能

## 1. WorkspaceManager trait拡張

- [x] 1.1. `find_existing_workspace(change_id: &str) -> Option<WorkspaceInfo>` メソッドを追加
- [x] 1.2. `WorkspaceInfo` 構造体を定義（path, change_id, workspace_name, last_modified）

## 2. jjバックエンド実装

- [x] 2.1. `list_jj_workspaces()` を活用して既存workspaceを検索
- [x] 2.2. workspace名からchange_idを抽出するロジックを実装
- [x] 2.3. workspace再利用時の初期化処理を実装
- [x] 2.4. 複数workspace検出時に最新を選択し古いものを削除するロジックを実装

## 3. Gitバックエンド実装

- [x] 3.1. `git worktree list` で既存worktreeを検索
- [x] 3.2. worktree名からchange_idを抽出するロジックを実装
- [x] 3.3. worktree再利用時の初期化処理を実装
- [x] 3.4. 複数worktree検出時に最新を選択し古いものを削除するロジックを実装

## 4. ParallelExecutor統合

- [x] 4.1. workspace作成前に既存workspaceをチェックするロジックを追加
- [x] 4.2. 既存workspaceがある場合は再利用する処理フローを実装

## 5. CLI対応

- [x] 5.1. `--no-resume` フラグを追加（常に新規作成）
- [x] 5.2. 自動レジューム時のログ出力を実装

## 6. TUI対応

- [x] 6.1. 既存workspace検出・自動再利用時の通知イベントを追加
- [x] 6.2. 再利用中のステータス表示を実装

## 7. テスト

- [x] 7.1. jj workspace検出のユニットテスト
- [x] 7.2. Git worktree検出のユニットテスト
- [x] 7.3. 複数workspace時の最新選択・古いもの削除のユニットテスト
- [x] 7.4. 再利用フローのE2Eテスト

## 8. ドキュメント

- [x] 8.1. README更新（レジューム機能の説明）
- [x] 8.2. CLIヘルプテキスト更新
