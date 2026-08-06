# tui-editor Specification

## Purpose
Defines TUI editor integration for opening change files in external editors.
## Requirements

### Requirement: エディタ起動キーバインド

TUIの選択モードで `e` キーを押すと、カーソル位置のchangeの`proposal.md`ファイルを優先的に開き、ファイルが存在しない場合はchangeディレクトリにフォールバックしてエディタが起動しなければならない（SHALL）。

#### Scenario: 選択モードでproposal.mdを直接開く

- **GIVEN** TUIが選択モードである
- **AND** 変更リストにカーソルが位置している
- **AND** `openspec/changes/{change_id}/proposal.md`ファイルが存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** TUIが一時停止する
- **AND** `$EDITOR` 環境変数で指定されたエディタが起動する
- **AND** エディタに `openspec/changes/{change_id}/proposal.md` のパスが引数として渡される
- **AND** ログに "Launching editor: {editor} (file: openspec/changes/{change_id}/proposal.md)" が記録される

#### Scenario: proposal.mdが存在しない場合のディレクトリフォールバック

- **GIVEN** TUIが選択モードである
- **AND** 変更リストにカーソルが位置している
- **AND** `openspec/changes/{change_id}/proposal.md`ファイルが存在しない
- **AND** `openspec/changes/{change_id}/`ディレクトリが存在する
- **WHEN** ユーザーが `e` キーを押す
- **THEN** TUIが一時停止する
- **AND** `$EDITOR` 環境変数で指定されたエディタが起動する
- **AND** 作業ディレクトリが `openspec/changes/{change_id}/` に設定される
- **AND** エディタに `.` が引数として渡される
- **AND** ログに "Launching editor: {editor} (cwd: openspec/changes/{change_id}/)" が記録される

#### Scenario: エディタ終了後のTUI復帰

- **GIVEN** エディタが起動している
- **WHEN** ユーザーがエディタを終了する
- **THEN** TUIが復帰する
- **AND** 画面が再描画される
- **AND** カーソル位置が維持される

#### Scenario: 実行モードではエディタ起動不可

- **GIVEN** TUIが実行モード（Running）である
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない
- **AND** TUIの表示は変更されない

#### Scenario: エラーモードではエディタ起動不可

- **GIVEN** TUIがErrorモードである
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない

#### Scenario: changeディレクトリが存在しない場合のエラー

- **GIVEN** TUIが選択モードである
- **AND** カーソル位置のchangeディレクトリが存在しない
- **AND** `proposal.md`ファイルも存在しない
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エラーログ "Change not found: {change_id}" が表示される
- **AND** TUIは正常に動作を継続する

#### Scenario: エディタプロセス起動失敗

- **GIVEN** `$EDITOR` で指定されたコマンドが存在しない
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エラーログ "Failed to launch editor" が表示される
- **AND** TUIが復帰する
- **AND** TUIは正常に動作を継続する

### Requirement: EDITOR環境変数

エディタは `$EDITOR` 環境変数から取得しなければならない（SHALL）。

#### Scenario: EDITOR環境変数が設定されている

- **GIVEN** `$EDITOR` 環境変数が `nvim` に設定されている
- **WHEN** ユーザーが `e` キーを押す
- **THEN** `nvim .` が実行される

#### Scenario: EDITOR環境変数が未設定

- **GIVEN** `$EDITOR` 環境変数が設定されていない
- **WHEN** ユーザーが `e` キーを押す
- **THEN** `vi .` がフォールバックとして実行される

#### Scenario: EDITORに引数が含まれている

- **GIVEN** `$EDITOR` 環境変数が `code --wait` に設定されている
- **WHEN** ユーザーが `e` キーを押す
- **THEN** `code --wait .` が実行される

### Requirement: ヘルプ表示の更新

選択モードのヘルプテキストにエディタ起動キーを含めなければならない（SHALL）。

#### Scenario: 選択モードのヘルプ表示

- **WHEN** TUIが選択モードである
- **THEN** ヘルプテキストに `e: edit` が表示される
- **AND** 他のキーバインド（↑↓/jk: move, Space: queue, @: approve, F5: run, q: quit）も表示される

### Requirement: エラーハンドリング

エディタ起動に失敗した場合、適切なエラーメッセージを表示しなければならない（SHALL）。

#### Scenario: changeディレクトリが存在しない

- **GIVEN** TUIが選択モードである
- **AND** カーソル位置のchangeディレクトリが存在しない
- **AND** `proposal.md`ファイルも存在しない
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エラーログが表示される
- **AND** TUIは正常に動作を継続する

#### Scenario: エディタプロセス起動失敗

- **GIVEN** `$EDITOR` で指定されたコマンドが存在しない
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エラーログ "Failed to launch editor" が表示される
- **AND** TUIが復帰する
- **AND** TUIは正常に動作を継続する

### Requirement: 変更一覧が空の場合

変更一覧が空の場合、エディタ起動は無効でなければならない（SHALL）。

#### Scenario: 変更一覧が空でエディタ起動試行

- **GIVEN** TUIが選択モードである
- **AND** 変更一覧が空である
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない
- **AND** 警告メッセージ "No change selected" がログに表示される

### Requirement: Proposal編集時のオーケストレーションステータス維持
TUIでproposal編集を開始・終了しても、オーケストレーションステータスは変更してはならない（MUST）。

#### Scenario: Proposal編集開始
- **GIVEN** TUIが選択モードであり、現在のオーケストレーションステータスが表示されている
- **WHEN** ユーザーが `e` キーでproposal編集を開始する
- **THEN** オーケストレーションステータスは編集開始前の値を維持する
- **AND** ヘッダのステータス表示は変更されない

#### Scenario: Proposal編集終了
- **GIVEN** proposal編集のためにエディタが起動している
- **WHEN** ユーザーがエディタを終了しTUIが復帰する
- **THEN** オーケストレーションステータスは編集開始前の値を維持する

### Requirement: Git Detection at TUI Startup

The local executable TUI SHALL verify repository identity and Git command availability before orchestration can start. It SHALL NOT silently degrade to serial execution.

#### Scenario: Git repository is usable at startup

- **GIVEN** user starts the local TUI in a Git repository
- **AND** the Git command is available
- **WHEN** startup validation completes
- **THEN** worktree orchestration controls are available
- **AND** no execution-mode toggle is displayed

#### Scenario: Git repository is unavailable at startup

- **GIVEN** user starts the local executable TUI outside a Git repository or without the Git command
- **WHEN** startup validation runs
- **THEN** startup fails with an actionable error before orchestration side effects
- **AND** the TUI does not offer a serial fallback
