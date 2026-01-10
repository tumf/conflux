# tui-editor Specification

## Purpose

TUIの選択モードにおいて、カーソル位置のchangeディレクトリでエディタを起動する機能を提供する。

## ADDED Requirements

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

#### Scenario: 完了モードではエディタ起動不可

- **GIVEN** TUIがCompletedモードである
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない

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
