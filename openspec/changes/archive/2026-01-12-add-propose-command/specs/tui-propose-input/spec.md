## ADDED Requirements

### Requirement: 提案入力モード

TUIはSelectモードで `+` キーを押すと提案入力モード（Proposing）に切り替わらなければならない（SHALL）。

#### Scenario: Selectモードから提案入力モードへ切り替え

- **GIVEN** TUIがSelectモードである
- **WHEN** ユーザーが `+` キーを押す
- **AND** `propose_command` が設定されている
- **THEN** TUIがProposingモードに切り替わる
- **AND** 画面中央にテキスト入力ボックスが表示される

#### Scenario: propose_command未設定時の警告

- **GIVEN** TUIがSelectモードである
- **AND** `propose_command` が設定されていない
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 警告メッセージ "propose_command is not configured" が表示される
- **AND** TUIはSelectモードのままである

#### Scenario: Runningモードでは提案入力不可

- **GIVEN** TUIがRunningモードである
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

### Requirement: 複数行テキスト入力

提案入力モードは複数行のテキスト入力をサポートしなければならない（SHALL）。

#### Scenario: 改行の入力

- **GIVEN** TUIがProposingモードである
- **WHEN** ユーザーが `Ctrl+Enter` を押す
- **THEN** 現在のカーソル位置で改行が挿入される
- **AND** カーソルが次の行の先頭に移動する

#### Scenario: 複数行テキストの表示

- **GIVEN** TUIがProposingモードである
- **AND** 入力テキストが3行ある
- **THEN** テキストボックスに3行すべてが表示される
- **AND** 各行の終端が正しく表示される

#### Scenario: 長いテキストのスクロール

- **GIVEN** TUIがProposingモードである
- **AND** 入力テキストがテキストボックスの高さを超える
- **WHEN** ユーザーがカーソルを下に移動する
- **THEN** テキストボックスがスクロールする
- **AND** カーソル位置が常に表示される

### Requirement: CJK文字幅対応

テキスト入力はCJK文字（日本語・中国語・韓国語）の表示幅を正しく計算しなければならない（SHALL）。

#### Scenario: CJK文字の表示幅計算

- **GIVEN** TUIがProposingモードである
- **WHEN** ユーザーが「日本語」と入力する
- **THEN** 3文字は表示幅6セルで計算される（各文字2セル）
- **AND** カーソル位置が正しく計算される

#### Scenario: 混合テキストの表示幅

- **GIVEN** TUIがProposingモードである
- **WHEN** ユーザーが「Hello日本語World」と入力する
- **THEN** 表示幅は16セル（Hello:5 + 日本語:6 + World:5）で計算される

### Requirement: 提案入力の確定とキャンセル

ユーザーは提案入力を確定またはキャンセルできなければならない（SHALL）。

#### Scenario: 入力の確定

- **GIVEN** TUIがProposingモードである
- **AND** テキスト「新しい機能の提案」が入力されている
- **WHEN** ユーザーが `Enter` キーを押す
- **THEN** 入力テキストで `propose_command` が実行される
- **AND** TUIがSelectモードに戻る
- **AND** ログに "Executing propose command..." が表示される

#### Scenario: 入力のキャンセル

- **GIVEN** TUIがProposingモードである
- **AND** テキストが入力されている
- **WHEN** ユーザーが `Esc` キーを押す
- **THEN** 入力がキャンセルされる
- **AND** TUIがSelectモードに戻る
- **AND** `propose_command` は実行されない

#### Scenario: 空テキストでの確定は無視

- **GIVEN** TUIがProposingモードである
- **AND** テキスト入力が空である
- **WHEN** ユーザーが `Enter` キーを押す
- **THEN** 警告メッセージ "Proposal text is empty" が表示される
- **AND** TUIはProposingモードのままである

### Requirement: propose_command の設定

設定ファイルで `propose_command` を定義できなければならない（SHALL）。

#### Scenario: propose_commandの設定と展開

- **GIVEN** 設定ファイルに以下が定義されている:
  ```jsonc
  {
    "propose_command": "opencode run '{proposal}'"
  }
  ```
- **WHEN** ユーザーが「新機能追加」と入力して確定する
- **THEN** `opencode run '新機能追加'` が実行される

#### Scenario: 複数行テキストの展開

- **GIVEN** `propose_command` が `"agent --prompt '{proposal}'"` に設定されている
- **WHEN** ユーザーが複数行テキストを入力する:
  ```
  行1
  行2
  ```
- **THEN** 改行を含むテキストがそのまま展開される

### Requirement: コマンド実行とログ表示

`propose_command` はバックグラウンドで実行され、結果がログに表示されなければならない（SHALL）。

#### Scenario: コマンド実行成功

- **GIVEN** `propose_command` が設定されている
- **WHEN** 提案入力を確定する
- **THEN** コマンドがバックグラウンドで実行される
- **AND** 実行開始ログが表示される
- **AND** コマンド完了時に成功ログが表示される
- **AND** TUIは操作可能なままである

#### Scenario: コマンド実行失敗

- **GIVEN** `propose_command` が不正なコマンドを含む
- **WHEN** 提案入力を確定する
- **THEN** エラーログが表示される
- **AND** TUIはSelectモードで操作可能なままである

### Requirement: キーヒントの表示

Proposingモードでは適切なキーヒントが表示されなければならない（SHALL）。

#### Scenario: Proposingモードのキーヒント

- **GIVEN** TUIがProposingモードである
- **THEN** フッターに以下のキーヒントが表示される:
  - `Enter: confirm`
  - `Ctrl+Enter: newline`
  - `Esc: cancel`
