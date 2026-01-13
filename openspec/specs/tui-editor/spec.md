# tui-editor Specification

## Purpose
Defines TUI editor integration for opening change files in external editors.
## Requirements
### Requirement: エディタ起動キーバインド

TUIの選択モードで `e` キーを押すと、カーソル位置のchangeディレクトリでエディタが起動しなければならない（SHALL）。

#### Scenario: 選択モードでエディタ起動

- **GIVEN** TUIが選択モードである
- **AND** 変更リストにカーソルが位置している
- **WHEN** ユーザーが `e` キーを押す
- **THEN** TUIが一時停止する
- **AND** `$EDITOR` 環境変数で指定されたエディタが起動する
- **AND** 作業ディレクトリが `openspec/changes/{change_id}/` に設定される
- **AND** エディタに `.` が引数として渡される

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

### Requirement: Parallel Mode Toggle Key

The TUI SHALL support toggling parallel mode using the `=` key, but only when git is available.

#### Scenario: Toggle parallel mode on with `=` key
- **GIVEN** TUI is in selection mode
- **AND** a `.git` directory exists (git repository detected)
- **AND** parallel mode is currently OFF
- **WHEN** user presses `=` key
- **THEN** parallel mode is enabled
- **AND** log displays "Parallel mode: ON"
- **AND** visual indicator shows parallel mode is active

#### Scenario: Toggle parallel mode off with `=` key
- **GIVEN** TUI is in selection mode
- **AND** parallel mode is currently ON
- **WHEN** user presses `=` key
- **THEN** parallel mode is disabled
- **AND** log displays "Parallel mode: OFF"
- **AND** visual indicator is removed

#### Scenario: `=` key hidden when git not available
- **GIVEN** TUI is in selection mode
- **AND** no `.git` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option is NOT displayed in help text
- **AND** pressing `=` key has no effect

#### Scenario: `=` key shown when git available
- **GIVEN** TUI is in selection mode
- **AND** a `.git` directory exists
- **WHEN** TUI renders the footer help text
- **THEN** the `=: parallel` option IS displayed in help text

### Requirement: Parallel Mode State Indicator

The TUI SHALL display a visual indicator when parallel mode is enabled.

#### Scenario: Parallel mode indicator in header
- **GIVEN** parallel mode is enabled
- **WHEN** TUI renders the header
- **THEN** a "[parallel]" badge is displayed in the header
- **AND** the badge uses a distinct color (e.g., cyan)

#### Scenario: No indicator when parallel mode off
- **GIVEN** parallel mode is disabled
- **WHEN** TUI renders the header
- **THEN** no parallel mode badge is displayed

### Requirement: Parallel Mode Toggle During Modes

The TUI SHALL restrict parallel mode toggling based on current app mode.

#### Scenario: Toggle allowed in selection mode
- **GIVEN** TUI is in Selecting mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is toggled

#### Scenario: Toggle allowed in stopped mode
- **GIVEN** TUI is in Stopped mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is toggled

#### Scenario: Toggle blocked in running mode
- **GIVEN** TUI is in Running mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is NOT toggled
- **AND** log displays "Cannot toggle parallel mode while running"

#### Scenario: Toggle blocked in stopping mode
- **GIVEN** TUI is in Stopping mode
- **WHEN** user presses `=` key
- **THEN** parallel mode is NOT toggled

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

The TUI SHALL detect git availability at startup and cache the result.

#### Scenario: git detected at startup
- **GIVEN** user starts the TUI
- **AND** a `.git` directory exists in the current working directory
- **THEN** git_available flag is set to true
- **AND** parallel mode features are enabled

#### Scenario: git not detected at startup
- **GIVEN** user starts the TUI
- **AND** no `.git` directory exists in the current working directory
- **THEN** git_available flag is set to false
- **AND** parallel mode features are hidden
- **AND** no error is displayed (silent degradation)
