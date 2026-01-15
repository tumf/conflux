# tui-editor 仕様変更

## MODIFIED Requirements

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
