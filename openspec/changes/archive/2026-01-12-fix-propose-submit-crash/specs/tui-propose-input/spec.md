## MODIFIED Requirements

### Requirement: 複数行テキスト入力

提案入力モードは複数行のテキスト入力をサポートしなければならない（SHALL）。

#### Scenario: 改行の入力

- **GIVEN** TUIがProposingモードである
- **WHEN** ユーザーが `Enter` を押す
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

### Requirement: 提案入力の確定とキャンセル

ユーザーは提案入力を確定またはキャンセルできなければならない（SHALL）。確定は `Ctrl+S` で行い、`Enter` は改行として扱われる。

#### Scenario: 入力の確定

- **GIVEN** TUIがProposingモードである
- **AND** テキスト「新しい機能の提案」が入力されている
- **WHEN** ユーザーが `Ctrl+S` を押す
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
- **WHEN** ユーザーが `Ctrl+S` を押す
- **THEN** 警告メッセージ "Proposal text is empty" が表示される
- **AND** TUIはProposingモードのままである

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
- **AND** TUIはProposingモードのままである
- **AND** 入力テキストは保持される

### Requirement: キーヒントの表示

Proposingモードでは適切なキーヒントが表示されなければならない（SHALL）。

#### Scenario: Proposingモードのキーヒント

- **GIVEN** TUIがProposingモードである
- **THEN** フッターに以下のキーヒントが表示される:
  - `Ctrl+S: confirm`
  - `Enter: newline`
  - `Esc: cancel`
