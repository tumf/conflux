# CLI Spec Delta: fix-completed-status-waiting

## MODIFIED Requirements

### Requirement: 実行モードダッシュボード

TUIは実行モードでダッシュボード形式のUIを表示しなければならない（SHALL）。

#### Scenario: 処理完了時の表示

- **WHEN** 全てのキュー内変更の処理が完了する
- **THEN** ヘッダーのステータスが「Completed」に変更される
- **AND** ステータスパネルの左側に「Done」が緑色で表示される
- **AND** TUIは表示を維持し、ユーザーが `q` キーで終了できる

#### Scenario: 完了後のキュー変更

- **WHEN** AppModeがCompletedである
- **AND** ユーザーがSpaceキーを押す
- **THEN** NotQueued状態の変更はQueuedに変更できる
- **AND** Queued状態の変更はNotQueuedに変更できる
- **AND** Completed/Archived/Error状態の変更は変更できない

#### Scenario: 完了後の再実行

- **WHEN** AppModeがCompletedである
- **AND** キューに変更が追加されている
- **AND** ユーザーがF5キーを押す
- **THEN** AppModeがRunningに変更される
- **AND** キュー内の変更の処理が開始される
