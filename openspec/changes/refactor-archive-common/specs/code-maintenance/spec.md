# code-maintenance spec delta

## ADDED Requirements

### Requirement: Common Archive Verification

システムは、アーカイブ操作の成功を検証するための共通関数を提供しなければならない（SHALL）。この関数は serial mode と parallel mode の両方で使用される。

#### Scenario: アーカイブ成功の検証

- **GIVEN** change_id = "my-change" のアーカイブ操作が完了した
- **WHEN** `verify_archive_completion()` を呼び出す
- **THEN** change が `openspec/changes/` から削除されていることを確認する
- **AND** change が `openspec/changes/archive/` に存在することを確認する

#### Scenario: 日付プレフィックス付きアーカイブの検証

- **GIVEN** change_id = "my-change" がアーカイブされた
- **WHEN** アーカイブディレクトリ名が "2026-01-12-my-change" 形式である
- **THEN** `verify_archive_completion()` は成功を返す

#### Scenario: アーカイブ失敗の検出

- **GIVEN** archive コマンドが実行されたが、ファイルが移動されていない
- **WHEN** `verify_archive_completion()` を呼び出す
- **THEN** エラーが返され、change ディレクトリがまだ存在することを示す

### Requirement: Common Task Completion Verification

システムは、タスクの完了状態を検証するための共通関数を提供しなければならない（SHALL）。

#### Scenario: タスク完了の確認

- **GIVEN** tasks.md に 10 個のタスクがあり、10 個が完了している
- **WHEN** `verify_task_completion()` を呼び出す
- **THEN** true が返される

#### Scenario: タスク未完了の確認

- **GIVEN** tasks.md に 10 個のタスクがあり、7 個が完了している
- **WHEN** `verify_task_completion()` を呼び出す
- **THEN** false が返され、進捗情報 (7/10) が含まれる

### Requirement: Common Archive Command Execution

システムは、archive コマンドを実行するための共通関数を提供しなければならない（SHALL）。この関数は workspace_path を受け取り、指定された場所でコマンドを実行する。

#### Scenario: メインワークスペースでの実行

- **GIVEN** workspace_path = None
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** カレントディレクトリで archive コマンドが実行される

#### Scenario: 別ワークスペースでの実行

- **GIVEN** workspace_path = Some("/path/to/workspace")
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** 指定されたワークスペースで archive コマンドが実行される
