## MODIFIED Requirements
### Requirement: Common Archive Command Execution
システムは、archive コマンドを実行するための共通関数を提供しなければならない（SHALL）。この関数は workspace_path を受け取り、指定された場所でコマンドを実行する。さらに、archive が成功した後に Git backend で未コミット変更が残っている場合、`git add -A` と `git commit -m "Archive: {change_id}"` 相当の操作で変更をコミットしなければならない（SHALL）。

#### Scenario: メインワークスペースでの実行
- **GIVEN** workspace_path = None
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** カレントディレクトリで archive コマンドが実行される

#### Scenario: 別ワークスペースでの実行
- **GIVEN** workspace_path = Some("/path/to/workspace")
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** 指定されたワークスペースで archive コマンドが実行される

#### Scenario: シリアルモードのアーカイブ後コミット
- **GIVEN** VCS backend が Git である
- **AND** serial モードで archive コマンドが成功した
- **AND** `git status --porcelain` が非空である
- **WHEN** archive の後処理が完了する
- **THEN** `git add -A` と `git commit -m "Archive: {change_id}"` 相当でコミットが作成される
