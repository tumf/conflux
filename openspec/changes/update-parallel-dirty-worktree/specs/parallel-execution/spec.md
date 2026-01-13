## MODIFIED Requirements
### Requirement: Git Clean Working Directory Requirement
Git バックエンド使用時、システムは未コミット変更がある場合に警告を表示しつつ並列実行を継続しなければならない（SHALL）。

#### Scenario: CLI warning on uncommitted changes
- **WHEN** `--parallel` フラグで実行される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** 以下の警告メッセージが表示される:
  ```
  Warning: Uncommitted changes detected.
  Parallel mode will continue, but uncommitted changes remain in your working directory.
  Consider committing or stashing if you need isolated workspaces.
  ```
- **AND** 並列実行が開始される
- **AND** 警告のみでは終了コードは非ゼロにならない

#### Scenario: TUI warning on uncommitted changes
- **WHEN** TUI で F5 キーが押される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** ポップアップダイアログが表示される
- **AND** タイトルは "Uncommitted Changes Detected" である
- **AND** 本文に警告内容と継続可能である旨が表示される
- **AND** 並列実行は開始される
