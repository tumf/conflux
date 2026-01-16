# parallel-execution Spec Delta

## ADDED Requirements

### Requirement: AI エージェントクラッシュリカバリー

Apply または Archive コマンドが異常終了（exit code ≠ 0）した場合、システムは自動的にリトライしなければならない（SHALL）。

リトライの動作は以下の通りとする：
- コマンドの終了ステータスを確認
- 終了ステータスが 0 以外の場合、リトライを試みる
- リトライ前に 2 秒間の待機を行う
- 最大リトライ回数に達した場合、エラーを返却する

Apply コマンドのリトライ回数は `max_apply_iterations` の値を使用する。
Archive コマンドのリトライ回数は `ARCHIVE_COMMAND_MAX_RETRIES` の値を使用する。

**変更理由**: AI エージェント（OpenCode, Claude Code など）がクラッシュした場合でも、一時的なエラーであれば自動的に回復できるようにするため。

#### Scenario: Apply コマンドクラッシュ時の自動リトライ

- **GIVEN** Apply コマンドが実行される
- **AND** `max_apply_iterations` が 3 に設定されている
- **WHEN** Apply コマンドが exit code 1 で異常終了する
- **THEN** システムは 2 秒待機後に Apply コマンドを再実行する
- **AND** ログに「Apply command crashed (iteration 1/3), exit code: 1. Retrying in 2s...」が出力される

#### Scenario: Apply コマンドリトライ後の正常終了

- **GIVEN** Apply コマンドが実行される
- **AND** 1 回目の実行が exit code 1 で異常終了する
- **WHEN** 2 回目の実行が exit code 0 で正常終了する
- **THEN** Apply は正常完了として扱われる
- **AND** エラーは返却されない

#### Scenario: Apply コマンド最大リトライ回数到達

- **GIVEN** Apply コマンドが実行される
- **AND** `max_apply_iterations` が 3 に設定されている
- **WHEN** Apply コマンドが 3 回連続で異常終了する
- **THEN** システムは「Apply command failed after 3 attempts with exit code: ...」エラーを返却する
- **AND** ワークスペースは保持される（既存の Workspace Preservation on Error 要件に従う）

#### Scenario: Archive コマンドクラッシュ時の自動リトライ

- **GIVEN** Archive コマンドが実行される
- **AND** `ARCHIVE_COMMAND_MAX_RETRIES` が 2 に設定されている
- **WHEN** Archive コマンドが exit code 1 で異常終了する
- **THEN** システムは 2 秒待機後に Archive コマンドを再実行する
- **AND** ログに「Archive command crashed (attempt 1/3), exit code: 1. Retrying in 2s...」が出力される

#### Scenario: Archive コマンドリトライ後の正常終了

- **GIVEN** Archive コマンドが実行される
- **AND** 1 回目の実行が exit code 1 で異常終了する
- **WHEN** 2 回目の実行が exit code 0 で正常終了する
- **AND** archive verification が成功する
- **THEN** Archive は正常完了として扱われる
- **AND** エラーは返却されない

#### Scenario: Archive コマンド最大リトライ回数到達

- **GIVEN** Archive コマンドが実行される
- **AND** `ARCHIVE_COMMAND_MAX_RETRIES` が 2 に設定されている
- **WHEN** Archive コマンドが 3 回連続で異常終了する
- **THEN** システムは「Archive command failed after 3 attempts with exit code: ...」エラーを返却する
- **AND** ワークスペースは保持される

#### Scenario: リトライ中の TUI イベント通知

- **GIVEN** TUI モードで並列実行中
- **AND** Apply または Archive コマンドがクラッシュする
- **WHEN** リトライが開始される
- **THEN** ログペインに「Retrying in 2s...」メッセージが表示される
- **AND** change のステータスは「Applying」または「Archiving」のまま維持される
