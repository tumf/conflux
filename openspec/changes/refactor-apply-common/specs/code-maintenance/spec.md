# code-maintenance spec delta

## ADDED Requirements

### Requirement: Common Apply Iteration Logic

システムは、apply コマンドの反復実行を管理するための共通ロジックを提供しなければならない（SHALL）。このロジックは serial mode と parallel mode の両方で使用される。

#### Scenario: 単一 apply の実行

- **GIVEN** change_id = "my-change" と apply コマンドが設定されている
- **WHEN** `execute_apply_iteration()` を呼び出す
- **THEN** apply コマンドが実行される
- **AND** 実行後の進捗情報が返される

#### Scenario: 反復 apply の実行

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** タスクが 100% 完了するまで反復する
- **THEN** 各反復で進捗をチェックする
- **AND** 完了したら反復を終了する

#### Scenario: 最大反復回数の制限

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** 50 回の反復後もタスクが完了しない
- **THEN** エラーが返される

### Requirement: Common Progress Commit Creation

システムは、進捗コミットを作成するための共通関数を提供しなければならない（SHALL）。この関数は VCS の種類に関係なく動作する。

#### Scenario: jj でのプログレスコミット

- **GIVEN** VCS backend が jj である
- **WHEN** `create_progress_commit()` を呼び出す
- **THEN** `jj describe` でコミットメッセージが設定される

#### Scenario: git でのプログレスコミット

- **GIVEN** VCS backend が git である
- **WHEN** `create_progress_commit()` を呼び出す
- **THEN** `git commit` でコミットが作成される

### Requirement: VCS Operations through WorkspaceManager

parallel/executor.rs 内の VCS 操作は、直接コマンドを実行する代わりに `WorkspaceManager` trait を使用しなければならない（SHALL）。

#### Scenario: コミットメッセージの設定

- **GIVEN** workspace_path でコミットメッセージを設定する必要がある
- **WHEN** `workspace_manager.set_commit_message()` を呼び出す
- **THEN** VCS backend に応じた適切なコマンドが実行される

#### Scenario: リビジョンの取得

- **GIVEN** workspace の現在のリビジョンを取得する必要がある
- **WHEN** `workspace_manager.get_revision_in_workspace()` を呼び出す
- **THEN** 現在のリビジョン ID が返される
